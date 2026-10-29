// Package config reads Devora's two-level JSON configuration: a global file (see ConfigPath) and a config.json in each profile's root directory.
// The format is shared with the Devora app, which also reads keys debi ignores (terminal.default-app, prepare-command, repos).
//
// Keys resolve profile first, then global (see Get).
// Resolution is per leaf, so the two levels can contribute different leaves under the same object (e.g., global sets task-tracker.asana.workspace-id, profile sets task-tracker.asana.project-id).
// The exceptions are profiles, which only the global file lists, and name and repos, which only a profile's own config holds.
//
// pr.auto-merge has a third, highest-precedence layer: a per-repo value in git's local config (devora.pr.auto-merge), shared by all worktrees of a clone.
// Callers pass it in through GetAutoMergeDefaultForRepo.
package config

import (
	"encoding/json"
	"errors"
	"log"
	"os"
	"path/filepath"
	"strings"
)

// Profile represents a Devora profile with its configuration.
type Profile struct {
	Name     string
	RootPath string
	Config   map[string]any
}

var activeProfile *Profile

func SetActiveProfile(p *Profile) {
	activeProfile = p
}

func GetActiveProfile() *Profile {
	return activeProfile
}

// --- Global config path and caching ---

var configPathValue = defaultConfigPath()

func defaultConfigPath() string {
	// DEVORA_CONFIG_PATH lets the host process (Devora's Tauri backend, or the acceptance-test harness) point debi at the same config file it uses.
	// Matches the core's override in devora-config's config.rs:global_config_path.
	if override := os.Getenv("DEVORA_CONFIG_PATH"); override != "" {
		return override
	}
	home, err := os.UserHomeDir()
	if err != nil {
		return ""
	}
	return filepath.Join(home, ".config", "devora", "config.json")
}

func ConfigPath() string {
	return configPathValue
}

func setConfigPathForTesting(path string) {
	configPathValue = path
	resetGlobalConfigCache()
}

func SetConfigPathForTesting(path string) {
	setConfigPathForTesting(path)
}

func resetForTesting() {
	activeProfile = nil
	resetGlobalConfigCache()
}

func ResetForTesting() {
	resetForTesting()
}

// Config is only accessed from the main goroutine, so the cache needs no lock.
var (
	cachedGlobalConfig map[string]any
	globalConfigLoaded bool
)

func resetGlobalConfigCache() {
	cachedGlobalConfig = nil
	globalConfigLoaded = false
}

func globalConfig() map[string]any {
	if globalConfigLoaded {
		return cachedGlobalConfig
	}
	cachedGlobalConfig = loadConfigFromDisk()
	globalConfigLoaded = true
	return cachedGlobalConfig
}

func loadConfigFromDisk() map[string]any {
	data, err := os.ReadFile(configPathValue)
	if err != nil {
		return map[string]any{}
	}
	var result map[string]any
	if err := json.Unmarshal(data, &result); err != nil {
		log.Printf("warning: failed to parse global config %s: %v", configPathValue, err)
		return map[string]any{}
	}
	return result
}

// --- Dot-path resolution ---

func resolvePath(config map[string]any, path string) (any, bool) {
	segments := strings.Split(path, ".")
	var current any = config
	for _, seg := range segments {
		m, ok := current.(map[string]any)
		if !ok {
			return nil, false
		}
		val, exists := m[seg]
		if !exists {
			return nil, false
		}
		current = val
	}
	return current, true
}

// setNestedBool sets (value non-nil) or deletes (value nil) a nested bool
// at the given dot-path segments. Prunes empty intermediate maps on delete.
func setNestedBool(cfg map[string]any, segments []string, value *bool) map[string]any {
	if len(segments) == 0 {
		return cfg
	}
	if len(segments) == 1 {
		if value == nil {
			delete(cfg, segments[0])
		} else {
			cfg[segments[0]] = *value
		}
		return cfg
	}
	head := segments[0]
	child, _ := cfg[head].(map[string]any)
	if child == nil {
		if value == nil {
			return cfg
		}
		child = map[string]any{}
	}
	child = setNestedBool(child, segments[1:], value)
	if len(child) == 0 {
		delete(cfg, head)
	} else {
		cfg[head] = child
	}
	return cfg
}

// --- Config resolution ---

// Get resolves a config key using the standard resolution chain:
// active profile first, then global config.
func Get(path string) (any, bool) {
	return get(path)
}

func get(path string) (any, bool) {
	if activeProfile != nil {
		if val, ok := resolvePath(activeProfile.Config, path); ok {
			return val, true
		}
	}
	return resolvePath(globalConfig(), path)
}

func getGlobal(path string) (any, bool) {
	return resolvePath(globalConfig(), path)
}

// --- Profile operations ---

func loadProfileConfig(rootPath string) (map[string]any, bool) {
	data, err := os.ReadFile(filepath.Join(rootPath, "config.json"))
	if err != nil {
		return nil, false
	}
	var result map[string]any
	if err := json.Unmarshal(data, &result); err != nil {
		return nil, false
	}
	if _, hasName := result["name"]; !hasName {
		return nil, false
	}
	return result, true
}

// GetProfiles loads the profiles listed in the global config.
// Unloadable profiles are logged and skipped rather than returned as errors, since the remaining profiles are still usable.
func GetProfiles() []Profile {
	cfg := globalConfig()
	profilesRaw, ok := cfg["profiles"]
	if !ok {
		return []Profile{}
	}
	profilesList, ok := profilesRaw.([]any)
	if !ok {
		return []Profile{}
	}

	var profiles []Profile
	for _, pathRaw := range profilesList {
		pathStr, ok := pathRaw.(string)
		if !ok {
			log.Printf("warning: non-string entry in profiles list: %v", pathRaw)
			continue
		}
		profileCfg, valid := loadProfileConfig(pathStr)
		if !valid {
			log.Printf("warning: skipping invalid or incomplete profile at %s", pathStr)
			continue
		}
		name, _ := profileCfg["name"].(string)
		profiles = append(profiles, Profile{
			Name:     name,
			RootPath: pathStr,
			Config:   profileCfg,
		})
	}
	return profiles
}

// loadFreshGlobalConfig reads the global config directly from disk, bypassing the cache.
func loadFreshGlobalConfig() map[string]any {
	data, err := os.ReadFile(configPathValue)
	if err != nil {
		return map[string]any{}
	}
	var cfg map[string]any
	if err := json.Unmarshal(data, &cfg); err != nil {
		return map[string]any{}
	}
	return cfg
}

// writeFreshGlobalConfig writes the global config to disk and resets the cache.
func writeFreshGlobalConfig(cfg map[string]any) error {
	if err := os.MkdirAll(filepath.Dir(configPathValue), 0o755); err != nil {
		return err
	}
	b, err := json.MarshalIndent(cfg, "", "    ")
	if err != nil {
		return err
	}
	if err := os.WriteFile(configPathValue, b, 0o644); err != nil {
		return err
	}
	resetGlobalConfigCache()
	return nil
}

// --- Config mutation ---

func updateProfileConfig(p *Profile, mutateFn func(map[string]any) map[string]any) error {
	configFile := filepath.Join(p.RootPath, "config.json")
	data, err := os.ReadFile(configFile)
	if err != nil {
		return err
	}
	var cfg map[string]any
	if err := json.Unmarshal(data, &cfg); err != nil {
		return err
	}

	cfg = mutateFn(cfg)

	b, err := json.MarshalIndent(cfg, "", "    ")
	if err != nil {
		return err
	}
	if err := os.WriteFile(configFile, b, 0o644); err != nil {
		return err
	}

	// Update active profile's in-memory config if this is the active profile
	if activeProfile != nil && activeProfile.RootPath == p.RootPath {
		activeProfile = &Profile{
			Name:     p.Name,
			RootPath: p.RootPath,
			Config:   cfg,
		}
	}

	return nil
}

// SetPrAutoMergeGlobal writes "pr.auto-merge" to the global config. A non-nil
// value sets the key; a nil value clears it and prunes the "pr" map when it
// becomes empty.
func SetPrAutoMergeGlobal(value *bool) error {
	cfg := loadFreshGlobalConfig()
	cfg = setNestedBool(cfg, []string{"pr", "auto-merge"}, value)
	return writeFreshGlobalConfig(cfg)
}

// SetPrAutoMergeProfile writes "pr.auto-merge" to the active profile's
// config. A non-nil value sets the key; a nil value clears it and prunes
// the "pr" map when it becomes empty. Returns an error if no profile is
// active.
func SetPrAutoMergeProfile(value *bool) error {
	if activeProfile == nil {
		return errors.New("no active profile set")
	}
	return updateProfileConfig(activeProfile, func(cfg map[string]any) map[string]any {
		return setNestedBool(cfg, []string{"pr", "auto-merge"}, value)
	})
}

// --- Workspace root ---

func WorkspacesRootForProfile(p *Profile) string {
	return filepath.Join(p.RootPath, "workspaces")
}

// --- Convenience getters ---

// GetTaskTrackerProvider returns the value of "task-tracker.provider"
// (profile-overridable). Returns "" when unset or when the stored value is
// not a string.
func GetTaskTrackerProvider() string {
	val, ok := get("task-tracker.provider")
	if !ok {
		return ""
	}
	s, ok := val.(string)
	if !ok {
		return ""
	}
	return s
}

// GetTaskTrackerString resolves "task-tracker.<provider>.<key>" via the
// standard profile-first / global-fallback resolver. Returns "" when the
// leaf is unset at both levels or the value is not a string.
func GetTaskTrackerString(provider, key string) string {
	val, ok := get("task-tracker." + provider + "." + key)
	if !ok {
		return ""
	}
	s, ok := val.(string)
	if !ok {
		return ""
	}
	return s
}

// GetBranchPrefix resolves "feature.branch-prefix" (profile-overridable)
// with a fallback returned when the key is unset or not a string.
func GetBranchPrefix(fallback string) string {
	val, ok := get("feature.branch-prefix")
	if !ok {
		return fallback
	}
	s, ok := val.(string)
	if !ok {
		return fallback
	}
	return s
}

// GetPrTitlePrefix resolves "pr.title-prefix" (profile-overridable).
// Returns "" (no prefix) when the key is unset or not a string.
func GetPrTitlePrefix() string {
	val, ok := get("pr.title-prefix")
	if !ok {
		return ""
	}
	s, ok := val.(string)
	if !ok {
		return ""
	}
	return s
}

// GetAutoMergeDefault resolves "pr.auto-merge" (profile-overridable) with the
// given fallback when the key is unset or not a bool.
func GetAutoMergeDefault(fallback bool) bool {
	val, ok := get("pr.auto-merge")
	if !ok {
		return fallback
	}
	b, ok := val.(bool)
	if !ok {
		return fallback
	}
	return b
}

// GetPrAutoMergeGlobalRaw returns a pointer to the value of "pr.auto-merge"
// stored in the global config, or nil if the key is unset or not a bool.
// Reads only from the global config -- does not fall back to any other
// scope. Intended for UI and diagnostic code that needs to distinguish
// "unset at this scope" from "set to false".
func GetPrAutoMergeGlobalRaw() *bool {
	val, ok := getGlobal("pr.auto-merge")
	if !ok {
		return nil
	}
	b, ok := val.(bool)
	if !ok {
		return nil
	}
	return &b
}

// GetPrAutoMergeProfileRaw returns a pointer to the value of "pr.auto-merge"
// stored in the active profile's config, or nil if there is no active
// profile, the key is unset at the profile scope, or the value is not a
// bool. Reads only from the active profile -- does not fall back to global.
func GetPrAutoMergeProfileRaw() *bool {
	if activeProfile == nil {
		return nil
	}
	val, ok := resolvePath(activeProfile.Config, "pr.auto-merge")
	if !ok {
		return nil
	}
	b, ok := val.(bool)
	if !ok {
		return nil
	}
	return &b
}

// GetAutoMergeDefaultForRepo resolves "pr.auto-merge" with per-repo > profile
// > global > fallback precedence. The getRepo callback is expected to return
// (*bool, error): a non-nil pointer wins outright. The callback pattern keeps
// this package free of dependencies on internal/git.
//
// If getRepo returns an error, a warning is written to stderr via the
// package-level log; resolution then falls through to GetAutoMergeDefault.
func GetAutoMergeDefaultForRepo(fallback bool, getRepo func() (*bool, error)) bool {
	if getRepo != nil {
		val, err := getRepo()
		if err != nil {
			log.Printf("warning: failed to read per-repo pr.auto-merge: %v", err)
		} else if val != nil {
			return *val
		}
	}
	return GetAutoMergeDefault(fallback)
}

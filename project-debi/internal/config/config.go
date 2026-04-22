package config

import (
	"encoding/json"
	"errors"
	"log"
	"os"
	"path/filepath"
	"sort"
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

// setNestedString sets (value non-nil) or deletes (value nil) a nested string
// at the given dot-path segments. Prunes empty intermediate maps on delete.
func setNestedString(cfg map[string]any, segments []string, value *string) map[string]any {
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
	child = setNestedString(child, segments[1:], value)
	if len(child) == 0 {
		delete(cfg, head)
	} else {
		cfg[head] = child
	}
	return cfg
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

func RegisterProfile(rootPath string, name string) (Profile, error) {
	absRootPath, err := filepath.Abs(rootPath)
	if err != nil {
		return Profile{}, err
	}

	var profileCfg map[string]any

	// Try to load existing profile config
	existingCfg, valid := loadProfileConfig(absRootPath)
	if valid {
		profileCfg = existingCfg
	} else {
		// Create directory structure
		for _, sub := range []string{"", "repos", "workspaces"} {
			dir := filepath.Join(absRootPath, sub)
			if err := os.MkdirAll(dir, 0o755); err != nil {
				return Profile{}, err
			}
		}

		// Write new config.json
		profileCfg = map[string]any{"name": name}
		b, err := json.MarshalIndent(profileCfg, "", "    ")
		if err != nil {
			return Profile{}, err
		}
		if err := os.WriteFile(filepath.Join(absRootPath, "config.json"), b, 0o644); err != nil {
			return Profile{}, err
		}
	}

	globalCfg := loadFreshGlobalConfig()

	// Update profiles list
	var profilePaths []any
	if existingList, ok := globalCfg["profiles"].([]any); ok {
		profilePaths = existingList
	}

	alreadyPresent := false
	for _, p := range profilePaths {
		if p == absRootPath {
			alreadyPresent = true
			break
		}
	}
	if !alreadyPresent {
		profilePaths = append(profilePaths, absRootPath)
	}
	globalCfg["profiles"] = profilePaths

	if err := writeFreshGlobalConfig(globalCfg); err != nil {
		return Profile{}, err
	}

	profileName, _ := profileCfg["name"].(string)
	return Profile{
		Name:     profileName,
		RootPath: absRootPath,
		Config:   profileCfg,
	}, nil
}

func UnregisterProfile(rootPath string) error {
	absRootPath, err := filepath.Abs(rootPath)
	if err != nil {
		return err
	}

	globalCfg := loadFreshGlobalConfig()

	// Remove from profiles list
	var profilePaths []any
	if existingList, ok := globalCfg["profiles"].([]any); ok {
		for _, p := range existingList {
			if p != absRootPath {
				profilePaths = append(profilePaths, p)
			}
		}
	}
	if profilePaths == nil {
		profilePaths = []any{}
	}
	globalCfg["profiles"] = profilePaths

	if err := writeFreshGlobalConfig(globalCfg); err != nil {
		return err
	}

	// Clear active profile if it was the removed one
	if activeProfile != nil && activeProfile.RootPath == absRootPath {
		activeProfile = nil
	}

	return nil
}

func IsInitializedProfile(rootPath string) bool {
	_, valid := loadProfileConfig(rootPath)
	return valid
}

// --- Repo operations ---

// configRepoPaths extracts the string paths from a config map's "repos" key.
func configRepoPaths(cfg map[string]any) []string {
	reposRaw, ok := cfg["repos"]
	if !ok {
		return nil
	}
	reposList, ok := reposRaw.([]any)
	if !ok {
		return nil
	}
	var paths []string
	for _, r := range reposList {
		if s, ok := r.(string); ok {
			paths = append(paths, s)
		}
	}
	return paths
}

func autoDiscoverRepos(reposDir string) map[string]string {
	result := map[string]string{}
	entries, err := os.ReadDir(reposDir)
	if err != nil {
		return result
	}
	for _, entry := range entries {
		if !entry.IsDir() {
			continue
		}
		gitDir := filepath.Join(reposDir, entry.Name(), ".git")
		if info, err := os.Stat(gitDir); err == nil && info.IsDir() {
			result[entry.Name()] = filepath.Join(reposDir, entry.Name())
		}
	}
	return result
}

func explicitRepos(paths []string) map[string]string {
	result := map[string]string{}
	for _, p := range paths {
		expanded := ExpandTilde(p)
		absPath, err := filepath.Abs(expanded)
		if err != nil {
			continue
		}
		if _, err := os.Stat(absPath); err != nil {
			continue
		}
		baseName := filepath.Base(absPath)
		result[baseName] = absPath
	}
	return result
}

func ExpandTilde(path string) string {
	if path == "~" || strings.HasPrefix(path, "~/") {
		home, err := os.UserHomeDir()
		if err != nil {
			return path
		}
		return filepath.Join(home, path[1:])
	}
	return path
}

func GetRegisteredRepos() ([]string, error) {
	if activeProfile == nil {
		return nil, errors.New("no active profile set")
	}

	reposDir := filepath.Join(activeProfile.RootPath, "repos")

	// Start with explicit repos
	merged := explicitRepos(configRepoPaths(activeProfile.Config))

	// Overlay auto-discovered repos (auto takes precedence)
	autoRepos := autoDiscoverRepos(reposDir)
	for name, path := range autoRepos {
		merged[name] = path
	}

	var result []string
	for _, path := range merged {
		result = append(result, path)
	}
	return result, nil
}

func GetRegisteredRepoNames() ([]string, error) {
	repos, err := GetRegisteredRepos()
	if err != nil {
		return nil, err
	}
	var names []string
	for _, r := range repos {
		names = append(names, filepath.Base(r))
	}
	sort.Strings(names)
	return names, nil
}

func RegisterRepo(path string) error {
	if activeProfile == nil {
		return errors.New("no active profile set")
	}

	absPath, err := filepath.Abs(path)
	if err != nil {
		return err
	}

	// Check if already present before calling updateProfileConfig
	for _, p := range configRepoPaths(activeProfile.Config) {
		if p == absPath {
			return nil // already registered
		}
	}

	return updateProfileConfig(activeProfile, func(cfg map[string]any) map[string]any {
		var repos []any
		if existing, ok := cfg["repos"].([]any); ok {
			repos = existing
		}
		repos = append(repos, absPath)
		cfg["repos"] = repos
		return cfg
	})
}

func UnregisterRepo(path string) error {
	if activeProfile == nil {
		return errors.New("no active profile set")
	}

	absPath, err := filepath.Abs(path)
	if err != nil {
		return err
	}

	// Check if the repo is in the list
	found := false
	for _, p := range configRepoPaths(activeProfile.Config) {
		if p == absPath {
			found = true
			break
		}
	}
	if !found {
		return errors.New("repo not found in config")
	}

	return updateProfileConfig(activeProfile, func(cfg map[string]any) map[string]any {
		var repos []any
		if existing, ok := cfg["repos"].([]any); ok {
			for _, r := range existing {
				if s, ok := r.(string); ok && s == absPath {
					continue
				}
				repos = append(repos, r)
			}
		}
		if repos == nil {
			repos = []any{}
		}
		cfg["repos"] = repos
		return cfg
	})
}

// ExplicitRepoEntry represents a repo explicitly listed in the profile config.
type ExplicitRepoEntry struct {
	Name string
	Path string
}

func GetExplicitRepoEntries() ([]ExplicitRepoEntry, error) {
	if activeProfile == nil {
		return nil, errors.New("no active profile set")
	}

	resolved := explicitRepos(configRepoPaths(activeProfile.Config))

	var entries []ExplicitRepoEntry
	for name, path := range resolved {
		entries = append(entries, ExplicitRepoEntry{Name: name, Path: path})
	}

	sort.Slice(entries, func(i, j int) bool {
		return entries[i].Name < entries[j].Name
	})

	return entries, nil
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

func SetPrepareCommand(value *string) error {
	if activeProfile == nil {
		return errors.New("no active profile set")
	}

	return updateProfileConfig(activeProfile, func(cfg map[string]any) map[string]any {
		if value == nil {
			delete(cfg, "prepare-command")
		} else {
			cfg["prepare-command"] = *value
		}
		return cfg
	})
}

func SetDefaultTerminalAppGlobal(value *string) error {
	cfg := loadFreshGlobalConfig()
	cfg = setNestedString(cfg, []string{"terminal", "default-app"}, value)
	return writeFreshGlobalConfig(cfg)
}

func SetDefaultTerminalAppProfile(value *string) error {
	if activeProfile == nil {
		return errors.New("no active profile set")
	}
	return updateProfileConfig(activeProfile, func(cfg map[string]any) map[string]any {
		return setNestedString(cfg, []string{"terminal", "default-app"}, value)
	})
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

func GetWorkspacesRootPath() (string, error) {
	if activeProfile == nil {
		return "", errors.New("no active profile set")
	}
	return WorkspacesRootForProfile(activeProfile), nil
}

// --- Convenience getters ---

func GetPrepareCommand() *string {
	val, ok := get("prepare-command")
	if !ok {
		return nil
	}
	s, ok := val.(string)
	if !ok {
		return nil
	}
	return &s
}

func GetDefaultTerminalApp(fallback string) string {
	val, ok := get("terminal.default-app")
	if !ok {
		return fallback
	}
	s, ok := val.(string)
	if !ok {
		return fallback
	}
	return s
}

func GetDefaultTerminalAppGlobalRaw() *string {
	val, ok := getGlobal("terminal.default-app")
	if !ok {
		return nil
	}
	s, ok := val.(string)
	if !ok {
		return nil
	}
	return &s
}

func GetDefaultTerminalAppProfileRaw() *string {
	if activeProfile == nil {
		return nil
	}
	val, ok := resolvePath(activeProfile.Config, "terminal.default-app")
	if !ok {
		return nil
	}
	s, ok := val.(string)
	if !ok {
		return nil
	}
	return &s
}

func TerminalSessionCreationTimeoutSeconds(fallback int) int {
	val, ok := getGlobal("terminal.session-creation-timeout-seconds")
	if !ok {
		return fallback
	}
	// JSON numbers are float64
	f, ok := val.(float64)
	if !ok {
		return fallback
	}
	return int(f)
}

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
//
// Each leaf is queried independently, so profile and global can contribute
// different leaves under the same provider (e.g., global sets workspace-id,
// profile sets project-id).
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

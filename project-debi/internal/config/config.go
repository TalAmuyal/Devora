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

	// Load fresh global config from disk (not from cache)
	globalData, err := os.ReadFile(configPathValue)
	var globalCfg map[string]any
	if err != nil {
		globalCfg = map[string]any{}
	} else {
		if err := json.Unmarshal(globalData, &globalCfg); err != nil {
			globalCfg = map[string]any{}
		}
	}

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

	// Write global config
	if err := os.MkdirAll(filepath.Dir(configPathValue), 0o755); err != nil {
		return Profile{}, err
	}
	b, err := json.MarshalIndent(globalCfg, "", "    ")
	if err != nil {
		return Profile{}, err
	}
	if err := os.WriteFile(configPathValue, b, 0o644); err != nil {
		return Profile{}, err
	}

	resetGlobalConfigCache()

	profileName, _ := profileCfg["name"].(string)
	return Profile{
		Name:     profileName,
		RootPath: absRootPath,
		Config:   profileCfg,
	}, nil
}

func IsInitializedProfile(rootPath string) bool {
	_, valid := loadProfileConfig(rootPath)
	return valid
}

// --- Repo operations ---

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
	merged := map[string]string{}
	if reposRaw, ok := activeProfile.Config["repos"]; ok {
		if reposList, ok := reposRaw.([]any); ok {
			var paths []string
			for _, r := range reposList {
				if s, ok := r.(string); ok {
					paths = append(paths, s)
				}
			}
			merged = explicitRepos(paths)
		}
	}

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
	if reposRaw, ok := activeProfile.Config["repos"]; ok {
		if reposList, ok := reposRaw.([]any); ok {
			for _, r := range reposList {
				if s, ok := r.(string); ok && s == absPath {
					return nil // already registered
				}
			}
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

package config

import (
	"encoding/json"
	"os"
	"path/filepath"
	"sort"
	"testing"
)

// helper: write JSON to a file with 4-space indentation
func writeJSON(t *testing.T, path string, data map[string]any) {
	t.Helper()
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatalf("failed to create dir for %s: %v", path, err)
	}
	b, err := json.MarshalIndent(data, "", "    ")
	if err != nil {
		t.Fatalf("failed to marshal JSON: %v", err)
	}
	if err := os.WriteFile(path, b, 0o644); err != nil {
		t.Fatalf("failed to write %s: %v", path, err)
	}
}

// helper: read JSON from a file
func readJSON(t *testing.T, path string) map[string]any {
	t.Helper()
	b, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("failed to read %s: %v", path, err)
	}
	var data map[string]any
	if err := json.Unmarshal(b, &data); err != nil {
		t.Fatalf("failed to parse %s: %v", path, err)
	}
	return data
}

// helper: set up a temp global config and reset state
func setupTest(t *testing.T) string {
	t.Helper()
	tmpDir := t.TempDir()
	globalConfigPath := filepath.Join(tmpDir, "config.json")
	setConfigPathForTesting(globalConfigPath)
	t.Cleanup(func() {
		resetForTesting()
	})
	return tmpDir
}

// helper: create a valid profile directory with config.json
func createProfile(t *testing.T, rootPath string, profileConfig map[string]any) {
	t.Helper()
	if err := os.MkdirAll(filepath.Join(rootPath, "repos"), 0o755); err != nil {
		t.Fatalf("failed to create profile dirs: %v", err)
	}
	if err := os.MkdirAll(filepath.Join(rootPath, "workspaces"), 0o755); err != nil {
		t.Fatalf("failed to create workspaces dir: %v", err)
	}
	writeJSON(t, filepath.Join(rootPath, "config.json"), profileConfig)
}

// --- Dot-path resolution tests ---

func TestResolvePath_SingleSegment(t *testing.T) {
	config := map[string]any{"name": "test-profile"}
	val, ok := resolvePath(config, "name")
	if !ok {
		t.Fatal("expected to find 'name'")
	}
	if val != "test-profile" {
		t.Fatalf("expected 'test-profile', got %v", val)
	}
}

func TestResolvePath_MultiSegment(t *testing.T) {
	config := map[string]any{
		"terminal": map[string]any{
			"default-app": "nvim",
		},
	}
	val, ok := resolvePath(config, "terminal.default-app")
	if !ok {
		t.Fatal("expected to find 'terminal.default-app'")
	}
	if val != "nvim" {
		t.Fatalf("expected 'nvim', got %v", val)
	}
}

func TestResolvePath_MissingIntermediateKey(t *testing.T) {
	config := map[string]any{"name": "test"}
	_, ok := resolvePath(config, "terminal.default-app")
	if ok {
		t.Fatal("expected not-found for missing intermediate key")
	}
}

func TestResolvePath_NonMapIntermediate(t *testing.T) {
	config := map[string]any{"a": "just-a-string"}
	_, ok := resolvePath(config, "a.b")
	if ok {
		t.Fatal("expected not-found when intermediate is not a map")
	}
}

// --- Config resolution chain tests ---

func TestGet_ValueInProfileOnly(t *testing.T) {
	tmpDir := setupTest(t)
	// No global config file written, so global config is empty
	profileDir := filepath.Join(tmpDir, "profile")
	createProfile(t, profileDir, map[string]any{
		"name":            "my-profile",
		"prepare-command": "make build",
	})

	p := Profile{Name: "my-profile", RootPath: profileDir, Config: map[string]any{
		"name":            "my-profile",
		"prepare-command": "make build",
	}}
	SetActiveProfile(&p)

	val, ok := get("prepare-command")
	if !ok {
		t.Fatal("expected to find 'prepare-command' in profile")
	}
	if val != "make build" {
		t.Fatalf("expected 'make build', got %v", val)
	}
}

func TestGet_ValueInGlobalOnly(t *testing.T) {
	tmpDir := setupTest(t)
	writeJSON(t, filepath.Join(tmpDir, "config.json"), map[string]any{
		"prepare-command": "global-prep",
	})
	resetGlobalConfigCache()

	val, ok := get("prepare-command")
	if !ok {
		t.Fatal("expected to find 'prepare-command' in global config")
	}
	if val != "global-prep" {
		t.Fatalf("expected 'global-prep', got %v", val)
	}
}

func TestGet_ProfileOverridesGlobal(t *testing.T) {
	tmpDir := setupTest(t)
	writeJSON(t, filepath.Join(tmpDir, "config.json"), map[string]any{
		"prepare-command": "global-prep",
	})
	resetGlobalConfigCache()

	p := Profile{Name: "test", RootPath: filepath.Join(tmpDir, "profile"), Config: map[string]any{
		"name":            "test",
		"prepare-command": "profile-prep",
	}}
	SetActiveProfile(&p)

	val, ok := get("prepare-command")
	if !ok {
		t.Fatal("expected to find 'prepare-command'")
	}
	if val != "profile-prep" {
		t.Fatalf("expected 'profile-prep', got %v", val)
	}
}

func TestGet_ValueInNeither(t *testing.T) {
	setupTest(t)
	_, ok := get("nonexistent-key")
	if ok {
		t.Fatal("expected not-found for key in neither profile nor global")
	}
}

func TestGetGlobal_IgnoresProfileConfig(t *testing.T) {
	tmpDir := setupTest(t)
	writeJSON(t, filepath.Join(tmpDir, "config.json"), map[string]any{
		"terminal": map[string]any{
			"session-creation-timeout-seconds": float64(5),
		},
	})
	resetGlobalConfigCache()

	p := Profile{Name: "test", RootPath: filepath.Join(tmpDir, "profile"), Config: map[string]any{
		"name": "test",
		"terminal": map[string]any{
			"session-creation-timeout-seconds": float64(99),
		},
	}}
	SetActiveProfile(&p)

	val, ok := getGlobal("terminal.session-creation-timeout-seconds")
	if !ok {
		t.Fatal("expected to find 'terminal.session-creation-timeout-seconds' in global")
	}
	if val != float64(5) {
		t.Fatalf("expected 5, got %v", val)
	}
}

// --- Global config loading tests ---

func TestGlobalConfig_FileNotExists_ReturnsEmptyMap(t *testing.T) {
	setupTest(t)
	// No config file written
	cfg := globalConfig()
	if len(cfg) != 0 {
		t.Fatalf("expected empty map, got %v", cfg)
	}
}

func TestGlobalConfig_InvalidJSON_ReturnsEmptyMap(t *testing.T) {
	tmpDir := setupTest(t)
	configPath := filepath.Join(tmpDir, "config.json")
	if err := os.MkdirAll(filepath.Dir(configPath), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(configPath, []byte("not valid json{{{"), 0o644); err != nil {
		t.Fatal(err)
	}
	resetGlobalConfigCache()

	cfg := globalConfig()
	if len(cfg) != 0 {
		t.Fatalf("expected empty map for invalid JSON, got %v", cfg)
	}
}

func TestGlobalConfig_IsCached(t *testing.T) {
	tmpDir := setupTest(t)
	writeJSON(t, filepath.Join(tmpDir, "config.json"), map[string]any{
		"prepare-command": "first",
	})
	resetGlobalConfigCache()

	cfg1 := globalConfig()
	if cfg1["prepare-command"] != "first" {
		t.Fatal("expected 'first'")
	}

	// Overwrite the file
	writeJSON(t, filepath.Join(tmpDir, "config.json"), map[string]any{
		"prepare-command": "second",
	})

	// Should still return cached value
	cfg2 := globalConfig()
	if cfg2["prepare-command"] != "first" {
		t.Fatal("expected cached 'first', got updated value without cache reset")
	}
}

func TestGlobalConfig_CacheResetReloads(t *testing.T) {
	tmpDir := setupTest(t)
	writeJSON(t, filepath.Join(tmpDir, "config.json"), map[string]any{
		"prepare-command": "first",
	})
	resetGlobalConfigCache()

	cfg1 := globalConfig()
	if cfg1["prepare-command"] != "first" {
		t.Fatal("expected 'first'")
	}

	writeJSON(t, filepath.Join(tmpDir, "config.json"), map[string]any{
		"prepare-command": "second",
	})
	resetGlobalConfigCache()

	cfg2 := globalConfig()
	if cfg2["prepare-command"] != "second" {
		t.Fatalf("expected 'second' after cache reset, got %v", cfg2["prepare-command"])
	}
}

// --- Profile loading tests ---

func TestGetProfiles_ValidProfiles(t *testing.T) {
	tmpDir := setupTest(t)
	profile1Dir := filepath.Join(tmpDir, "profile1")
	profile2Dir := filepath.Join(tmpDir, "profile2")

	createProfile(t, profile1Dir, map[string]any{"name": "alpha"})
	createProfile(t, profile2Dir, map[string]any{"name": "beta"})

	writeJSON(t, filepath.Join(tmpDir, "config.json"), map[string]any{
		"profiles": []any{profile1Dir, profile2Dir},
	})
	resetGlobalConfigCache()

	profiles := GetProfiles()
	if len(profiles) != 2 {
		t.Fatalf("expected 2 profiles, got %d", len(profiles))
	}

	names := []string{profiles[0].Name, profiles[1].Name}
	sort.Strings(names)
	if names[0] != "alpha" || names[1] != "beta" {
		t.Fatalf("unexpected profile names: %v", names)
	}
}

func TestGetProfiles_MissingConfigJSON_Skipped(t *testing.T) {
	tmpDir := setupTest(t)
	profileDir := filepath.Join(tmpDir, "profile-no-config")
	if err := os.MkdirAll(profileDir, 0o755); err != nil {
		t.Fatal(err)
	}
	// No config.json in this directory

	writeJSON(t, filepath.Join(tmpDir, "config.json"), map[string]any{
		"profiles": []any{profileDir},
	})
	resetGlobalConfigCache()

	profiles := GetProfiles()
	if len(profiles) != 0 {
		t.Fatalf("expected 0 profiles (missing config.json), got %d", len(profiles))
	}
}

func TestGetProfiles_InvalidJSON_Skipped(t *testing.T) {
	tmpDir := setupTest(t)
	profileDir := filepath.Join(tmpDir, "profile-bad-json")
	if err := os.MkdirAll(profileDir, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(profileDir, "config.json"), []byte("{bad json"), 0o644); err != nil {
		t.Fatal(err)
	}

	writeJSON(t, filepath.Join(tmpDir, "config.json"), map[string]any{
		"profiles": []any{profileDir},
	})
	resetGlobalConfigCache()

	profiles := GetProfiles()
	if len(profiles) != 0 {
		t.Fatalf("expected 0 profiles (invalid JSON), got %d", len(profiles))
	}
}

func TestGetProfiles_MissingNameKey_Skipped(t *testing.T) {
	tmpDir := setupTest(t)
	profileDir := filepath.Join(tmpDir, "profile-no-name")
	if err := os.MkdirAll(profileDir, 0o755); err != nil {
		t.Fatal(err)
	}
	writeJSON(t, filepath.Join(profileDir, "config.json"), map[string]any{
		"some-key": "some-value",
	})

	writeJSON(t, filepath.Join(tmpDir, "config.json"), map[string]any{
		"profiles": []any{profileDir},
	})
	resetGlobalConfigCache()

	profiles := GetProfiles()
	if len(profiles) != 0 {
		t.Fatalf("expected 0 profiles (missing name key), got %d", len(profiles))
	}
}

func TestGetProfiles_EmptyProfilesList(t *testing.T) {
	tmpDir := setupTest(t)
	writeJSON(t, filepath.Join(tmpDir, "config.json"), map[string]any{
		"profiles": []any{},
	})
	resetGlobalConfigCache()

	profiles := GetProfiles()
	if len(profiles) != 0 {
		t.Fatalf("expected 0 profiles for empty list, got %d", len(profiles))
	}
}

func TestGetProfiles_NoProfilesKey(t *testing.T) {
	tmpDir := setupTest(t)
	writeJSON(t, filepath.Join(tmpDir, "config.json"), map[string]any{
		"other-key": "value",
	})
	resetGlobalConfigCache()

	profiles := GetProfiles()
	if len(profiles) != 0 {
		t.Fatalf("expected 0 profiles when key absent, got %d", len(profiles))
	}
}

// --- Profile registration tests ---

func TestRegisterProfile_NewProfile(t *testing.T) {
	tmpDir := setupTest(t)
	profileDir := filepath.Join(tmpDir, "new-profile")

	profile, err := RegisterProfile(profileDir, "my-new-profile")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if profile.Name != "my-new-profile" {
		t.Fatalf("expected name 'my-new-profile', got %q", profile.Name)
	}
	if profile.RootPath != profileDir {
		t.Fatalf("expected rootPath %q, got %q", profileDir, profile.RootPath)
	}

	// Verify directories were created
	for _, sub := range []string{"repos", "workspaces"} {
		info, err := os.Stat(filepath.Join(profileDir, sub))
		if err != nil {
			t.Fatalf("expected %s directory to exist: %v", sub, err)
		}
		if !info.IsDir() {
			t.Fatalf("expected %s to be a directory", sub)
		}
	}

	// Verify config.json was written
	cfg := readJSON(t, filepath.Join(profileDir, "config.json"))
	if cfg["name"] != "my-new-profile" {
		t.Fatalf("expected name in config.json, got %v", cfg["name"])
	}

	// Verify global config was updated
	globalCfg := readJSON(t, filepath.Join(tmpDir, "config.json"))
	profilesList, ok := globalCfg["profiles"].([]any)
	if !ok {
		t.Fatalf("expected profiles list in global config, got %T", globalCfg["profiles"])
	}

	absProfileDir, _ := filepath.Abs(profileDir)
	found := false
	for _, p := range profilesList {
		if p == absProfileDir {
			found = true
			break
		}
	}
	if !found {
		t.Fatalf("expected profile path %q in global profiles list, got %v", absProfileDir, profilesList)
	}
}

func TestRegisterProfile_ExistingInitializedProfile(t *testing.T) {
	tmpDir := setupTest(t)
	profileDir := filepath.Join(tmpDir, "existing-profile")
	createProfile(t, profileDir, map[string]any{
		"name":            "existing-name",
		"prepare-command": "existing-cmd",
	})

	profile, err := RegisterProfile(profileDir, "ignored-name")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	// Name should come from existing config, not the parameter
	if profile.Name != "existing-name" {
		t.Fatalf("expected 'existing-name', got %q", profile.Name)
	}

	// Existing config should not be overwritten
	cfg := readJSON(t, filepath.Join(profileDir, "config.json"))
	if cfg["prepare-command"] != "existing-cmd" {
		t.Fatalf("expected existing config to be preserved, got %v", cfg)
	}
}

func TestRegisterProfile_DuplicateRegistration(t *testing.T) {
	tmpDir := setupTest(t)
	profileDir := filepath.Join(tmpDir, "dup-profile")

	_, err := RegisterProfile(profileDir, "first")
	if err != nil {
		t.Fatalf("first registration failed: %v", err)
	}

	_, err = RegisterProfile(profileDir, "first")
	if err != nil {
		t.Fatalf("second registration failed: %v", err)
	}

	// Verify no duplicate in global profiles list
	globalCfg := readJSON(t, filepath.Join(tmpDir, "config.json"))
	profilesList := globalCfg["profiles"].([]any)
	absProfileDir, _ := filepath.Abs(profileDir)
	count := 0
	for _, p := range profilesList {
		if p == absProfileDir {
			count++
		}
	}
	if count != 1 {
		t.Fatalf("expected exactly 1 entry for profile, got %d", count)
	}
}

func TestRegisterProfile_GlobalConfigDirCreated(t *testing.T) {
	tmpDir := setupTest(t)
	// Point global config to a path whose parent doesn't exist yet
	deepPath := filepath.Join(tmpDir, "deep", "nested", "config.json")
	setConfigPathForTesting(deepPath)

	profileDir := filepath.Join(tmpDir, "profile")
	_, err := RegisterProfile(profileDir, "test")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	// The global config directory should have been created
	if _, err := os.Stat(filepath.Dir(deepPath)); err != nil {
		t.Fatalf("expected global config dir to be created: %v", err)
	}
}

func TestRegisterProfile_CacheResetAfterRegistration(t *testing.T) {
	tmpDir := setupTest(t)
	profileDir := filepath.Join(tmpDir, "profile")

	// Access global config before registration (caches empty)
	cfg := globalConfig()
	if len(cfg) != 0 {
		t.Fatal("expected empty global config before registration")
	}

	_, err := RegisterProfile(profileDir, "test")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	// After registration, cache should be reset and contain the profile
	cfg = globalConfig()
	if cfg["profiles"] == nil {
		t.Fatal("expected global config to have profiles after registration and cache reset")
	}
}

// --- IsInitializedProfile tests ---

func TestIsInitializedProfile_Valid(t *testing.T) {
	tmpDir := setupTest(t)
	profileDir := filepath.Join(tmpDir, "profile")
	createProfile(t, profileDir, map[string]any{"name": "test"})

	if !IsInitializedProfile(profileDir) {
		t.Fatal("expected profile to be initialized")
	}
}

func TestIsInitializedProfile_MissingConfig(t *testing.T) {
	tmpDir := setupTest(t)
	profileDir := filepath.Join(tmpDir, "no-profile")
	if err := os.MkdirAll(profileDir, 0o755); err != nil {
		t.Fatal(err)
	}

	if IsInitializedProfile(profileDir) {
		t.Fatal("expected profile to not be initialized (no config.json)")
	}
}

func TestIsInitializedProfile_InvalidJSON(t *testing.T) {
	tmpDir := setupTest(t)
	profileDir := filepath.Join(tmpDir, "bad-profile")
	if err := os.MkdirAll(profileDir, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(profileDir, "config.json"), []byte("{bad"), 0o644); err != nil {
		t.Fatal(err)
	}

	if IsInitializedProfile(profileDir) {
		t.Fatal("expected profile to not be initialized (invalid JSON)")
	}
}

func TestIsInitializedProfile_MissingNameKey(t *testing.T) {
	tmpDir := setupTest(t)
	profileDir := filepath.Join(tmpDir, "no-name-profile")
	if err := os.MkdirAll(profileDir, 0o755); err != nil {
		t.Fatal(err)
	}
	writeJSON(t, filepath.Join(profileDir, "config.json"), map[string]any{
		"other": "value",
	})

	if IsInitializedProfile(profileDir) {
		t.Fatal("expected profile to not be initialized (missing name)")
	}
}

// --- Active profile tests ---

func TestActiveProfile_SetAndGet(t *testing.T) {
	setupTest(t)

	if GetActiveProfile() != nil {
		t.Fatal("expected nil active profile initially")
	}

	p := &Profile{Name: "test", RootPath: "/tmp/test", Config: map[string]any{"name": "test"}}
	SetActiveProfile(p)

	got := GetActiveProfile()
	if got == nil || got.Name != "test" {
		t.Fatal("expected active profile to be set")
	}

	SetActiveProfile(nil)
	if GetActiveProfile() != nil {
		t.Fatal("expected nil active profile after clearing")
	}
}

// --- Repo discovery tests ---

func TestGetRegisteredRepos_NoActiveProfile(t *testing.T) {
	setupTest(t)
	_, err := GetRegisteredRepos()
	if err == nil {
		t.Fatal("expected error when no active profile")
	}
	if err.Error() != "no active profile set" {
		t.Fatalf("unexpected error message: %v", err)
	}
}

func TestGetRegisteredRepos_AutoDiscovered(t *testing.T) {
	tmpDir := setupTest(t)
	profileDir := filepath.Join(tmpDir, "profile")
	createProfile(t, profileDir, map[string]any{"name": "test"})

	// Create repos with .git directories
	repo1 := filepath.Join(profileDir, "repos", "repo-a")
	repo2 := filepath.Join(profileDir, "repos", "repo-b")
	notARepo := filepath.Join(profileDir, "repos", "not-a-repo")

	for _, dir := range []string{
		filepath.Join(repo1, ".git"),
		filepath.Join(repo2, ".git"),
		notARepo, // no .git
	} {
		if err := os.MkdirAll(dir, 0o755); err != nil {
			t.Fatal(err)
		}
	}

	p := Profile{Name: "test", RootPath: profileDir, Config: map[string]any{"name": "test"}}
	SetActiveProfile(&p)

	repos, err := GetRegisteredRepos()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if len(repos) != 2 {
		t.Fatalf("expected 2 repos, got %d: %v", len(repos), repos)
	}

	// Check both repos are present
	repoSet := map[string]bool{}
	for _, r := range repos {
		repoSet[r] = true
	}
	if !repoSet[repo1] || !repoSet[repo2] {
		t.Fatalf("expected repo-a and repo-b, got %v", repos)
	}
}

func TestGetRegisteredRepos_ExplicitRepos(t *testing.T) {
	tmpDir := setupTest(t)
	profileDir := filepath.Join(tmpDir, "profile")

	// Create an explicit repo directory on disk
	explicitRepo := filepath.Join(tmpDir, "external-repo")
	if err := os.MkdirAll(explicitRepo, 0o755); err != nil {
		t.Fatal(err)
	}

	createProfile(t, profileDir, map[string]any{
		"name":  "test",
		"repos": []any{explicitRepo},
	})

	p := Profile{Name: "test", RootPath: profileDir, Config: map[string]any{
		"name":  "test",
		"repos": []any{explicitRepo},
	}}
	SetActiveProfile(&p)

	repos, err := GetRegisteredRepos()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if len(repos) != 1 {
		t.Fatalf("expected 1 repo, got %d: %v", len(repos), repos)
	}

	absExplicit, _ := filepath.Abs(explicitRepo)
	if repos[0] != absExplicit {
		t.Fatalf("expected %q, got %q", absExplicit, repos[0])
	}
}

func TestGetRegisteredRepos_ExplicitRepoNotOnDisk_Excluded(t *testing.T) {
	tmpDir := setupTest(t)
	profileDir := filepath.Join(tmpDir, "profile")

	createProfile(t, profileDir, map[string]any{
		"name":  "test",
		"repos": []any{"/nonexistent/path/repo"},
	})

	p := Profile{Name: "test", RootPath: profileDir, Config: map[string]any{
		"name":  "test",
		"repos": []any{"/nonexistent/path/repo"},
	}}
	SetActiveProfile(&p)

	repos, err := GetRegisteredRepos()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if len(repos) != 0 {
		t.Fatalf("expected 0 repos (nonexistent), got %d: %v", len(repos), repos)
	}
}

func TestGetRegisteredRepos_NameCollision_AutoDiscoveredWins(t *testing.T) {
	tmpDir := setupTest(t)
	profileDir := filepath.Join(tmpDir, "profile")

	// Auto-discovered repo
	autoRepo := filepath.Join(profileDir, "repos", "my-repo")
	if err := os.MkdirAll(filepath.Join(autoRepo, ".git"), 0o755); err != nil {
		t.Fatal(err)
	}

	// Explicit repo with the same base name in a different location
	explicitRepo := filepath.Join(tmpDir, "external", "my-repo")
	if err := os.MkdirAll(explicitRepo, 0o755); err != nil {
		t.Fatal(err)
	}

	createProfile(t, profileDir, map[string]any{
		"name":  "test",
		"repos": []any{explicitRepo},
	})

	p := Profile{Name: "test", RootPath: profileDir, Config: map[string]any{
		"name":  "test",
		"repos": []any{explicitRepo},
	}}
	SetActiveProfile(&p)

	repos, err := GetRegisteredRepos()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if len(repos) != 1 {
		t.Fatalf("expected 1 repo (collision resolved), got %d: %v", len(repos), repos)
	}

	// Auto-discovered should win
	if repos[0] != autoRepo {
		t.Fatalf("expected auto-discovered %q to win, got %q", autoRepo, repos[0])
	}
}

// --- GetRegisteredRepoNames tests ---

func TestGetRegisteredRepoNames_NoActiveProfile(t *testing.T) {
	setupTest(t)
	_, err := GetRegisteredRepoNames()
	if err == nil {
		t.Fatal("expected error when no active profile")
	}
}

func TestGetRegisteredRepoNames_SortedAlphabetically(t *testing.T) {
	tmpDir := setupTest(t)
	profileDir := filepath.Join(tmpDir, "profile")
	createProfile(t, profileDir, map[string]any{"name": "test"})

	for _, name := range []string{"zebra", "alpha", "middle"} {
		if err := os.MkdirAll(filepath.Join(profileDir, "repos", name, ".git"), 0o755); err != nil {
			t.Fatal(err)
		}
	}

	p := Profile{Name: "test", RootPath: profileDir, Config: map[string]any{"name": "test"}}
	SetActiveProfile(&p)

	names, err := GetRegisteredRepoNames()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	expected := []string{"alpha", "middle", "zebra"}
	if len(names) != len(expected) {
		t.Fatalf("expected %v, got %v", expected, names)
	}
	for i, name := range names {
		if name != expected[i] {
			t.Fatalf("expected %v, got %v", expected, names)
		}
	}
}

// --- Repo registration tests ---

func TestRegisterRepo_NoActiveProfile(t *testing.T) {
	setupTest(t)
	err := RegisterRepo("/some/path")
	if err == nil {
		t.Fatal("expected error when no active profile")
	}
}

func TestRegisterRepo_AppendsToConfig(t *testing.T) {
	tmpDir := setupTest(t)
	profileDir := filepath.Join(tmpDir, "profile")
	createProfile(t, profileDir, map[string]any{"name": "test"})

	p := Profile{Name: "test", RootPath: profileDir, Config: map[string]any{"name": "test"}}
	SetActiveProfile(&p)

	repoPath := filepath.Join(tmpDir, "my-repo")
	if err := os.MkdirAll(repoPath, 0o755); err != nil {
		t.Fatal(err)
	}

	err := RegisterRepo(repoPath)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	// Verify the config.json was updated
	cfg := readJSON(t, filepath.Join(profileDir, "config.json"))
	repos, ok := cfg["repos"].([]any)
	if !ok {
		t.Fatalf("expected repos list, got %T", cfg["repos"])
	}

	absPath, _ := filepath.Abs(repoPath)
	found := false
	for _, r := range repos {
		if r == absPath {
			found = true
			break
		}
	}
	if !found {
		t.Fatalf("expected repo path %q in repos list, got %v", absPath, repos)
	}
}

func TestRegisterRepo_DuplicateIsNoOp(t *testing.T) {
	tmpDir := setupTest(t)
	profileDir := filepath.Join(tmpDir, "profile")
	createProfile(t, profileDir, map[string]any{"name": "test"})

	p := Profile{Name: "test", RootPath: profileDir, Config: map[string]any{"name": "test"}}
	SetActiveProfile(&p)

	repoPath := filepath.Join(tmpDir, "my-repo")
	if err := os.MkdirAll(repoPath, 0o755); err != nil {
		t.Fatal(err)
	}

	if err := RegisterRepo(repoPath); err != nil {
		t.Fatal(err)
	}
	if err := RegisterRepo(repoPath); err != nil {
		t.Fatal(err)
	}

	cfg := readJSON(t, filepath.Join(profileDir, "config.json"))
	repos := cfg["repos"].([]any)

	absPath, _ := filepath.Abs(repoPath)
	count := 0
	for _, r := range repos {
		if r == absPath {
			count++
		}
	}
	if count != 1 {
		t.Fatalf("expected exactly 1 entry for repo, got %d", count)
	}
}

// --- Config mutation tests ---

func TestSetPrepareCommand_NoActiveProfile(t *testing.T) {
	setupTest(t)
	val := "test"
	err := SetPrepareCommand(&val)
	if err == nil {
		t.Fatal("expected error when no active profile")
	}
}

func TestSetPrepareCommand_SetValue(t *testing.T) {
	tmpDir := setupTest(t)
	profileDir := filepath.Join(tmpDir, "profile")
	createProfile(t, profileDir, map[string]any{"name": "test"})

	p := Profile{Name: "test", RootPath: profileDir, Config: map[string]any{"name": "test"}}
	SetActiveProfile(&p)

	val := "make prepare"
	err := SetPrepareCommand(&val)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	// Verify on disk
	cfg := readJSON(t, filepath.Join(profileDir, "config.json"))
	if cfg["prepare-command"] != "make prepare" {
		t.Fatalf("expected 'make prepare' in config, got %v", cfg["prepare-command"])
	}

	// Verify active profile's in-memory config is updated
	active := GetActiveProfile()
	if active.Config["prepare-command"] != "make prepare" {
		t.Fatalf("expected in-memory config to be updated, got %v", active.Config["prepare-command"])
	}
}

func TestSetPrepareCommand_NilRemovesKey(t *testing.T) {
	tmpDir := setupTest(t)
	profileDir := filepath.Join(tmpDir, "profile")
	createProfile(t, profileDir, map[string]any{
		"name":            "test",
		"prepare-command": "old-cmd",
	})

	p := Profile{Name: "test", RootPath: profileDir, Config: map[string]any{
		"name":            "test",
		"prepare-command": "old-cmd",
	}}
	SetActiveProfile(&p)

	err := SetPrepareCommand(nil)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	cfg := readJSON(t, filepath.Join(profileDir, "config.json"))
	if _, exists := cfg["prepare-command"]; exists {
		t.Fatal("expected 'prepare-command' key to be removed")
	}

	active := GetActiveProfile()
	if _, exists := active.Config["prepare-command"]; exists {
		t.Fatal("expected in-memory 'prepare-command' to be removed")
	}
}

// --- Workspace root tests ---

func TestWorkspacesRootForProfile(t *testing.T) {
	p := &Profile{Name: "test", RootPath: "/tmp/my-profile"}
	result := WorkspacesRootForProfile(p)
	expected := filepath.Join("/tmp/my-profile", "workspaces")
	if result != expected {
		t.Fatalf("expected %q, got %q", expected, result)
	}
}

func TestGetWorkspacesRootPath_NoActiveProfile(t *testing.T) {
	setupTest(t)
	_, err := GetWorkspacesRootPath()
	if err == nil {
		t.Fatal("expected error when no active profile")
	}
}

func TestGetWorkspacesRootPath_WithActiveProfile(t *testing.T) {
	setupTest(t)
	p := &Profile{Name: "test", RootPath: "/tmp/my-profile", Config: map[string]any{"name": "test"}}
	SetActiveProfile(p)

	result, err := GetWorkspacesRootPath()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	expected := filepath.Join("/tmp/my-profile", "workspaces")
	if result != expected {
		t.Fatalf("expected %q, got %q", expected, result)
	}
}

// --- Convenience getter tests ---

func TestGetPrepareCommand_Found(t *testing.T) {
	tmpDir := setupTest(t)
	writeJSON(t, filepath.Join(tmpDir, "config.json"), map[string]any{
		"prepare-command": "my-prep",
	})
	resetGlobalConfigCache()

	result := GetPrepareCommand()
	if result == nil {
		t.Fatal("expected non-nil result")
	}
	if *result != "my-prep" {
		t.Fatalf("expected 'my-prep', got %q", *result)
	}
}

func TestGetPrepareCommand_NotFound(t *testing.T) {
	setupTest(t)
	result := GetPrepareCommand()
	if result != nil {
		t.Fatalf("expected nil, got %v", *result)
	}
}

func TestGetPrepareCommand_WrongType(t *testing.T) {
	tmpDir := setupTest(t)
	writeJSON(t, filepath.Join(tmpDir, "config.json"), map[string]any{
		"prepare-command": 42,
	})
	resetGlobalConfigCache()

	result := GetPrepareCommand()
	if result != nil {
		t.Fatalf("expected nil for wrong type, got %v", *result)
	}
}

func TestGetDefaultTerminalApp_Found(t *testing.T) {
	tmpDir := setupTest(t)
	writeJSON(t, filepath.Join(tmpDir, "config.json"), map[string]any{
		"terminal": map[string]any{
			"default-app": "my-app",
		},
	})
	resetGlobalConfigCache()

	result := GetDefaultTerminalApp("fallback-app")
	if result != "my-app" {
		t.Fatalf("expected 'my-app', got %q", result)
	}
}

func TestGetDefaultTerminalApp_NotFound_ReturnsFallback(t *testing.T) {
	setupTest(t)
	result := GetDefaultTerminalApp("fallback-app")
	if result != "fallback-app" {
		t.Fatalf("expected 'fallback-app', got %q", result)
	}
}

func TestGetDefaultTerminalApp_WrongType_ReturnsFallback(t *testing.T) {
	tmpDir := setupTest(t)
	writeJSON(t, filepath.Join(tmpDir, "config.json"), map[string]any{
		"terminal": map[string]any{
			"default-app": 123,
		},
	})
	resetGlobalConfigCache()

	result := GetDefaultTerminalApp("fallback-app")
	if result != "fallback-app" {
		t.Fatalf("expected 'fallback-app' for wrong type, got %q", result)
	}
}

func TestTerminalSessionCreationTimeoutSeconds_Found(t *testing.T) {
	tmpDir := setupTest(t)
	writeJSON(t, filepath.Join(tmpDir, "config.json"), map[string]any{
		"terminal": map[string]any{
			"session-creation-timeout-seconds": float64(5),
		},
	})
	resetGlobalConfigCache()

	result := TerminalSessionCreationTimeoutSeconds(3)
	if result != 5 {
		t.Fatalf("expected 5, got %d", result)
	}
}

func TestTerminalSessionCreationTimeoutSeconds_NotFound_ReturnsFallback(t *testing.T) {
	setupTest(t)
	result := TerminalSessionCreationTimeoutSeconds(3)
	if result != 3 {
		t.Fatalf("expected 3, got %d", result)
	}
}

func TestTerminalSessionCreationTimeoutSeconds_WrongType_ReturnsFallback(t *testing.T) {
	tmpDir := setupTest(t)
	writeJSON(t, filepath.Join(tmpDir, "config.json"), map[string]any{
		"terminal": map[string]any{
			"session-creation-timeout-seconds": "not-a-number",
		},
	})
	resetGlobalConfigCache()

	result := TerminalSessionCreationTimeoutSeconds(3)
	if result != 3 {
		t.Fatalf("expected 3 for wrong type, got %d", result)
	}
}

// --- ExpandTilde tests ---

func TestExpandTilde_BareTilde(t *testing.T) {
	home, err := os.UserHomeDir()
	if err != nil {
		t.Fatalf("could not get home dir: %v", err)
	}

	result := ExpandTilde("~")
	if result != home {
		t.Fatalf("expected %q, got %q", home, result)
	}
}

func TestExpandTilde_TildeSlash(t *testing.T) {
	home, err := os.UserHomeDir()
	if err != nil {
		t.Fatalf("could not get home dir: %v", err)
	}

	result := ExpandTilde("~/Documents")
	expected := filepath.Join(home, "Documents")
	if result != expected {
		t.Fatalf("expected %q, got %q", expected, result)
	}
}

func TestExpandTilde_AbsolutePath(t *testing.T) {
	result := ExpandTilde("/usr/local/bin")
	if result != "/usr/local/bin" {
		t.Fatalf("expected /usr/local/bin, got %q", result)
	}
}

func TestExpandTilde_RelativePath(t *testing.T) {
	result := ExpandTilde("some/path")
	if result != "some/path" {
		t.Fatalf("expected some/path, got %q", result)
	}
}

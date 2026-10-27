package config

import (
	"encoding/json"
	"errors"
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
		"example": map[string]any{
			"number": float64(5),
		},
	})
	resetGlobalConfigCache()

	p := Profile{Name: "test", RootPath: filepath.Join(tmpDir, "profile"), Config: map[string]any{
		"name": "test",
		"example": map[string]any{
			"number": float64(99),
		},
	}}
	SetActiveProfile(&p)

	val, ok := getGlobal("example.number")
	if !ok {
		t.Fatal("expected to find 'example.number' in global")
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

// --- IsInitializedProfile tests ---

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

// --- GetRegisteredRepoNames tests ---

// --- Repo registration tests ---

// --- Config mutation tests ---

// --- Workspace root tests ---

func TestWorkspacesRootForProfile(t *testing.T) {
	p := &Profile{Name: "test", RootPath: "/tmp/my-profile"}
	result := WorkspacesRootForProfile(p)
	expected := filepath.Join("/tmp/my-profile", "workspaces")
	if result != expected {
		t.Fatalf("expected %q, got %q", expected, result)
	}
}

// --- Convenience getter tests ---

// --- Task-tracker getter tests ---

func TestGetTaskTrackerProvider_GlobalOnly(t *testing.T) {
	tmpDir := setupTest(t)
	writeJSON(t, filepath.Join(tmpDir, "config.json"), map[string]any{
		"task-tracker": map[string]any{
			"provider": "asana",
		},
	})
	resetGlobalConfigCache()

	if got := GetTaskTrackerProvider(); got != "asana" {
		t.Fatalf("expected \"asana\", got %q", got)
	}
}

func TestGetTaskTrackerProvider_ProfileOnly(t *testing.T) {
	tmpDir := setupTest(t)
	profileDir := filepath.Join(tmpDir, "profile")
	profileCfg := map[string]any{
		"name": "p",
		"task-tracker": map[string]any{
			"provider": "asana",
		},
	}
	createProfile(t, profileDir, profileCfg)
	SetActiveProfile(&Profile{Name: "p", RootPath: profileDir, Config: profileCfg})

	if got := GetTaskTrackerProvider(); got != "asana" {
		t.Fatalf("expected \"asana\", got %q", got)
	}
}

func TestGetTaskTrackerProvider_ProfileOverridesGlobal(t *testing.T) {
	tmpDir := setupTest(t)
	writeJSON(t, filepath.Join(tmpDir, "config.json"), map[string]any{
		"task-tracker": map[string]any{
			"provider": "asana",
		},
	})
	resetGlobalConfigCache()

	profileDir := filepath.Join(tmpDir, "profile")
	profileCfg := map[string]any{
		"name": "p",
		"task-tracker": map[string]any{
			"provider": "linear",
		},
	}
	createProfile(t, profileDir, profileCfg)
	SetActiveProfile(&Profile{Name: "p", RootPath: profileDir, Config: profileCfg})

	if got := GetTaskTrackerProvider(); got != "linear" {
		t.Fatalf("expected \"linear\" (profile wins), got %q", got)
	}
}

func TestGetTaskTrackerProvider_Unset_ReturnsEmpty(t *testing.T) {
	setupTest(t)
	if got := GetTaskTrackerProvider(); got != "" {
		t.Fatalf("expected empty string, got %q", got)
	}
}

func TestGetTaskTrackerProvider_WrongType_ReturnsEmpty(t *testing.T) {
	tmpDir := setupTest(t)
	writeJSON(t, filepath.Join(tmpDir, "config.json"), map[string]any{
		"task-tracker": map[string]any{
			"provider": 42,
		},
	})
	resetGlobalConfigCache()

	if got := GetTaskTrackerProvider(); got != "" {
		t.Fatalf("expected empty string for wrong type, got %q", got)
	}
}

// Verifies the Section 4.4 merge example: global provides provider,
// workspace-id and cli-tag; profile provides project-id. All leaves must
// resolve correctly, and a missing leaf returns "".
func TestGetTaskTrackerString_PerLeafProfileGlobalMerge(t *testing.T) {
	tmpDir := setupTest(t)
	writeJSON(t, filepath.Join(tmpDir, "config.json"), map[string]any{
		"task-tracker": map[string]any{
			"provider": "asana",
			"asana": map[string]any{
				"workspace-id": "W1",
				"cli-tag":      "T1",
			},
		},
	})
	resetGlobalConfigCache()

	profileDir := filepath.Join(tmpDir, "profile")
	profileCfg := map[string]any{
		"name": "p",
		"task-tracker": map[string]any{
			"asana": map[string]any{
				"project-id": "P1",
			},
		},
	}
	createProfile(t, profileDir, profileCfg)
	SetActiveProfile(&Profile{Name: "p", RootPath: profileDir, Config: profileCfg})

	if got := GetTaskTrackerProvider(); got != "asana" {
		t.Fatalf("expected provider \"asana\", got %q", got)
	}
	if got := GetTaskTrackerString("asana", "workspace-id"); got != "W1" {
		t.Fatalf("expected workspace-id \"W1\" (global), got %q", got)
	}
	if got := GetTaskTrackerString("asana", "cli-tag"); got != "T1" {
		t.Fatalf("expected cli-tag \"T1\" (global), got %q", got)
	}
	if got := GetTaskTrackerString("asana", "project-id"); got != "P1" {
		t.Fatalf("expected project-id \"P1\" (profile), got %q", got)
	}
	if got := GetTaskTrackerString("asana", "section-id"); got != "" {
		t.Fatalf("expected section-id \"\" (unset), got %q", got)
	}
}

func TestGetTaskTrackerString_ProfileOverridesGlobalAtLeaf(t *testing.T) {
	tmpDir := setupTest(t)
	writeJSON(t, filepath.Join(tmpDir, "config.json"), map[string]any{
		"task-tracker": map[string]any{
			"asana": map[string]any{
				"workspace-id": "W-global",
			},
		},
	})
	resetGlobalConfigCache()

	profileDir := filepath.Join(tmpDir, "profile")
	profileCfg := map[string]any{
		"name": "p",
		"task-tracker": map[string]any{
			"asana": map[string]any{
				"workspace-id": "W-profile",
			},
		},
	}
	createProfile(t, profileDir, profileCfg)
	SetActiveProfile(&Profile{Name: "p", RootPath: profileDir, Config: profileCfg})

	if got := GetTaskTrackerString("asana", "workspace-id"); got != "W-profile" {
		t.Fatalf("expected profile value \"W-profile\", got %q", got)
	}
}

func TestGetTaskTrackerString_Unset_ReturnsEmpty(t *testing.T) {
	setupTest(t)
	if got := GetTaskTrackerString("asana", "workspace-id"); got != "" {
		t.Fatalf("expected empty string, got %q", got)
	}
}

func TestGetTaskTrackerString_WrongType_ReturnsEmpty(t *testing.T) {
	tmpDir := setupTest(t)
	writeJSON(t, filepath.Join(tmpDir, "config.json"), map[string]any{
		"task-tracker": map[string]any{
			"asana": map[string]any{
				"workspace-id": 123,
			},
		},
	})
	resetGlobalConfigCache()

	if got := GetTaskTrackerString("asana", "workspace-id"); got != "" {
		t.Fatalf("expected empty string for wrong type, got %q", got)
	}
}

func TestGetBranchPrefix_Global(t *testing.T) {
	tmpDir := setupTest(t)
	writeJSON(t, filepath.Join(tmpDir, "config.json"), map[string]any{
		"feature": map[string]any{
			"branch-prefix": "feat",
		},
	})
	resetGlobalConfigCache()

	if got := GetBranchPrefix("fallback"); got != "feat" {
		t.Fatalf("expected \"feat\", got %q", got)
	}
}

func TestGetBranchPrefix_ProfileOverridesGlobal(t *testing.T) {
	tmpDir := setupTest(t)
	writeJSON(t, filepath.Join(tmpDir, "config.json"), map[string]any{
		"feature": map[string]any{
			"branch-prefix": "feat",
		},
	})
	resetGlobalConfigCache()

	profileDir := filepath.Join(tmpDir, "profile")
	profileCfg := map[string]any{
		"name": "p",
		"feature": map[string]any{
			"branch-prefix": "work",
		},
	}
	createProfile(t, profileDir, profileCfg)
	SetActiveProfile(&Profile{Name: "p", RootPath: profileDir, Config: profileCfg})

	if got := GetBranchPrefix("fallback"); got != "work" {
		t.Fatalf("expected \"work\" (profile wins), got %q", got)
	}
}

func TestGetBranchPrefix_Unset_ReturnsFallback(t *testing.T) {
	setupTest(t)
	if got := GetBranchPrefix("fallback"); got != "fallback" {
		t.Fatalf("expected \"fallback\", got %q", got)
	}
}

func TestGetBranchPrefix_WrongType_ReturnsFallback(t *testing.T) {
	tmpDir := setupTest(t)
	writeJSON(t, filepath.Join(tmpDir, "config.json"), map[string]any{
		"feature": map[string]any{
			"branch-prefix": 42,
		},
	})
	resetGlobalConfigCache()

	if got := GetBranchPrefix("fallback"); got != "fallback" {
		t.Fatalf("expected \"fallback\" for wrong type, got %q", got)
	}
}

func TestGetAutoMergeDefault_Global(t *testing.T) {
	tmpDir := setupTest(t)
	writeJSON(t, filepath.Join(tmpDir, "config.json"), map[string]any{
		"pr": map[string]any{
			"auto-merge": false,
		},
	})
	resetGlobalConfigCache()

	if got := GetAutoMergeDefault(true); got != false {
		t.Fatalf("expected false, got %v", got)
	}
}

func TestGetAutoMergeDefault_ProfileOverridesGlobal(t *testing.T) {
	tmpDir := setupTest(t)
	writeJSON(t, filepath.Join(tmpDir, "config.json"), map[string]any{
		"pr": map[string]any{
			"auto-merge": true,
		},
	})
	resetGlobalConfigCache()

	profileDir := filepath.Join(tmpDir, "profile")
	profileCfg := map[string]any{
		"name": "p",
		"pr": map[string]any{
			"auto-merge": false,
		},
	}
	createProfile(t, profileDir, profileCfg)
	SetActiveProfile(&Profile{Name: "p", RootPath: profileDir, Config: profileCfg})

	if got := GetAutoMergeDefault(true); got != false {
		t.Fatalf("expected false (profile wins), got %v", got)
	}
}

func TestGetAutoMergeDefault_Unset_ReturnsFallback(t *testing.T) {
	setupTest(t)
	if got := GetAutoMergeDefault(true); got != true {
		t.Fatalf("expected true (fallback), got %v", got)
	}
	if got := GetAutoMergeDefault(false); got != false {
		t.Fatalf("expected false (fallback), got %v", got)
	}
}

func TestGetAutoMergeDefault_WrongType_ReturnsFallback(t *testing.T) {
	tmpDir := setupTest(t)
	writeJSON(t, filepath.Join(tmpDir, "config.json"), map[string]any{
		"pr": map[string]any{
			"auto-merge": "yes",
		},
	})
	resetGlobalConfigCache()

	if got := GetAutoMergeDefault(true); got != true {
		t.Fatalf("expected true (fallback for wrong type), got %v", got)
	}
}

// --- Profile unregistration tests ---

// --- ExpandTilde tests ---

// --- Repo unregistration tests ---

// --- GetExplicitRepoEntries tests ---

func TestConfigPath_ReturnsConfigPathValue(t *testing.T) {
	tmpDir := setupTest(t)
	customPath := filepath.Join(tmpDir, "custom", "config.json")
	setConfigPathForTesting(customPath)

	got := ConfigPath()
	if got != customPath {
		t.Fatalf("expected %q, got %q", customPath, got)
	}
}

func TestDefaultConfigPath_HonorsDevoraConfigPathEnv(t *testing.T) {
	t.Setenv("DEVORA_CONFIG_PATH", "/custom/devora/config.json")

	got := defaultConfigPath()
	if got != "/custom/devora/config.json" {
		t.Fatalf("expected DEVORA_CONFIG_PATH to win, got %q", got)
	}
}

func TestDefaultConfigPath_FallsBackToHomeWhenEnvUnset(t *testing.T) {
	t.Setenv("DEVORA_CONFIG_PATH", "")

	home, err := os.UserHomeDir()
	if err != nil {
		t.Fatalf("could not get home dir: %v", err)
	}
	want := filepath.Join(home, ".config", "devora", "config.json")

	got := defaultConfigPath()
	if got != want {
		t.Fatalf("expected fallback %q, got %q", want, got)
	}
}

// --- Default-terminal-app setter/raw-getter tests ---

// --- pr.auto-merge setter/raw-getter tests ---

func TestSetPrAutoMergeGlobal_Set(t *testing.T) {
	tmpDir := setupTest(t)

	val := true
	err := SetPrAutoMergeGlobal(&val)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	cfg := readJSON(t, filepath.Join(tmpDir, "config.json"))
	pr, ok := cfg["pr"].(map[string]any)
	if !ok {
		t.Fatalf("expected 'pr' map, got %v", cfg["pr"])
	}
	if pr["auto-merge"] != true {
		t.Fatalf("expected true, got %v", pr["auto-merge"])
	}
}

func TestSetPrAutoMergeGlobal_ClearWithNil(t *testing.T) {
	tmpDir := setupTest(t)
	writeJSON(t, filepath.Join(tmpDir, "config.json"), map[string]any{
		"pr": map[string]any{
			"auto-merge": false,
		},
	})
	resetGlobalConfigCache()

	err := SetPrAutoMergeGlobal(nil)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	cfg := readJSON(t, filepath.Join(tmpDir, "config.json"))
	if _, exists := cfg["pr"]; exists {
		t.Fatalf("expected 'pr' map to be removed, got %v", cfg["pr"])
	}
}

func TestSetPrAutoMergeGlobal_PreservesSiblings(t *testing.T) {
	tmpDir := setupTest(t)
	writeJSON(t, filepath.Join(tmpDir, "config.json"), map[string]any{
		"pr": map[string]any{
			"auto-merge":  true,
			"other-field": "keep-me",
		},
	})
	resetGlobalConfigCache()

	err := SetPrAutoMergeGlobal(nil)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	cfg := readJSON(t, filepath.Join(tmpDir, "config.json"))
	pr, ok := cfg["pr"].(map[string]any)
	if !ok {
		t.Fatalf("expected 'pr' map to still exist, got %v", cfg["pr"])
	}
	if _, exists := pr["auto-merge"]; exists {
		t.Fatalf("expected 'auto-merge' to be removed, got %v", pr["auto-merge"])
	}
	if pr["other-field"] != "keep-me" {
		t.Fatalf("expected 'other-field' to be preserved, got %v", pr["other-field"])
	}
}

func TestSetPrAutoMergeProfile_Set(t *testing.T) {
	tmpDir := setupTest(t)
	profileDir := filepath.Join(tmpDir, "profile")
	createProfile(t, profileDir, map[string]any{"name": "test"})

	p := Profile{Name: "test", RootPath: profileDir, Config: map[string]any{"name": "test"}}
	SetActiveProfile(&p)

	val := false
	err := SetPrAutoMergeProfile(&val)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	cfg := readJSON(t, filepath.Join(profileDir, "config.json"))
	pr, ok := cfg["pr"].(map[string]any)
	if !ok {
		t.Fatalf("expected 'pr' map, got %v", cfg["pr"])
	}
	if pr["auto-merge"] != false {
		t.Fatalf("expected false on disk, got %v", pr["auto-merge"])
	}

	active := GetActiveProfile()
	activePr, ok := active.Config["pr"].(map[string]any)
	if !ok {
		t.Fatalf("expected in-memory 'pr' map, got %v", active.Config["pr"])
	}
	if activePr["auto-merge"] != false {
		t.Fatalf("expected in-memory false, got %v", activePr["auto-merge"])
	}
}

func TestSetPrAutoMergeProfile_ClearWithNil(t *testing.T) {
	tmpDir := setupTest(t)
	profileDir := filepath.Join(tmpDir, "profile")
	profileCfg := map[string]any{
		"name": "test",
		"pr": map[string]any{
			"auto-merge": true,
		},
	}
	createProfile(t, profileDir, profileCfg)

	p := Profile{Name: "test", RootPath: profileDir, Config: profileCfg}
	SetActiveProfile(&p)

	err := SetPrAutoMergeProfile(nil)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	cfg := readJSON(t, filepath.Join(profileDir, "config.json"))
	if _, exists := cfg["pr"]; exists {
		t.Fatalf("expected 'pr' map to be removed, got %v", cfg["pr"])
	}

	active := GetActiveProfile()
	if _, exists := active.Config["pr"]; exists {
		t.Fatalf("expected in-memory 'pr' to be removed, got %v", active.Config["pr"])
	}
}

func TestSetPrAutoMergeProfile_NoActiveProfile(t *testing.T) {
	setupTest(t)
	val := true
	err := SetPrAutoMergeProfile(&val)
	if err == nil {
		t.Fatal("expected error when no active profile")
	}
}

func TestGetPrAutoMergeGlobalRaw_UnsetReturnsNil(t *testing.T) {
	setupTest(t)
	result := GetPrAutoMergeGlobalRaw()
	if result != nil {
		t.Fatalf("expected nil, got %v", *result)
	}
}

func TestGetPrAutoMergeGlobalRaw_SetReturnsPointer(t *testing.T) {
	tmpDir := setupTest(t)
	writeJSON(t, filepath.Join(tmpDir, "config.json"), map[string]any{
		"pr": map[string]any{
			"auto-merge": true,
		},
	})
	resetGlobalConfigCache()

	result := GetPrAutoMergeGlobalRaw()
	if result == nil {
		t.Fatal("expected non-nil result")
	}
	if *result != true {
		t.Fatalf("expected true, got %v", *result)
	}
}

func TestGetPrAutoMergeGlobalRaw_IgnoresProfile(t *testing.T) {
	tmpDir := setupTest(t)
	profileDir := filepath.Join(tmpDir, "profile")
	profileCfg := map[string]any{
		"name": "test",
		"pr": map[string]any{
			"auto-merge": true,
		},
	}
	createProfile(t, profileDir, profileCfg)
	SetActiveProfile(&Profile{Name: "test", RootPath: profileDir, Config: profileCfg})

	result := GetPrAutoMergeGlobalRaw()
	if result != nil {
		t.Fatalf("expected nil (should ignore profile), got %v", *result)
	}
}

func TestGetPrAutoMergeGlobalRaw_WrongTypeReturnsNil(t *testing.T) {
	tmpDir := setupTest(t)
	writeJSON(t, filepath.Join(tmpDir, "config.json"), map[string]any{
		"pr": map[string]any{
			"auto-merge": "yes",
		},
	})
	resetGlobalConfigCache()

	result := GetPrAutoMergeGlobalRaw()
	if result != nil {
		t.Fatalf("expected nil for wrong-type value, got %v", *result)
	}
}

func TestGetPrAutoMergeProfileRaw_UnsetReturnsNil(t *testing.T) {
	tmpDir := setupTest(t)
	profileDir := filepath.Join(tmpDir, "profile")
	profileCfg := map[string]any{"name": "test"}
	createProfile(t, profileDir, profileCfg)
	SetActiveProfile(&Profile{Name: "test", RootPath: profileDir, Config: profileCfg})

	result := GetPrAutoMergeProfileRaw()
	if result != nil {
		t.Fatalf("expected nil, got %v", *result)
	}
}

func TestGetPrAutoMergeProfileRaw_SetReturnsPointer(t *testing.T) {
	tmpDir := setupTest(t)
	profileDir := filepath.Join(tmpDir, "profile")
	profileCfg := map[string]any{
		"name": "test",
		"pr": map[string]any{
			"auto-merge": false,
		},
	}
	createProfile(t, profileDir, profileCfg)
	SetActiveProfile(&Profile{Name: "test", RootPath: profileDir, Config: profileCfg})

	result := GetPrAutoMergeProfileRaw()
	if result == nil {
		t.Fatal("expected non-nil result")
	}
	if *result != false {
		t.Fatalf("expected false, got %v", *result)
	}
}

func TestGetPrAutoMergeProfileRaw_NoActiveProfileReturnsNil(t *testing.T) {
	setupTest(t)
	result := GetPrAutoMergeProfileRaw()
	if result != nil {
		t.Fatalf("expected nil, got %v", *result)
	}
}

func TestGetPrAutoMergeProfileRaw_WrongTypeReturnsNil(t *testing.T) {
	tmpDir := setupTest(t)
	profileDir := filepath.Join(tmpDir, "profile")
	profileCfg := map[string]any{
		"name": "test",
		"pr": map[string]any{
			"auto-merge": "nope",
		},
	}
	createProfile(t, profileDir, profileCfg)
	SetActiveProfile(&Profile{Name: "test", RootPath: profileDir, Config: profileCfg})

	result := GetPrAutoMergeProfileRaw()
	if result != nil {
		t.Fatalf("expected nil for wrong-type value, got %v", *result)
	}
}

// --- GetAutoMergeDefaultForRepo tests ---

func TestGetAutoMergeDefaultForRepo_PerRepoWinsOverProfileAndGlobal(t *testing.T) {
	tmpDir := setupTest(t)
	writeJSON(t, filepath.Join(tmpDir, "config.json"), map[string]any{
		"pr": map[string]any{"auto-merge": true},
	})
	resetGlobalConfigCache()

	profileDir := filepath.Join(tmpDir, "profile")
	profileCfg := map[string]any{
		"name": "test",
		"pr":   map[string]any{"auto-merge": true},
	}
	createProfile(t, profileDir, profileCfg)
	SetActiveProfile(&Profile{Name: "test", RootPath: profileDir, Config: profileCfg})

	repoVal := false
	getRepo := func() (*bool, error) { return &repoVal, nil }

	if got := GetAutoMergeDefaultForRepo(true, getRepo); got != false {
		t.Fatalf("expected false (per-repo wins), got %v", got)
	}
}

func TestGetAutoMergeDefaultForRepo_PerRepoUnsetFallsThroughToProfile(t *testing.T) {
	tmpDir := setupTest(t)
	writeJSON(t, filepath.Join(tmpDir, "config.json"), map[string]any{
		"pr": map[string]any{"auto-merge": true},
	})
	resetGlobalConfigCache()

	profileDir := filepath.Join(tmpDir, "profile")
	profileCfg := map[string]any{
		"name": "test",
		"pr":   map[string]any{"auto-merge": false},
	}
	createProfile(t, profileDir, profileCfg)
	SetActiveProfile(&Profile{Name: "test", RootPath: profileDir, Config: profileCfg})

	getRepo := func() (*bool, error) { return nil, nil }

	if got := GetAutoMergeDefaultForRepo(true, getRepo); got != false {
		t.Fatalf("expected false (profile wins over global when repo unset), got %v", got)
	}
}

func TestGetAutoMergeDefaultForRepo_AllUnsetReturnsFallback(t *testing.T) {
	setupTest(t)

	getRepo := func() (*bool, error) { return nil, nil }

	if got := GetAutoMergeDefaultForRepo(true, getRepo); got != true {
		t.Fatalf("expected fallback true, got %v", got)
	}
	if got := GetAutoMergeDefaultForRepo(false, getRepo); got != false {
		t.Fatalf("expected fallback false, got %v", got)
	}
}

func TestGetAutoMergeDefaultForRepo_RepoErrorWarnsAndFallsThrough(t *testing.T) {
	tmpDir := setupTest(t)
	writeJSON(t, filepath.Join(tmpDir, "config.json"), map[string]any{
		"pr": map[string]any{"auto-merge": false},
	})
	resetGlobalConfigCache()

	getRepo := func() (*bool, error) { return nil, errors.New("boom") }

	if got := GetAutoMergeDefaultForRepo(true, getRepo); got != false {
		t.Fatalf("expected global value false, got %v", got)
	}
}

func TestGetAutoMergeDefaultForRepo_NilGetRepo(t *testing.T) {
	tmpDir := setupTest(t)
	writeJSON(t, filepath.Join(tmpDir, "config.json"), map[string]any{
		"pr": map[string]any{"auto-merge": false},
	})
	resetGlobalConfigCache()

	if got := GetAutoMergeDefaultForRepo(true, nil); got != false {
		t.Fatalf("expected global value false, got %v", got)
	}
}

// --- Exported Get tests ---

func TestGet_ProfileValueOnly_ReturnsProfileValue(t *testing.T) {
	setupTest(t)
	p := &Profile{Name: "test", RootPath: "/tmp/test", Config: map[string]any{
		"name":            "test",
		"prepare-command": "make build",
	}}
	SetActiveProfile(p)

	val, ok := Get("prepare-command")
	if !ok {
		t.Fatal("expected to find 'prepare-command' via Get")
	}
	if val != "make build" {
		t.Fatalf("expected 'make build', got %v", val)
	}
}

func TestGet_GlobalValueOnly_ReturnsGlobalValue(t *testing.T) {
	tmpDir := setupTest(t)
	writeJSON(t, filepath.Join(tmpDir, "config.json"), map[string]any{
		"prepare-command": "global-prep",
	})
	resetGlobalConfigCache()

	val, ok := Get("prepare-command")
	if !ok {
		t.Fatal("expected to find 'prepare-command' in global config via Get")
	}
	if val != "global-prep" {
		t.Fatalf("expected 'global-prep', got %v", val)
	}
}

func TestGet_BothLevels_ProfileWins(t *testing.T) {
	tmpDir := setupTest(t)
	writeJSON(t, filepath.Join(tmpDir, "config.json"), map[string]any{
		"prepare-command": "global-prep",
	})
	resetGlobalConfigCache()

	p := &Profile{Name: "test", RootPath: filepath.Join(tmpDir, "profile"), Config: map[string]any{
		"name":            "test",
		"prepare-command": "profile-prep",
	}}
	SetActiveProfile(p)

	val, ok := Get("prepare-command")
	if !ok {
		t.Fatal("expected to find 'prepare-command' via Get")
	}
	if val != "profile-prep" {
		t.Fatalf("expected 'profile-prep' (profile wins), got %v", val)
	}
}

func TestGet_NoValue_ReturnsNotFound(t *testing.T) {
	setupTest(t)
	val, ok := Get("nonexistent-key")
	if ok {
		t.Fatalf("expected not-found, got val=%v", val)
	}
	if val != nil {
		t.Fatalf("expected nil value for not-found key, got %v", val)
	}
}

func TestGet_NestedPath_Resolves(t *testing.T) {
	setupTest(t)
	p := &Profile{Name: "test", RootPath: "/tmp/test", Config: map[string]any{
		"name": "test",
		"a": map[string]any{
			"b": "deep-value",
		},
	}}
	SetActiveProfile(p)

	val, ok := Get("a.b")
	if !ok {
		t.Fatal("expected to find 'a.b' via Get")
	}
	if val != "deep-value" {
		t.Fatalf("expected 'deep-value', got %v", val)
	}
}

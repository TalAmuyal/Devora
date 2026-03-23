package workspace

import (
	"encoding/json"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"testing"

	"devora/internal/config"
)

// --- Test helpers ---

func setupConfigTest(t *testing.T) string {
	t.Helper()
	tmpDir := t.TempDir()
	globalConfigPath := filepath.Join(tmpDir, "config.json")
	config.SetConfigPathForTesting(globalConfigPath)
	t.Cleanup(func() {
		config.ResetForTesting()
	})
	return tmpDir
}

func createProfile(t *testing.T, rootPath string, profileConfig map[string]any) {
	t.Helper()
	if err := os.MkdirAll(filepath.Join(rootPath, "repos"), 0o755); err != nil {
		t.Fatalf("failed to create profile dirs: %v", err)
	}
	if err := os.MkdirAll(filepath.Join(rootPath, "workspaces"), 0o755); err != nil {
		t.Fatalf("failed to create workspaces dir: %v", err)
	}
	b, err := json.MarshalIndent(profileConfig, "", "    ")
	if err != nil {
		t.Fatalf("failed to marshal JSON: %v", err)
	}
	if err := os.WriteFile(filepath.Join(rootPath, "config.json"), b, 0o644); err != nil {
		t.Fatalf("failed to write config.json: %v", err)
	}
}

func writeGlobalConfig(t *testing.T, dir string, data map[string]any) {
	t.Helper()
	b, err := json.MarshalIndent(data, "", "    ")
	if err != nil {
		t.Fatalf("failed to marshal JSON: %v", err)
	}
	if err := os.WriteFile(filepath.Join(dir, "config.json"), b, 0o644); err != nil {
		t.Fatalf("failed to write config.json: %v", err)
	}
}

func makeWorkspace(t *testing.T, workspacesRoot string, name string, initialized bool, hasTask bool, repos []string) string {
	t.Helper()
	wsPath := filepath.Join(workspacesRoot, name)
	if err := os.MkdirAll(wsPath, 0o755); err != nil {
		t.Fatalf("failed to create workspace dir: %v", err)
	}
	if initialized {
		if err := os.WriteFile(filepath.Join(wsPath, InitializedMarkerName), []byte{}, 0o666); err != nil {
			t.Fatalf("failed to create initialized marker: %v", err)
		}
	}
	if hasTask {
		if err := os.WriteFile(filepath.Join(wsPath, TaskFileName), []byte("{}"), 0o666); err != nil {
			t.Fatalf("failed to create task.json: %v", err)
		}
	}
	for _, repo := range repos {
		if err := os.MkdirAll(filepath.Join(wsPath, repo), 0o755); err != nil {
			t.Fatalf("failed to create repo dir: %v", err)
		}
	}
	return wsPath
}

// --- State detection tests ---

func TestIsActive_WithInitializedAndTask(t *testing.T) {
	wsPath := makeWorkspace(t, t.TempDir(), "ws-1", true, true, nil)
	if !IsActive(wsPath) {
		t.Fatal("expected workspace to be active")
	}
}

func TestIsActive_WithInitializedOnly(t *testing.T) {
	wsPath := makeWorkspace(t, t.TempDir(), "ws-1", true, false, nil)
	if IsActive(wsPath) {
		t.Fatal("expected workspace to not be active (no task)")
	}
}

func TestIsInactive_WithInitializedOnly(t *testing.T) {
	wsPath := makeWorkspace(t, t.TempDir(), "ws-1", true, false, nil)
	if !IsInactive(wsPath) {
		t.Fatal("expected workspace to be inactive")
	}
}

func TestIsInactive_WithInitializedAndTask(t *testing.T) {
	wsPath := makeWorkspace(t, t.TempDir(), "ws-1", true, true, nil)
	if IsInactive(wsPath) {
		t.Fatal("expected workspace to not be inactive (has task)")
	}
}

func TestIsInvalid_WithNeitherFile(t *testing.T) {
	wsPath := makeWorkspace(t, t.TempDir(), "ws-1", false, false, nil)
	if !IsInvalid(wsPath) {
		t.Fatal("expected workspace to be invalid")
	}
}

func TestIsInvalid_WithTaskOnly(t *testing.T) {
	wsPath := makeWorkspace(t, t.TempDir(), "ws-1", false, true, nil)
	if !IsInvalid(wsPath) {
		t.Fatal("expected workspace to be invalid (task only, not initialized)")
	}
}

func TestIsInvalid_WithInitialized(t *testing.T) {
	wsPath := makeWorkspace(t, t.TempDir(), "ws-1", true, false, nil)
	if IsInvalid(wsPath) {
		t.Fatal("expected workspace to not be invalid (initialized)")
	}
}

// --- Path helper tests ---

func TestGetWorkspaceRepoPath(t *testing.T) {
	result := GetWorkspaceRepoPath("/workspaces/ws-1", "my-repo")
	expected := filepath.Join("/workspaces/ws-1", "my-repo")
	if result != expected {
		t.Fatalf("expected %q, got %q", expected, result)
	}
}

func TestGetWorkspaceTaskPath(t *testing.T) {
	result := GetWorkspaceTaskPath("/workspaces/ws-1")
	expected := filepath.Join("/workspaces/ws-1", TaskFileName)
	if result != expected {
		t.Fatalf("expected %q, got %q", expected, result)
	}
}

// --- Enumeration tests ---

func TestGetWorkspaces_EmptyDirectory(t *testing.T) {
	root := t.TempDir()
	workspaces, err := GetWorkspaces(root)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(workspaces) != 0 {
		t.Fatalf("expected empty result, got %v", workspaces)
	}
}

func TestGetWorkspaces_NonExistentDirectory(t *testing.T) {
	workspaces, err := GetWorkspaces("/nonexistent/path")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(workspaces) != 0 {
		t.Fatalf("expected empty result, got %v", workspaces)
	}
}

func TestGetWorkspaces_MixOfWsAndOtherDirs(t *testing.T) {
	root := t.TempDir()
	for _, name := range []string{"ws-1", "ws-2", "other-dir", "not-workspace"} {
		if err := os.MkdirAll(filepath.Join(root, name), 0o755); err != nil {
			t.Fatal(err)
		}
	}
	// Also create a file starting with ws-
	if err := os.WriteFile(filepath.Join(root, "ws-file"), []byte("not a dir"), 0o644); err != nil {
		t.Fatal(err)
	}

	workspaces, err := GetWorkspaces(root)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if len(workspaces) != 2 {
		t.Fatalf("expected 2 workspaces, got %d: %v", len(workspaces), workspaces)
	}

	// Sort for deterministic comparison
	sort.Strings(workspaces)
	expected := []string{
		filepath.Join(root, "ws-1"),
		filepath.Join(root, "ws-2"),
	}
	sort.Strings(expected)
	for i, ws := range workspaces {
		if ws != expected[i] {
			t.Fatalf("expected %v, got %v", expected, workspaces)
		}
	}
}

func TestGetWorkspaceRepos_ReturnsNonDotPrefixedDirs(t *testing.T) {
	wsPath := t.TempDir()
	for _, name := range []string{"repo-a", "repo-b", ".hidden", ".creation-lock"} {
		if err := os.MkdirAll(filepath.Join(wsPath, name), 0o755); err != nil {
			t.Fatal(err)
		}
	}
	// Also create a file (should be excluded)
	if err := os.WriteFile(filepath.Join(wsPath, "some-file"), []byte("data"), 0o644); err != nil {
		t.Fatal(err)
	}
	// Create initialized marker (a file, not a directory)
	if err := os.WriteFile(filepath.Join(wsPath, "initialized"), []byte{}, 0o666); err != nil {
		t.Fatal(err)
	}

	repos, err := GetWorkspaceRepos(wsPath)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	sort.Strings(repos)
	expected := []string{"repo-a", "repo-b"}
	if len(repos) != len(expected) {
		t.Fatalf("expected %v, got %v", expected, repos)
	}
	for i, r := range repos {
		if r != expected[i] {
			t.Fatalf("expected %v, got %v", expected, repos)
		}
	}
}

func TestGetWorkspaceRepos_ExcludesFiles(t *testing.T) {
	wsPath := t.TempDir()
	// Only files, no directories
	for _, name := range []string{"initialized", "task.json", ".creation-lock"} {
		if err := os.WriteFile(filepath.Join(wsPath, name), []byte{}, 0o666); err != nil {
			t.Fatal(err)
		}
	}

	repos, err := GetWorkspaceRepos(wsPath)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(repos) != 0 {
		t.Fatalf("expected empty result, got %v", repos)
	}
}

// --- Filtered query tests ---

func TestGetActiveWorkspaces_ReturnsOnlyActiveUnlocked(t *testing.T) {
	tmpDir := setupConfigTest(t)
	profileDir := filepath.Join(tmpDir, "profile")
	createProfile(t, profileDir, map[string]any{"name": "test"})
	config.SetActiveProfile(&config.Profile{Name: "test", RootPath: profileDir, Config: map[string]any{"name": "test"}})

	workspacesRoot := filepath.Join(profileDir, "workspaces")
	makeWorkspace(t, workspacesRoot, "ws-1", true, true, nil)   // active
	makeWorkspace(t, workspacesRoot, "ws-2", true, false, nil)  // inactive
	makeWorkspace(t, workspacesRoot, "ws-3", false, false, nil) // invalid

	active, err := GetActiveWorkspaces()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(active) != 1 {
		t.Fatalf("expected 1 active workspace, got %d: %v", len(active), active)
	}
	if filepath.Base(active[0]) != "ws-1" {
		t.Fatalf("expected ws-1, got %s", filepath.Base(active[0]))
	}
}

func TestGetInactiveWorkspaces_ReturnsOnlyInactiveUnlocked(t *testing.T) {
	tmpDir := setupConfigTest(t)
	profileDir := filepath.Join(tmpDir, "profile")
	createProfile(t, profileDir, map[string]any{"name": "test"})
	config.SetActiveProfile(&config.Profile{Name: "test", RootPath: profileDir, Config: map[string]any{"name": "test"}})

	workspacesRoot := filepath.Join(profileDir, "workspaces")
	makeWorkspace(t, workspacesRoot, "ws-1", true, true, nil)   // active
	makeWorkspace(t, workspacesRoot, "ws-2", true, false, nil)  // inactive
	makeWorkspace(t, workspacesRoot, "ws-3", false, false, nil) // invalid

	inactive, err := GetInactiveWorkspaces()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(inactive) != 1 {
		t.Fatalf("expected 1 inactive workspace, got %d: %v", len(inactive), inactive)
	}
	if filepath.Base(inactive[0]) != "ws-2" {
		t.Fatalf("expected ws-2, got %s", filepath.Base(inactive[0]))
	}
}

func TestGetInvalidWorkspaces_ReturnsOnlyInvalidUnlocked(t *testing.T) {
	tmpDir := setupConfigTest(t)
	profileDir := filepath.Join(tmpDir, "profile")
	createProfile(t, profileDir, map[string]any{"name": "test"})
	config.SetActiveProfile(&config.Profile{Name: "test", RootPath: profileDir, Config: map[string]any{"name": "test"}})

	workspacesRoot := filepath.Join(profileDir, "workspaces")
	makeWorkspace(t, workspacesRoot, "ws-1", true, true, nil)   // active
	makeWorkspace(t, workspacesRoot, "ws-2", true, false, nil)  // inactive
	makeWorkspace(t, workspacesRoot, "ws-3", false, false, nil) // invalid

	invalid, err := GetInvalidWorkspaces()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(invalid) != 1 {
		t.Fatalf("expected 1 invalid workspace, got %d: %v", len(invalid), invalid)
	}
	if filepath.Base(invalid[0]) != "ws-3" {
		t.Fatalf("expected ws-3, got %s", filepath.Base(invalid[0]))
	}
}

// --- Search tests ---

func TestSearchAvailableWorkspace_MatchingInactiveWorkspace(t *testing.T) {
	root := t.TempDir()
	makeWorkspace(t, root, "ws-1", true, false, []string{"repo-a", "repo-b"})

	result, err := SearchAvailableWorkspace([]string{"repo-a", "repo-b"}, root)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result == "" {
		t.Fatal("expected a matching workspace, got empty string")
	}
	if filepath.Base(result) != "ws-1" {
		t.Fatalf("expected ws-1, got %s", filepath.Base(result))
	}
}

func TestSearchAvailableWorkspace_NoMatch(t *testing.T) {
	root := t.TempDir()
	makeWorkspace(t, root, "ws-1", true, false, []string{"repo-a", "repo-b"})

	result, err := SearchAvailableWorkspace([]string{"repo-c"}, root)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result != "" {
		t.Fatalf("expected empty string, got %q", result)
	}
}

func TestSearchAvailableWorkspace_SkipsActiveWorkspace(t *testing.T) {
	root := t.TempDir()
	makeWorkspace(t, root, "ws-1", true, true, []string{"repo-a"}) // active (has task)

	result, err := SearchAvailableWorkspace([]string{"repo-a"}, root)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result != "" {
		t.Fatalf("expected empty string (active should be skipped), got %q", result)
	}
}

func TestSearchAvailableWorkspace_SkipsLockedWorkspace(t *testing.T) {
	root := t.TempDir()
	wsPath := makeWorkspace(t, root, "ws-1", true, false, []string{"repo-a"})

	// Lock the workspace
	lock, err := LockWorkspace(wsPath)
	if err != nil {
		t.Fatalf("failed to lock workspace: %v", err)
	}
	defer lock.Close()

	result, err := SearchAvailableWorkspace([]string{"repo-a"}, root)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result != "" {
		t.Fatalf("expected empty string (locked should be skipped), got %q", result)
	}
}

func TestSearchAvailableWorkspace_DifferentOrderStillMatches(t *testing.T) {
	root := t.TempDir()
	makeWorkspace(t, root, "ws-1", true, false, []string{"repo-b", "repo-a"})

	// Search with different order
	result, err := SearchAvailableWorkspace([]string{"repo-a", "repo-b"}, root)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result == "" {
		t.Fatal("expected a matching workspace (different order), got empty string")
	}
}

// --- Creation tests ---

func TestCreateWorkspaceDirectory_EmptyRoot(t *testing.T) {
	root := t.TempDir()
	wsPath, err := CreateWorkspaceDirectory(root)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if filepath.Base(wsPath) != "ws-1" {
		t.Fatalf("expected ws-1, got %s", filepath.Base(wsPath))
	}
	info, err := os.Stat(wsPath)
	if err != nil {
		t.Fatalf("workspace directory should exist: %v", err)
	}
	if !info.IsDir() {
		t.Fatal("expected a directory")
	}
}

func TestCreateWorkspaceDirectory_WithExistingWs1(t *testing.T) {
	root := t.TempDir()
	if err := os.MkdirAll(filepath.Join(root, "ws-1"), 0o755); err != nil {
		t.Fatal(err)
	}

	wsPath, err := CreateWorkspaceDirectory(root)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if filepath.Base(wsPath) != "ws-2" {
		t.Fatalf("expected ws-2, got %s", filepath.Base(wsPath))
	}
}

func TestCreateWorkspaceDirectory_FillsGaps(t *testing.T) {
	root := t.TempDir()
	for _, name := range []string{"ws-1", "ws-3"} {
		if err := os.MkdirAll(filepath.Join(root, name), 0o755); err != nil {
			t.Fatal(err)
		}
	}

	wsPath, err := CreateWorkspaceDirectory(root)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if filepath.Base(wsPath) != "ws-2" {
		t.Fatalf("expected ws-2 (filling gap), got %s", filepath.Base(wsPath))
	}
}

func TestMarkInitialized_CreatesMarkerFile(t *testing.T) {
	wsPath := t.TempDir()
	if err := MarkInitialized(wsPath); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !IsInitialized(wsPath) {
		t.Fatal("expected workspace to be initialized after MarkInitialized")
	}
}

// --- CLAUDE.md tests ---

func TestWriteWorkspaceCLAUDEMD_CreatesFileWithExpectedContent(t *testing.T) {
	wsPath := t.TempDir()
	if err := WriteWorkspaceCLAUDEMD(wsPath); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	content, err := os.ReadFile(filepath.Join(wsPath, ClaudeMDFileName))
	if err != nil {
		t.Fatalf("failed to read CLAUDE.md: %v", err)
	}
	if string(content) != WorkspaceCLAUDEMDContent {
		t.Fatalf("expected %q, got %q", WorkspaceCLAUDEMDContent, string(content))
	}
}

func TestEnsureWorkspaceCLAUDEMD_CreatesWhenMultipleReposAndAbsent(t *testing.T) {
	wsPath := t.TempDir()
	// Create two repo directories
	for _, repo := range []string{"repo-a", "repo-b"} {
		if err := os.MkdirAll(filepath.Join(wsPath, repo), 0o755); err != nil {
			t.Fatal(err)
		}
	}

	if err := EnsureWorkspaceCLAUDEMD(wsPath); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	content, err := os.ReadFile(filepath.Join(wsPath, ClaudeMDFileName))
	if err != nil {
		t.Fatalf("expected CLAUDE.md to be created: %v", err)
	}
	if string(content) != WorkspaceCLAUDEMDContent {
		t.Fatalf("unexpected content: %q", string(content))
	}
}

func TestEnsureWorkspaceCLAUDEMD_DoesNotCreateWhenSingleRepo(t *testing.T) {
	wsPath := t.TempDir()
	if err := os.MkdirAll(filepath.Join(wsPath, "repo-a"), 0o755); err != nil {
		t.Fatal(err)
	}

	if err := EnsureWorkspaceCLAUDEMD(wsPath); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if _, err := os.Stat(filepath.Join(wsPath, ClaudeMDFileName)); err == nil {
		t.Fatal("expected CLAUDE.md to not be created for single repo")
	}
}

func TestEnsureWorkspaceCLAUDEMD_DoesNotOverwriteExisting(t *testing.T) {
	wsPath := t.TempDir()
	for _, repo := range []string{"repo-a", "repo-b"} {
		if err := os.MkdirAll(filepath.Join(wsPath, repo), 0o755); err != nil {
			t.Fatal(err)
		}
	}

	customContent := "custom content"
	if err := os.WriteFile(filepath.Join(wsPath, ClaudeMDFileName), []byte(customContent), 0o666); err != nil {
		t.Fatal(err)
	}

	if err := EnsureWorkspaceCLAUDEMD(wsPath); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	content, err := os.ReadFile(filepath.Join(wsPath, ClaudeMDFileName))
	if err != nil {
		t.Fatal(err)
	}
	if string(content) != customContent {
		t.Fatalf("expected existing content to be preserved, got %q", string(content))
	}
}

// --- Locking tests ---

func TestLockWorkspace_AcquiresLock(t *testing.T) {
	wsPath := t.TempDir()

	lock, err := LockWorkspace(wsPath)
	if err != nil {
		t.Fatalf("failed to acquire lock: %v", err)
	}
	defer lock.Close()

	if !IsWorkspaceLocked(wsPath) {
		t.Fatal("expected workspace to be locked")
	}
}

func TestLockWorkspace_ReleasedAfterClose(t *testing.T) {
	wsPath := t.TempDir()

	lock, err := LockWorkspace(wsPath)
	if err != nil {
		t.Fatalf("failed to acquire lock: %v", err)
	}
	lock.Close()

	if IsWorkspaceLocked(wsPath) {
		t.Fatal("expected workspace to be unlocked after closing")
	}
}

func TestIsWorkspaceLocked_NonExistentLockFile(t *testing.T) {
	wsPath := t.TempDir()
	if IsWorkspaceLocked(wsPath) {
		t.Fatal("expected workspace to not be locked (no lock file)")
	}
}

// --- Session working directory tests ---

func TestGetSessionWorkingDirectory_SingleRepo(t *testing.T) {
	wsPath := t.TempDir()
	if err := os.MkdirAll(filepath.Join(wsPath, "my-repo"), 0o755); err != nil {
		t.Fatal(err)
	}

	result, err := GetSessionWorkingDirectory(wsPath)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	expected := filepath.Join(wsPath, "my-repo")
	if result != expected {
		t.Fatalf("expected %q, got %q", expected, result)
	}
}

func TestGetSessionWorkingDirectory_MultipleRepos(t *testing.T) {
	wsPath := t.TempDir()
	for _, repo := range []string{"repo-a", "repo-b"} {
		if err := os.MkdirAll(filepath.Join(wsPath, repo), 0o755); err != nil {
			t.Fatal(err)
		}
	}

	result, err := GetSessionWorkingDirectory(wsPath)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result != wsPath {
		t.Fatalf("expected workspace path %q, got %q", wsPath, result)
	}
}

func TestGetSessionWorkingDirectory_ZeroRepos(t *testing.T) {
	wsPath := t.TempDir()

	result, err := GetSessionWorkingDirectory(wsPath)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result != wsPath {
		t.Fatalf("expected workspace path %q, got %q", wsPath, result)
	}
}

// --- ResolveWorkspaceFromCWD tests ---

func TestResolveWorkspaceFromCWD_InsideWorkspace(t *testing.T) {
	tmpDir := setupConfigTest(t)
	profileDir := filepath.Join(tmpDir, "profile")
	createProfile(t, profileDir, map[string]any{"name": "test"})

	writeGlobalConfig(t, tmpDir, map[string]any{
		"profiles": []any{profileDir},
	})

	workspacesRoot := filepath.Join(profileDir, "workspaces")
	wsPath := makeWorkspace(t, workspacesRoot, "ws-1", true, false, []string{"repo-a"})

	cwd := filepath.Join(wsPath, "repo-a")
	profile, resolvedWsPath, err := ResolveWorkspaceFromCWD(cwd)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if profile == nil {
		t.Fatal("expected a profile, got nil")
	}
	if profile.Name != "test" {
		t.Fatalf("expected profile name 'test', got %q", profile.Name)
	}
	// Compare resolved paths to handle symlinks
	expectedWsPath, _ := filepath.EvalSymlinks(wsPath)
	resolvedResult, _ := filepath.EvalSymlinks(resolvedWsPath)
	if resolvedResult != expectedWsPath {
		t.Fatalf("expected workspace path %q, got %q", expectedWsPath, resolvedResult)
	}
}

func TestResolveWorkspaceFromCWD_NotInsideAnyWorkspace(t *testing.T) {
	tmpDir := setupConfigTest(t)
	profileDir := filepath.Join(tmpDir, "profile")
	createProfile(t, profileDir, map[string]any{"name": "test"})

	writeGlobalConfig(t, tmpDir, map[string]any{
		"profiles": []any{profileDir},
	})

	profile, wsPath, err := ResolveWorkspaceFromCWD("/some/random/path")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if profile != nil {
		t.Fatal("expected nil profile")
	}
	if wsPath != "" {
		t.Fatalf("expected empty workspace path, got %q", wsPath)
	}
}

func TestResolveWorkspaceFromCWD_InsideWorkspacesRootButNotSpecificWorkspace(t *testing.T) {
	tmpDir := setupConfigTest(t)
	profileDir := filepath.Join(tmpDir, "profile")
	createProfile(t, profileDir, map[string]any{"name": "test"})

	writeGlobalConfig(t, tmpDir, map[string]any{
		"profiles": []any{profileDir},
	})

	workspacesRoot := filepath.Join(profileDir, "workspaces")

	// CWD is the workspaces root itself, not inside a specific ws-N
	profile, wsPath, err := ResolveWorkspaceFromCWD(workspacesRoot)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if profile != nil {
		t.Fatal("expected nil profile when at workspaces root")
	}
	if wsPath != "" {
		t.Fatalf("expected empty workspace path, got %q", wsPath)
	}
}

func TestResolveWorkspaceFromCWD_InsideUninitializedWorkspace(t *testing.T) {
	tmpDir := setupConfigTest(t)
	profileDir := filepath.Join(tmpDir, "profile")
	createProfile(t, profileDir, map[string]any{"name": "test"})

	writeGlobalConfig(t, tmpDir, map[string]any{
		"profiles": []any{profileDir},
	})

	workspacesRoot := filepath.Join(profileDir, "workspaces")
	wsPath := makeWorkspace(t, workspacesRoot, "ws-1", false, false, nil) // not initialized

	profile, resolvedPath, err := ResolveWorkspaceFromCWD(wsPath)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if profile != nil {
		t.Fatal("expected nil profile for uninitialized workspace")
	}
	if resolvedPath != "" {
		t.Fatalf("expected empty path, got %q", resolvedPath)
	}
}

// --- DeleteWorkspace test (basic, no git worktrees) ---

func TestDeleteWorkspace_RemovesDirectory(t *testing.T) {
	root := t.TempDir()
	wsPath := makeWorkspace(t, root, "ws-1", true, true, nil)

	err := DeleteWorkspace(wsPath)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if _, err := os.Stat(wsPath); !os.IsNotExist(err) {
		t.Fatal("expected workspace directory to be deleted")
	}
}

// --- Deactivation tests ---

func TestDeactivateWorkspace_RemovesTaskFile(t *testing.T) {
	root := t.TempDir()
	wsPath := makeWorkspace(t, root, "ws-1", true, true, nil)

	err := DeactivateWorkspace(wsPath)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if HasTask(wsPath) {
		t.Fatal("expected task.json to be removed after deactivation")
	}
	if !IsInactive(wsPath) {
		t.Fatal("expected workspace to be inactive after deactivation")
	}
}

func TestDeactivateWorkspace_AlreadyInactive(t *testing.T) {
	root := t.TempDir()
	wsPath := makeWorkspace(t, root, "ws-1", true, false, nil)

	err := DeactivateWorkspace(wsPath)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if !IsInactive(wsPath) {
		t.Fatal("expected workspace to remain inactive")
	}
}

func TestDeactivateWorkspace_PreservesWorktrees(t *testing.T) {
	root := t.TempDir()
	wsPath := makeWorkspace(t, root, "ws-1", true, true, []string{"repo-a", "repo-b"})

	err := DeactivateWorkspace(wsPath)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	repos, err := GetWorkspaceRepos(wsPath)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	sort.Strings(repos)
	expected := []string{"repo-a", "repo-b"}
	if len(repos) != len(expected) {
		t.Fatalf("expected repos %v, got %v", expected, repos)
	}
	for i, r := range repos {
		if r != expected[i] {
			t.Fatalf("expected repos %v, got %v", expected, repos)
		}
	}
}

func TestDeactivateWorkspace_PreservesInitializedMarker(t *testing.T) {
	root := t.TempDir()
	wsPath := makeWorkspace(t, root, "ws-1", true, true, nil)

	err := DeactivateWorkspace(wsPath)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if !IsInitialized(wsPath) {
		t.Fatal("expected initialized marker to be preserved")
	}
}

// --- Constants test ---

func TestConstants(t *testing.T) {
	if LockFileName != ".creation-lock" {
		t.Fatalf("unexpected LockFileName: %q", LockFileName)
	}
	if InitializedMarkerName != "initialized" {
		t.Fatalf("unexpected InitializedMarkerName: %q", InitializedMarkerName)
	}
	if TaskFileName != "task.json" {
		t.Fatalf("unexpected TaskFileName: %q", TaskFileName)
	}
	if ClaudeMDFileName != "CLAUDE.md" {
		t.Fatalf("unexpected ClaudeMDFileName: %q", ClaudeMDFileName)
	}
	if WorkspacePrefix != "ws-" {
		t.Fatalf("unexpected WorkspacePrefix: %q", WorkspacePrefix)
	}
	if !strings.Contains(WorkspaceCLAUDEMDContent, "workspace with multiple repositories") {
		t.Fatal("unexpected WorkspaceCLAUDEMDContent")
	}
}

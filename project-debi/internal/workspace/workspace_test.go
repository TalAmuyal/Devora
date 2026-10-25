package workspace

import (
	"encoding/json"
	"os"
	"path/filepath"
	"sort"
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

func makeWorkspace(t *testing.T, workspacesRoot string, name string, initialized bool, repos []string) string {
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
	for _, repo := range repos {
		if err := os.MkdirAll(filepath.Join(wsPath, repo), 0o755); err != nil {
			t.Fatalf("failed to create repo dir: %v", err)
		}
	}
	return wsPath
}

// --- Enumeration tests ---

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

// --- ResolveWorkspaceFromCWD tests ---

func TestResolveWorkspaceFromCWD_InsideWorkspace(t *testing.T) {
	tmpDir := setupConfigTest(t)
	profileDir := filepath.Join(tmpDir, "profile")
	createProfile(t, profileDir, map[string]any{"name": "test"})

	writeGlobalConfig(t, tmpDir, map[string]any{
		"profiles": []any{profileDir},
	})

	workspacesRoot := filepath.Join(profileDir, "workspaces")
	wsPath := makeWorkspace(t, workspacesRoot, "ws-1", true, []string{"repo-a"})

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
	wsPath := makeWorkspace(t, workspacesRoot, "ws-1", false, nil) // not initialized

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

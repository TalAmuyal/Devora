package workspace

import (
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
)

// --- Test helpers for git operations ---

func runGit(t *testing.T, cwd string, args ...string) string {
	t.Helper()
	cmd := exec.Command("git", args...)
	cmd.Dir = cwd
	out, err := cmd.CombinedOutput()
	if err != nil {
		t.Fatalf("git %v failed in %s: %v\nOutput: %s", args, cwd, err, string(out))
	}
	return strings.TrimSpace(string(out))
}

func initGitRepo(t *testing.T, repoPath string) {
	t.Helper()
	if err := os.MkdirAll(repoPath, 0o755); err != nil {
		t.Fatalf("failed to create repo dir: %v", err)
	}
	runGit(t, repoPath, "init")
	runGit(t, repoPath, "config", "user.email", "test@test.com")
	runGit(t, repoPath, "config", "user.name", "Test")
	// Create an initial commit
	dummyFile := filepath.Join(repoPath, "README.md")
	if err := os.WriteFile(dummyFile, []byte("# Test Repo\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	runGit(t, repoPath, "add", ".")
	runGit(t, repoPath, "commit", "-m", "initial commit")
}

// --- GetRepoBranch tests ---

func TestGetRepoBranch_OnBranch(t *testing.T) {
	repoPath := filepath.Join(t.TempDir(), "repo")
	initGitRepo(t, repoPath)

	// Default branch after git init
	branch, err := GetRepoBranch(repoPath)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	// Could be "main" or "master" depending on git config
	if branch != "main" && branch != "master" {
		t.Fatalf("expected 'main' or 'master', got %q", branch)
	}
}

func TestGetRepoBranch_DetachedHead(t *testing.T) {
	repoPath := filepath.Join(t.TempDir(), "repo")
	initGitRepo(t, repoPath)

	// Detach HEAD
	commitHash := runGit(t, repoPath, "rev-parse", "HEAD")
	runGit(t, repoPath, "checkout", "--detach", commitHash)

	branch, err := GetRepoBranch(repoPath)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if branch != "HEAD" {
		t.Fatalf("expected 'HEAD', got %q", branch)
	}
}

// --- IsRepoClean tests ---

func TestIsRepoClean_CleanRepo(t *testing.T) {
	repoPath := filepath.Join(t.TempDir(), "repo")
	initGitRepo(t, repoPath)

	clean, err := IsRepoClean(repoPath)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !clean {
		t.Fatal("expected repo to be clean")
	}
}

func TestIsRepoClean_DirtyRepo(t *testing.T) {
	repoPath := filepath.Join(t.TempDir(), "repo")
	initGitRepo(t, repoPath)

	// Make a change
	if err := os.WriteFile(filepath.Join(repoPath, "dirty.txt"), []byte("dirty"), 0o644); err != nil {
		t.Fatal(err)
	}

	clean, err := IsRepoClean(repoPath)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if clean {
		t.Fatal("expected repo to be dirty")
	}
}

// --- IsGitRepo tests ---

func TestIsGitRepo_GitDirectory(t *testing.T) {
	repoPath := filepath.Join(t.TempDir(), "repo")
	initGitRepo(t, repoPath)

	if !IsGitRepo(repoPath) {
		t.Fatal("expected path to be a git repo")
	}
}

func TestIsGitRepo_NonGitDirectory(t *testing.T) {
	nonGitDir := t.TempDir()

	if IsGitRepo(nonGitDir) {
		t.Fatal("expected path to not be a git repo")
	}
}

package git_test

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"devora/internal/git"
)

func TestChildRepos_ReturnsOnlyRepoDirsSorted(t *testing.T) {
	base := t.TempDir()

	// Normal repo: .git is a directory.
	if err := os.MkdirAll(filepath.Join(base, "beta", ".git"), 0o755); err != nil {
		t.Fatal(err)
	}
	// Worktree/submodule checkout: .git is a file.
	if err := os.MkdirAll(filepath.Join(base, "alpha"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(base, "alpha", ".git"), []byte("gitdir: elsewhere\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	// Plain directory without .git: not a repo.
	if err := os.MkdirAll(filepath.Join(base, "notes"), 0o755); err != nil {
		t.Fatal(err)
	}
	// Plain file: not a repo.
	if err := os.WriteFile(filepath.Join(base, "README.md"), []byte("hi"), 0o644); err != nil {
		t.Fatal(err)
	}
	// Hidden directory with .git: skipped.
	if err := os.MkdirAll(filepath.Join(base, ".hidden", ".git"), 0o755); err != nil {
		t.Fatal(err)
	}

	repos, err := git.ChildRepos(base)
	if err != nil {
		t.Fatalf("ChildRepos: %v", err)
	}
	if strings.Join(repos, ",") != "alpha,beta" {
		t.Fatalf("ChildRepos = %v, want [alpha beta]", repos)
	}
}

func TestChildRepos_EmptyDir_ReturnsNoRepos(t *testing.T) {
	repos, err := git.ChildRepos(t.TempDir())
	if err != nil {
		t.Fatalf("ChildRepos: %v", err)
	}
	if len(repos) != 0 {
		t.Fatalf("ChildRepos = %v, want empty", repos)
	}
}

func TestChildRepos_MissingDir_ReturnsError(t *testing.T) {
	_, err := git.ChildRepos(filepath.Join(t.TempDir(), "does-not-exist"))
	if err == nil {
		t.Fatal("expected error for missing directory")
	}
}

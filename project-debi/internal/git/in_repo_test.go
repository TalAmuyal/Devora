package git_test

import (
	"errors"
	"os"
	"os/exec"
	"testing"

	"devora/internal/git"
	"devora/internal/process"
)

func TestEnsureInRepo_NotInRepoReturnsSentinel(t *testing.T) {
	dir := t.TempDir()

	err := git.EnsureInRepo(process.WithCwd(dir))
	if err == nil {
		t.Fatal("expected error outside a git repository")
	}
	if !errors.Is(err, git.ErrNotInGitRepo) {
		t.Fatalf("expected ErrNotInGitRepo, got: %v", err)
	}
}

func TestEnsureInRepo_InRepoReturnsNil(t *testing.T) {
	dir := t.TempDir()

	cmd := exec.Command("git", "init", "-b", "main")
	cmd.Dir = dir
	cmd.Env = append(os.Environ(),
		"GIT_AUTHOR_NAME=test", "GIT_AUTHOR_EMAIL=test@test.com",
		"GIT_COMMITTER_NAME=test", "GIT_COMMITTER_EMAIL=test@test.com",
		"GIT_CONFIG_GLOBAL=/dev/null", "GIT_CONFIG_SYSTEM=/dev/null",
	)
	if out, err := cmd.CombinedOutput(); err != nil {
		t.Fatalf("git init failed: %v\n%s", err, out)
	}

	if err := git.EnsureInRepo(process.WithCwd(dir)); err != nil {
		t.Fatalf("expected nil inside a git repository, got: %v", err)
	}
}

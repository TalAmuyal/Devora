package git_test

import (
	"os"
	"os/exec"
	"testing"

	"devora/internal/git"
	"devora/internal/process"
)

func TestDefaultBranchName(t *testing.T) {
	dir := setupGitRepo(t)

	branch, err := git.DefaultBranchName(process.WithCwd(dir))
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if branch != "main" {
		t.Fatalf("expected %q, got %q", "main", branch)
	}
}

func setupGitRepo(t *testing.T) string {
	t.Helper()
	dir := t.TempDir()

	commands := [][]string{
		{"git", "init", "-b", "main"},
		{"git", "commit", "--allow-empty", "-m", "initial"},
		{"git", "remote", "add", "origin", dir},
		{"git", "fetch", "origin"},
		{"git", "symbolic-ref", "refs/remotes/origin/HEAD", "refs/remotes/origin/main"},
	}
	for _, args := range commands {
		cmd := exec.Command(args[0], args[1:]...)
		cmd.Dir = dir
		cmd.Env = append(os.Environ(),
			"GIT_AUTHOR_NAME=test", "GIT_AUTHOR_EMAIL=test@test.com",
			"GIT_COMMITTER_NAME=test", "GIT_COMMITTER_EMAIL=test@test.com",
		)
		out, err := cmd.CombinedOutput()
		if err != nil {
			t.Fatalf("setup command %v failed: %v\n%s", args, err, out)
		}
	}
	return dir
}

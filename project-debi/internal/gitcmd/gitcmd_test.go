package gitcmd

import (
	"bytes"
	"errors"
	"io"
	"os"
	"strings"
	"testing"

	"devora/internal/git"
	"devora/internal/process"
)

func stubPassthrough(t *testing.T) *[]string {
	t.Helper()
	var captured []string
	orig := runPassthrough
	runPassthrough = func(command []string, opts ...process.ExecOption) error {
		captured = command
		return nil
	}
	t.Cleanup(func() { runPassthrough = orig })
	return &captured
}

func unsetGitDir(t *testing.T) {
	t.Helper()
	t.Setenv("GIT_DIR", "") // register the restore
	os.Unsetenv("GIT_DIR")
}

func stubEnsureInRepo(t *testing.T, err error) {
	t.Helper()
	orig := ensureInRepo
	ensureInRepo = func(opts ...process.ExecOption) error { return err }
	t.Cleanup(func() { ensureInRepo = orig })
}

func stubChildRepos(t *testing.T, repos []string, err error) {
	t.Helper()
	orig := childRepos
	childRepos = func(base string) ([]string, error) { return repos, err }
	t.Cleanup(func() { childRepos = orig })
}

func stubFanOut(t *testing.T) *struct {
	args  []string
	repos []string
} {
	t.Helper()
	captured := &struct {
		args  []string
		repos []string
	}{}
	orig := runFanOut
	runFanOut = func(w io.Writer, args, repos []string) error {
		captured.args = args
		captured.repos = repos
		return nil
	}
	t.Cleanup(func() { runFanOut = orig })
	return captured
}

func TestRun_EmptyArgs_ReturnsUsageListingCustoms(t *testing.T) {
	var buf bytes.Buffer
	err := Run(&buf, nil)

	var bad *BadUsageError
	if !errors.As(err, &bad) {
		t.Fatalf("expected *BadUsageError, got %T: %v", err, err)
	}
	for _, want := range []string{"usage: debi git", "add-all-commit", "rebase-latest", "summary"} {
		if !strings.Contains(bad.Message, want) {
			t.Errorf("usage message missing %q:\n%s", want, bad.Message)
		}
	}
}

func TestRun_CustomSubcommand_Routed(t *testing.T) {
	// add-all-commit without a message proves custom routing without touching git
	var buf bytes.Buffer
	err := Run(&buf, []string{"add-all-commit"})

	var bad *BadUsageError
	if !errors.As(err, &bad) {
		t.Fatalf("expected *BadUsageError, got %T: %v", err, err)
	}
	if !strings.Contains(bad.Message, "usage: debi git add-all-commit <msg>") {
		t.Fatalf("unexpected message: %s", bad.Message)
	}
}

func TestRun_DashLeadingArg_PassesThrough(t *testing.T) {
	captured := stubPassthrough(t)
	stubEnsureInRepo(t, git.ErrNotInGitRepo)
	stubChildRepos(t, []string{"a"}, nil)

	var buf bytes.Buffer
	if err := Run(&buf, []string{"--version"}); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if strings.Join(*captured, " ") != "git --version" {
		t.Fatalf("passthrough command = %v, want [git --version]", *captured)
	}
}

func TestRun_GitDirEnv_PassesThrough(t *testing.T) {
	captured := stubPassthrough(t)
	stubEnsureInRepo(t, git.ErrNotInGitRepo)
	stubChildRepos(t, []string{"a"}, nil)
	t.Setenv("GIT_DIR", "/some/repo/.git")

	var buf bytes.Buffer
	if err := Run(&buf, []string{"status"}); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if strings.Join(*captured, " ") != "git status" {
		t.Fatalf("passthrough command = %v, want [git status]", *captured)
	}
}

func TestRun_NonWhitelistedSubcommand_PassesThrough(t *testing.T) {
	captured := stubPassthrough(t)
	stubEnsureInRepo(t, git.ErrNotInGitRepo)
	stubChildRepos(t, []string{"a"}, nil)
	t.Setenv("GIT_DIR", "")

	var buf bytes.Buffer
	if err := Run(&buf, []string{"push", "origin"}); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if strings.Join(*captured, " ") != "git push origin" {
		t.Fatalf("passthrough command = %v, want [git push origin]", *captured)
	}
}

func TestRun_InsideRepo_WhitelistedSubcommand_PassesThrough(t *testing.T) {
	captured := stubPassthrough(t)
	stubEnsureInRepo(t, nil)
	stubChildRepos(t, []string{"a"}, nil)
	t.Setenv("GIT_DIR", "")

	var buf bytes.Buffer
	if err := Run(&buf, []string{"status", "-sb"}); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if strings.Join(*captured, " ") != "git status -sb" {
		t.Fatalf("passthrough command = %v, want [git status -sb]", *captured)
	}
}

func TestRun_NotInRepo_NoChildRepos_PassesThrough(t *testing.T) {
	captured := stubPassthrough(t)
	stubEnsureInRepo(t, git.ErrNotInGitRepo)
	stubChildRepos(t, nil, nil)
	t.Setenv("GIT_DIR", "")

	var buf bytes.Buffer
	if err := Run(&buf, []string{"status"}); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if strings.Join(*captured, " ") != "git status" {
		t.Fatalf("passthrough command = %v, want [git status]", *captured)
	}
}

func TestRun_ChildReposError_PassesThrough(t *testing.T) {
	captured := stubPassthrough(t)
	stubEnsureInRepo(t, git.ErrNotInGitRepo)
	stubChildRepos(t, nil, errors.New("boom"))
	t.Setenv("GIT_DIR", "")

	var buf bytes.Buffer
	if err := Run(&buf, []string{"status"}); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if strings.Join(*captured, " ") != "git status" {
		t.Fatalf("passthrough command = %v, want [git status]", *captured)
	}
}

func TestRun_NotInRepo_WithChildRepos_FansOut(t *testing.T) {
	captured := stubPassthrough(t)
	stubEnsureInRepo(t, git.ErrNotInGitRepo)
	stubChildRepos(t, []string{"a", "b"}, nil)
	fanned := stubFanOut(t)
	t.Setenv("GIT_DIR", "")

	var buf bytes.Buffer
	if err := Run(&buf, []string{"log", "-1"}); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if *captured != nil {
		t.Fatalf("expected no passthrough, got %v", *captured)
	}
	if strings.Join(fanned.args, " ") != "log -1" {
		t.Fatalf("fan-out args = %v, want [log -1]", fanned.args)
	}
	if strings.Join(fanned.repos, " ") != "a b" {
		t.Fatalf("fan-out repos = %v, want [a b]", fanned.repos)
	}
}

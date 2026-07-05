package gitcmd

import (
	"bytes"
	"errors"
	"io"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"devora/internal/git"
	"devora/internal/process"
	"devora/internal/workspace/wsgit"
)

func initGitRepo(t *testing.T, path string) {
	t.Helper()
	if err := os.MkdirAll(path, 0o755); err != nil {
		t.Fatal(err)
	}
	run := func(args ...string) {
		t.Helper()
		if _, err := process.GetOutput(append([]string{"git"}, args...), process.WithCwd(path)); err != nil {
			t.Fatalf("git %v: %v", args, err)
		}
	}
	run("init", "-b", "main")
	run("config", "user.email", "test@example.com")
	run("config", "user.name", "Test")
	if err := os.WriteFile(filepath.Join(path, "initial.txt"), []byte("initial"), 0o644); err != nil {
		t.Fatal(err)
	}
	run("add", ".")
	run("commit", "-m", "initial")
}

// silenceStdio routes the process's stdout/stderr to /dev/null for the test: the git compositions run children wired to os.Stdout, and their chatter must not pollute test output
func silenceStdio(t *testing.T) {
	t.Helper()
	devnull, err := os.OpenFile(os.DevNull, os.O_WRONLY, 0)
	if err != nil {
		t.Fatal(err)
	}
	origOut, origErr := os.Stdout, os.Stderr
	os.Stdout, os.Stderr = devnull, devnull
	t.Cleanup(func() {
		os.Stdout, os.Stderr = origOut, origErr
		devnull.Close()
	})
}

func stubWorkspaceRoot(t *testing.T, wsPath string, err error) {
	t.Helper()
	orig := wsgitEnsureAtWorkspaceRoot
	wsgitEnsureAtWorkspaceRoot = func(cwd string) (string, error) { return wsPath, err }
	t.Cleanup(func() { wsgitEnsureAtWorkspaceRoot = orig })
}

func TestRun_AddAllCommit_CommitsEverythingWithJoinedMessage(t *testing.T) {
	repo := filepath.Join(t.TempDir(), "repo")
	initGitRepo(t, repo)
	t.Chdir(repo)
	silenceStdio(t)
	if err := os.WriteFile(filepath.Join(repo, "untracked.txt"), []byte("new"), 0o644); err != nil {
		t.Fatal(err)
	}

	var buf bytes.Buffer
	if err := Run(&buf, []string{"add-all-commit", "hello", "world"}); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	subject, err := process.GetOutput([]string{"git", "log", "-1", "--format=%s"}, process.WithCwd(repo))
	if err != nil {
		t.Fatal(err)
	}
	if subject != "hello world" {
		t.Fatalf("commit subject = %q, want %q", subject, "hello world")
	}
	status, err := process.GetOutput([]string{"git", "status", "--porcelain"}, process.WithCwd(repo))
	if err != nil {
		t.Fatal(err)
	}
	if status != "" {
		t.Fatalf("expected clean tree after add-all-commit, got %q", status)
	}
}

func TestRun_AddAllCommitPush_MissingMessage_UsageError(t *testing.T) {
	var buf bytes.Buffer
	err := Run(&buf, []string{"add-all-commit-push"})

	var bad *BadUsageError
	if !errors.As(err, &bad) {
		t.Fatalf("expected *BadUsageError, got %T: %v", err, err)
	}
	if !strings.Contains(bad.Message, "usage: debi git add-all-commit-push <msg>") {
		t.Fatalf("unexpected message: %s", bad.Message)
	}
}

func TestRun_NoArgCustoms_RejectExtraArgs(t *testing.T) {
	for _, name := range []string{
		"add-all-amend",
		"add-all-amend-push",
		"branch-delete-current",
		"checkout-latest",
		"rebase-origin-default",
		"summary",
	} {
		var buf bytes.Buffer
		err := Run(&buf, []string{name, "extra"})

		var bad *BadUsageError
		if !errors.As(err, &bad) {
			t.Fatalf("%s: expected *BadUsageError, got %T: %v", name, err, err)
		}
		if !strings.Contains(bad.Message, "usage: debi git "+name) {
			t.Fatalf("%s: unexpected message: %s", name, bad.Message)
		}
	}
}

func TestRun_RepoGatedCustoms_OutsideRepo_UsageError(t *testing.T) {
	t.Chdir(t.TempDir())
	stubWorkspaceRoot(t, "", wsgit.ErrNotAtWorkspaceRoot)

	for _, args := range [][]string{
		{"branch-delete-current"},
		{"checkout-latest"},
		{"checkout-origin-default"},
		{"rebase-interactive"},
		{"rebase-latest"},
		{"rebase-origin-default"},
	} {
		var buf bytes.Buffer
		err := Run(&buf, args)

		var bad *BadUsageError
		if !errors.As(err, &bad) {
			t.Fatalf("%v: expected *BadUsageError, got %T: %v", args, err, err)
		}
		if bad.Message != git.NotInRepoMessage {
			t.Fatalf("%v: message = %q, want %q", args, bad.Message, git.NotInRepoMessage)
		}
	}
}

func TestRun_RebaseLatest_UnknownFlag_UsageError(t *testing.T) {
	var buf bytes.Buffer
	err := Run(&buf, []string{"rebase-latest", "--bogus"})

	var bad *BadUsageError
	if !errors.As(err, &bad) {
		t.Fatalf("expected *BadUsageError, got %T: %v", err, err)
	}
	if !strings.Contains(bad.Message, "usage: debi git rebase-latest [--push]") {
		t.Fatalf("unexpected message: %s", bad.Message)
	}
}

func TestRun_CheckoutLatest_AtWorkspaceRoot_RunsClean(t *testing.T) {
	stubWorkspaceRoot(t, "/ws/path", nil)
	var cleanedPath string
	origClean := wsgitRunClean
	wsgitRunClean = func(w io.Writer, wsPath string) error {
		cleanedPath = wsPath
		return nil
	}
	t.Cleanup(func() { wsgitRunClean = origClean })

	var buf bytes.Buffer
	if err := Run(&buf, []string{"checkout-latest"}); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if cleanedPath != "/ws/path" {
		t.Fatalf("RunClean path = %q, want %q", cleanedPath, "/ws/path")
	}
}

func TestRun_CheckoutLatest_NotAtRoot_FallsBackToSingleRepo(t *testing.T) {
	stubWorkspaceRoot(t, "", wsgit.ErrNotAtWorkspaceRoot)
	stubEnsureInRepo(t, nil)
	called := false
	origCheckout := gitCheckoutLatest
	gitCheckoutLatest = func() error {
		called = true
		return nil
	}
	t.Cleanup(func() { gitCheckoutLatest = origCheckout })

	var buf bytes.Buffer
	if err := Run(&buf, []string{"checkout-latest"}); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !called {
		t.Fatal("expected single-repo checkout-latest to run")
	}
}

func TestRun_Summary_NoChildRepos_UsageError(t *testing.T) {
	t.Chdir(t.TempDir())

	var buf bytes.Buffer
	err := Run(&buf, []string{"summary"})

	var bad *BadUsageError
	if !errors.As(err, &bad) {
		t.Fatalf("expected *BadUsageError, got %T: %v", err, err)
	}
	if !strings.Contains(bad.Message, "child") {
		t.Fatalf("unexpected message: %s", bad.Message)
	}
}

func TestRun_Summary_WithChildRepos_RendersStatusTable(t *testing.T) {
	stubChildRepos(t, []string{"a"}, nil)
	var statusDir string
	origStatus := wsgitRunStatusDir
	wsgitRunStatusDir = func(w io.Writer, dir string) error {
		statusDir = dir
		return nil
	}
	t.Cleanup(func() { wsgitRunStatusDir = origStatus })

	var buf bytes.Buffer
	if err := Run(&buf, []string{"summary"}); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	cwd, err := os.Getwd()
	if err != nil {
		t.Fatal(err)
	}
	if statusDir != cwd {
		t.Fatalf("RunStatusDir dir = %q, want cwd %q", statusDir, cwd)
	}
}

func TestRun_CustomHelp_ShortCircuits(t *testing.T) {
	// Run in a non-repo dir: if -h didn't short-circuit, these customs would invoke git (or hit the repo gate) and fail
	t.Chdir(t.TempDir())

	for _, name := range []string{"add-all-amend", "rebase-interactive"} {
		for _, flag := range []string{"-h", "--help"} {
			var buf bytes.Buffer
			if err := Run(&buf, []string{name, flag}); err != nil {
				t.Fatalf("%s %s: unexpected error: %v", name, flag, err)
			}
			out := buf.String()
			if !strings.Contains(out, "usage: debi git "+name) {
				t.Fatalf("%s %s: help output missing usage line:\n%s", name, flag, out)
			}
		}
	}
}

func TestSubCommandInfos_MatchesCustomRegistry(t *testing.T) {
	infos := SubCommandInfos()
	if len(infos) != len(customCommands) {
		t.Fatalf("SubCommandInfos len = %d, want %d", len(infos), len(customCommands))
	}
	for i, c := range customCommands {
		if infos[i].Name != c.info.Name {
			t.Fatalf("info[%d].Name = %q, want %q", i, infos[i].Name, c.info.Name)
		}
		if infos[i].Description == "" {
			t.Fatalf("custom %q has no description", c.info.Name)
		}
	}
}

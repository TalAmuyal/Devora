package gitcmd

import (
	"bytes"
	"errors"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"devora/internal/process"
)

func stubStdoutIsTTY(t *testing.T, isTTY bool) {
	t.Helper()
	orig := stdoutIsTTY
	stdoutIsTTY = func() bool { return isTTY }
	t.Cleanup(func() { stdoutIsTTY = orig })
}

func TestFanOut_RunsInEachChildRepoInOrder(t *testing.T) {
	parent := t.TempDir()
	initGitRepo(t, filepath.Join(parent, "beta"))
	initGitRepo(t, filepath.Join(parent, "alpha"))
	if err := os.MkdirAll(filepath.Join(parent, "notes"), 0o755); err != nil {
		t.Fatal(err)
	}
	t.Chdir(parent)
	unsetGitDir(t)
	stubStdoutIsTTY(t, false)

	var buf bytes.Buffer
	if err := Run(&buf, []string{"status"}); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	out := buf.String()
	idxAlpha := strings.Index(out, "## alpha")
	idxBeta := strings.Index(out, "## beta")
	if idxAlpha < 0 || idxBeta < 0 {
		t.Fatalf("expected headers for both repos, got:\n%s", out)
	}
	if idxAlpha > idxBeta {
		t.Fatalf("expected alpha section before beta, got:\n%s", out)
	}
	if strings.Contains(out, "notes") {
		t.Fatalf("expected non-repo dir to be skipped, got:\n%s", out)
	}
	if strings.Count(out, "On branch main") != 2 {
		t.Fatalf("expected git status output for both repos, got:\n%s", out)
	}
}

func TestFanOut_FailingRepo_PrintsAllAndPropagatesExitCode(t *testing.T) {
	parent := t.TempDir()
	initGitRepo(t, filepath.Join(parent, "a-good"))
	// A repo without commits: `git log` fails with exit 128.
	emptyRepo := filepath.Join(parent, "b-empty")
	if err := os.MkdirAll(emptyRepo, 0o755); err != nil {
		t.Fatal(err)
	}
	if _, err := process.GetOutput([]string{"git", "init", "-b", "main"}, process.WithCwd(emptyRepo)); err != nil {
		t.Fatal(err)
	}
	t.Chdir(parent)
	unsetGitDir(t)
	stubStdoutIsTTY(t, false)

	var buf bytes.Buffer
	err := Run(&buf, []string{"log", "--oneline"})

	var ptErr *process.PassthroughError
	if !errors.As(err, &ptErr) {
		t.Fatalf("expected *PassthroughError, got %T: %v", err, err)
	}
	if ptErr.Code != 128 {
		t.Fatalf("exit code = %d, want 128 (last nonzero)", ptErr.Code)
	}
	out := buf.String()
	if !strings.Contains(out, "## a-good") || !strings.Contains(out, "## b-empty") {
		t.Fatalf("expected headers for both repos even on failure, got:\n%s", out)
	}
	if !strings.Contains(out, "initial") {
		t.Fatalf("expected the good repo's log output, got:\n%s", out)
	}
	if !strings.Contains(out, "does not have any commits") {
		t.Fatalf("expected the empty repo's git error in its section, got:\n%s", out)
	}
}

func TestFanOut_GrepNoMatch_PropagatesExitOne(t *testing.T) {
	parent := t.TempDir()
	initGitRepo(t, filepath.Join(parent, "a"))
	initGitRepo(t, filepath.Join(parent, "b"))
	t.Chdir(parent)
	unsetGitDir(t)
	stubStdoutIsTTY(t, false)

	var buf bytes.Buffer
	err := Run(&buf, []string{"grep", "zzz-no-such-string"})

	var ptErr *process.PassthroughError
	if !errors.As(err, &ptErr) {
		t.Fatalf("expected *PassthroughError, got %T: %v", err, err)
	}
	if ptErr.Code != 1 {
		t.Fatalf("exit code = %d, want 1", ptErr.Code)
	}
	out := buf.String()
	if !strings.Contains(out, "## a") || !strings.Contains(out, "## b") {
		t.Fatalf("expected headers for both repos, got:\n%s", out)
	}
}

func TestFanOut_TTY_InjectsColorAlways(t *testing.T) {
	var mu sync.Mutex
	var captured [][]string
	origRun := runPassthrough
	runPassthrough = func(command []string, opts ...process.ExecOption) error {
		mu.Lock()
		captured = append(captured, command)
		mu.Unlock()
		return nil
	}
	t.Cleanup(func() { runPassthrough = origRun })

	stubStdoutIsTTY(t, true)
	var buf bytes.Buffer
	if err := fanOut(&buf, []string{"status"}, []string{"a"}); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(captured) != 1 {
		t.Fatalf("expected 1 child invocation, got %d", len(captured))
	}
	want := "git -c color.ui=always status"
	if strings.Join(captured[0], " ") != want {
		t.Fatalf("child command = %v, want %q", captured[0], want)
	}
}

func TestFanOut_NonTTY_NoColorInjection(t *testing.T) {
	var mu sync.Mutex
	var captured [][]string
	origRun := runPassthrough
	runPassthrough = func(command []string, opts ...process.ExecOption) error {
		mu.Lock()
		captured = append(captured, command)
		mu.Unlock()
		return nil
	}
	t.Cleanup(func() { runPassthrough = origRun })

	stubStdoutIsTTY(t, false)
	var buf bytes.Buffer
	if err := fanOut(&buf, []string{"status"}, []string{"a"}); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if strings.Join(captured[0], " ") != "git status" {
		t.Fatalf("child command = %v, want [git status]", captured[0])
	}
}

func TestFanOut_ConcurrencyIsCapped(t *testing.T) {
	var current, peak atomic.Int32
	origRun := runPassthrough
	runPassthrough = func(command []string, opts ...process.ExecOption) error {
		n := current.Add(1)
		for {
			p := peak.Load()
			if n <= p || peak.CompareAndSwap(p, n) {
				break
			}
		}
		defer current.Add(-1)
		// Hold the slot briefly so overlap is observable.
		time.Sleep(time.Millisecond)
		return nil
	}
	t.Cleanup(func() { runPassthrough = origRun })

	stubStdoutIsTTY(t, false)
	repos := make([]string, 32)
	for i := range repos {
		repos[i] = string(rune('a' + i%26))
	}
	var buf bytes.Buffer
	if err := fanOut(&buf, []string{"status"}, repos); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if peak.Load() > fanOutConcurrency {
		t.Fatalf("peak concurrency = %d, want <= %d", peak.Load(), fanOutConcurrency)
	}
}

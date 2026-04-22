package git_test

import (
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"

	"devora/internal/git"
	"devora/internal/process"
)

func TestGenerateBranchName(t *testing.T) {
	tests := []struct {
		name     string
		prefix   string
		taskName string
		want     string
	}{
		{
			name:     "simple ASCII task name",
			prefix:   "feat",
			taskName: "Add user login",
			want:     "feat-add-user-login",
		},
		{
			name:     "empty task name falls back to unnamed",
			prefix:   "feat",
			taskName: "",
			want:     "feat-unnamed",
		},
		{
			name:     "all non-ASCII falls back to unnamed",
			prefix:   "feat",
			taskName: "\U0001F600\U0001F389",
			want:     "feat-unnamed",
		},
		{
			name:     "long task name truncated to 70 chars with no trailing dash",
			prefix:   "feat",
			taskName: "A" + strings.Repeat("b", 100),
			want:     "feat-a" + strings.Repeat("b", 64), // "feat-a" = 6 chars, +64 b's = 70 total
		},
		{
			name:     "multiple consecutive dashes collapse to one",
			prefix:   "feat",
			taskName: "multi--dashes",
			want:     "feat-multi-dashes",
		},
		{
			name:     "leading and trailing whitespace trimmed",
			prefix:   "feat",
			taskName: "  leading trailing  ",
			want:     "feat-leading-trailing",
		},
		{
			name:     "empty prefix uses unnamed-style result",
			prefix:   "",
			taskName: "",
			want:     "unnamed",
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			got := git.GenerateBranchName(tc.prefix, tc.taskName)
			if got != tc.want {
				t.Fatalf("GenerateBranchName(%q, %q) = %q, want %q", tc.prefix, tc.taskName, got, tc.want)
			}
			if len(got) > 70 {
				t.Fatalf("result %q exceeds 70 chars (len=%d)", got, len(got))
			}
			if strings.HasSuffix(got, "-") {
				t.Fatalf("result %q has trailing dash", got)
			}
		})
	}
}

func TestIsProtectedBranch(t *testing.T) {
	tests := []struct {
		branch string
		want   bool
	}{
		{"main", true},
		{"master", true},
		{"develop", true},
		{"feat-foo", false},
		{"", false},
		{"mainline", false},
	}
	for _, tc := range tests {
		got := git.IsProtectedBranch(tc.branch)
		if got != tc.want {
			t.Errorf("IsProtectedBranch(%q) = %v, want %v", tc.branch, got, tc.want)
		}
	}
}

func TestCurrentBranchOrDetached_OnBranch(t *testing.T) {
	dir := setupGitRepo(t)

	branch, err := git.CurrentBranchOrDetached(process.WithCwd(dir))
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if branch != "main" {
		t.Fatalf("expected %q, got %q", "main", branch)
	}
}

func TestCurrentBranchOrDetached_Detached(t *testing.T) {
	dir := setupGitRepo(t)

	runGitSetup(t, dir, [][]string{
		{"git", "checkout", "--detach", "HEAD"},
	})

	branch, err := git.CurrentBranchOrDetached(process.WithCwd(dir))
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if branch != "" {
		t.Fatalf("expected empty string for detached HEAD, got %q", branch)
	}
}

func TestBranchConfig_RoundTrip(t *testing.T) {
	dir := setupGitRepo(t)

	// Missing key returns "" with no error.
	val, err := git.GetBranchConfig("main", "task-id", process.WithCwd(dir))
	if err != nil {
		t.Fatalf("unexpected error reading missing key: %v", err)
	}
	if val != "" {
		t.Fatalf("expected empty string for missing key, got %q", val)
	}

	// Round-trip: Set then Get. Use WithCwd for Get to target the repo;
	// SetBranchConfig uses passthrough in the current process CWD, so run
	// it via direct git to avoid depending on debi's passthrough for cwd.
	runGitSetup(t, dir, [][]string{
		{"git", "config", "--local", "branch.main.task-id", "12345"},
	})

	val, err = git.GetBranchConfig("main", "task-id", process.WithCwd(dir))
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if val != "12345" {
		t.Fatalf("expected %q, got %q", "12345", val)
	}
}

func TestSetBranchConfig_WritesValue(t *testing.T) {
	dir := setupGitRepo(t)

	// SetBranchConfig uses passthrough, which inherits the current process
	// CWD. Chdir into the temp repo for this test only.
	origWd, err := os.Getwd()
	if err != nil {
		t.Fatalf("getwd failed: %v", err)
	}
	if err := os.Chdir(dir); err != nil {
		t.Fatalf("chdir failed: %v", err)
	}
	t.Cleanup(func() {
		_ = os.Chdir(origWd)
	})

	if err := git.SetBranchConfig("main", "task-id", "abc-123"); err != nil {
		t.Fatalf("SetBranchConfig: %v", err)
	}

	val, err := git.GetBranchConfig("main", "task-id", process.WithCwd(dir))
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if val != "abc-123" {
		t.Fatalf("expected %q, got %q", "abc-123", val)
	}
}

func TestHasLocalBranch(t *testing.T) {
	dir := setupGitRepo(t)

	if !git.HasLocalBranch("main", process.WithCwd(dir)) {
		t.Fatalf("expected main to exist")
	}
	if git.HasLocalBranch("nonexistent-branch", process.WithCwd(dir)) {
		t.Fatalf("expected nonexistent-branch to not exist")
	}
}

func runGitSetup(t *testing.T, dir string, commands [][]string) {
	t.Helper()
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
}

// autoMergeKey is the git config key used by the repo-config helper tests.
// It mirrors the key `internal/submit` reads in production so the tests
// exercise the full round-trip.
const autoMergeKey = "devora.pr.auto-merge"

func TestRepoConfigBool_UnsetReturnsNil(t *testing.T) {
	dir := setupGitRepo(t)

	val, err := git.GetRepoConfigBool(autoMergeKey, process.WithCwd(dir))
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if val != nil {
		t.Fatalf("expected nil for unset key, got %v", *val)
	}
}

func TestRepoConfigBool_RoundTripTrue(t *testing.T) {
	dir := setupGitRepo(t)

	runGitSetup(t, dir, [][]string{
		{"git", "config", "--type=bool", "--local", autoMergeKey, "true"},
	})

	val, err := git.GetRepoConfigBool(autoMergeKey, process.WithCwd(dir))
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if val == nil {
		t.Fatal("expected non-nil result")
	}
	if *val != true {
		t.Fatalf("expected true, got %v", *val)
	}
}

func TestRepoConfigBool_RoundTripFalse(t *testing.T) {
	dir := setupGitRepo(t)

	runGitSetup(t, dir, [][]string{
		{"git", "config", "--type=bool", "--local", autoMergeKey, "false"},
	})

	val, err := git.GetRepoConfigBool(autoMergeKey, process.WithCwd(dir))
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if val == nil {
		t.Fatal("expected non-nil result")
	}
	if *val != false {
		t.Fatalf("expected false, got %v", *val)
	}
}

func TestRepoConfigBool_WrongTypeFallsThrough(t *testing.T) {
	dir := setupGitRepo(t)

	runGitSetup(t, dir, [][]string{
		{"git", "config", "--local", autoMergeKey, "maybe"},
	})

	val, err := git.GetRepoConfigBool(autoMergeKey, process.WithCwd(dir))
	if err != nil {
		t.Fatalf("expected nil error for wrong-type value, got: %v", err)
	}
	if val != nil {
		t.Fatalf("expected nil for wrong-type value, got %v", *val)
	}
}

func TestUnsetRepoConfig_ClearsValue(t *testing.T) {
	dir := setupGitRepo(t)

	runGitSetup(t, dir, [][]string{
		{"git", "config", "--type=bool", "--local", autoMergeKey, "true"},
	})

	if err := git.UnsetRepoConfig(autoMergeKey, process.WithCwd(dir)); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	val, err := git.GetRepoConfigBool(autoMergeKey, process.WithCwd(dir))
	if err != nil {
		t.Fatalf("unexpected error after unset: %v", err)
	}
	if val != nil {
		t.Fatalf("expected nil after unset, got %v", *val)
	}
}

func TestUnsetRepoConfig_IdempotentWhenAlreadyUnset(t *testing.T) {
	dir := setupGitRepo(t)

	if err := git.UnsetRepoConfig(autoMergeKey, process.WithCwd(dir)); err != nil {
		t.Fatalf("expected nil error when key is already unset, got: %v", err)
	}
}

// setRepoConfigBoolFromDir invokes SetRepoConfigBool with the process cwd
// temporarily pointing at dir. SetRepoConfigBool uses passthrough, which
// inherits the current process CWD; Chdir for the duration of the test only.
func setRepoConfigBoolFromDir(t *testing.T, dir, key string, value bool) {
	t.Helper()
	origWd, err := os.Getwd()
	if err != nil {
		t.Fatalf("getwd failed: %v", err)
	}
	if err := os.Chdir(dir); err != nil {
		t.Fatalf("chdir failed: %v", err)
	}
	defer func() {
		_ = os.Chdir(origWd)
	}()
	if err := git.SetRepoConfigBool(key, value); err != nil {
		t.Fatalf("SetRepoConfigBool: %v", err)
	}
}

func TestRepoConfigBool_SharedAcrossLinkedWorktrees(t *testing.T) {
	dir := setupGitRepo(t)

	linked := filepath.Join(t.TempDir(), "linked")
	runGitSetup(t, dir, [][]string{
		{"git", "worktree", "add", linked, "-b", "linked-branch"},
	})

	setRepoConfigBoolFromDir(t, dir, autoMergeKey, true)

	for _, path := range []string{dir, linked} {
		val, err := git.GetRepoConfigBool(autoMergeKey, process.WithCwd(path))
		if err != nil {
			t.Fatalf("unexpected error reading from %s: %v", path, err)
		}
		if val == nil {
			t.Fatalf("expected non-nil from %s", path)
		}
		if *val != true {
			t.Fatalf("expected true from %s, got %v", path, *val)
		}
	}
}

func TestRepoConfigBool_BareClone(t *testing.T) {
	src := setupGitRepo(t)

	bareDir := filepath.Join(t.TempDir(), "bare.git")
	runGitSetup(t, "", [][]string{
		{"git", "clone", "--bare", src, bareDir},
	})

	setRepoConfigBoolFromDir(t, bareDir, autoMergeKey, false)

	val, err := git.GetRepoConfigBool(autoMergeKey, process.WithCwd(bareDir))
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if val == nil {
		t.Fatal("expected non-nil result")
	}
	if *val != false {
		t.Fatalf("expected false, got %v", *val)
	}
}

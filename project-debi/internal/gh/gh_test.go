package gh

import (
	"errors"
	"os/exec"
	"strings"
	"testing"

	"devora/internal/process"
)

// stubGHDeps replaces the package-level runGH* vars for the duration of the
// test and restores them on cleanup.
func stubGHDeps(t *testing.T) {
	t.Helper()
	origGetOutput := runGHGetOutput
	origPassthrough := runGHPassthrough
	t.Cleanup(func() {
		runGHGetOutput = origGetOutput
		runGHPassthrough = origPassthrough
	})
}

// --- GetRepo ---

func TestGetRepo_ParsesPayload(t *testing.T) {
	stubGHDeps(t)

	runGHGetOutput = func(args []string) (string, error) {
		return `{
			"name": "devora",
			"owner": {"login": "TalAmuyal"},
			"defaultBranchRef": {"name": "main"}
		}`, nil
	}

	info, err := GetRepo()
	if err != nil {
		t.Fatalf("expected no error, got: %v", err)
	}
	if info.Owner != "TalAmuyal" {
		t.Fatalf("expected Owner 'TalAmuyal', got %q", info.Owner)
	}
	if info.Name != "devora" {
		t.Fatalf("expected Name 'devora', got %q", info.Name)
	}
	if info.DefaultBranch != "main" {
		t.Fatalf("expected DefaultBranch 'main', got %q", info.DefaultBranch)
	}
}

func TestGetRepo_MalformedJSON(t *testing.T) {
	stubGHDeps(t)

	runGHGetOutput = func(args []string) (string, error) {
		return `{"name": "devora"`, nil // truncated
	}

	_, err := GetRepo()
	if err == nil {
		t.Fatal("expected error for malformed JSON")
	}
	if !strings.Contains(err.Error(), "parse gh repo view") {
		t.Fatalf("expected error to mention 'parse gh repo view', got: %v", err)
	}
}

func TestGetRepo_GhNotInstalled(t *testing.T) {
	stubGHDeps(t)

	runGHGetOutput = func(args []string) (string, error) {
		return "", &process.VerboseExecError{
			Command: []string{"gh", "repo", "view"},
			Err:     exec.ErrNotFound,
		}
	}

	_, err := GetRepo()
	if err == nil {
		t.Fatal("expected error when gh is not installed")
	}
	if !errors.Is(err, ErrGHNotInstalled) {
		t.Fatalf("expected errors.Is(err, ErrGHNotInstalled) to be true, got: %v", err)
	}
}

// --- GetPRForBranch ---

func TestGetPRForBranch_NoPR(t *testing.T) {
	stubGHDeps(t)

	runGHGetOutput = func(args []string) (string, error) {
		return "", &process.VerboseExecError{
			Command: []string{"gh", "pr", "view", "foo"},
			Err:     &exec.ExitError{},
			Stderr:  `no pull requests found for branch "foo"`,
		}
	}

	pr, err := GetPRForBranch("foo")
	if err != nil {
		t.Fatalf("expected nil error for 'no pull requests found', got: %v", err)
	}
	if pr != nil {
		t.Fatalf("expected nil *PRSummary for 'no pull requests found', got: %+v", pr)
	}
}

func TestGetPRForBranch_ParsesPayload(t *testing.T) {
	stubGHDeps(t)

	runGHGetOutput = func(args []string) (string, error) {
		return `{
			"number": 42,
			"url": "https://github.com/org/repo/pull/42",
			"state": "OPEN",
			"isDraft": false,
			"title": "Add a thing"
		}`, nil
	}

	pr, err := GetPRForBranch("feature/x")
	if err != nil {
		t.Fatalf("expected no error, got: %v", err)
	}
	if pr == nil {
		t.Fatal("expected non-nil *PRSummary")
	}
	if pr.Number != 42 {
		t.Fatalf("expected Number 42, got %d", pr.Number)
	}
	if pr.URL != "https://github.com/org/repo/pull/42" {
		t.Fatalf("expected URL, got %q", pr.URL)
	}
	if pr.State != "OPEN" {
		t.Fatalf("expected State 'OPEN', got %q", pr.State)
	}
	if pr.IsDraft {
		t.Fatal("expected IsDraft false")
	}
	if pr.Merged {
		t.Fatal("expected Merged false for OPEN state")
	}
	if pr.Title != "Add a thing" {
		t.Fatalf("expected Title 'Add a thing', got %q", pr.Title)
	}
}

func TestGetPRForBranch_StateMerged_DerivesMergedTrue(t *testing.T) {
	stubGHDeps(t)

	runGHGetOutput = func(args []string) (string, error) {
		return `{
			"number": 7,
			"url": "https://github.com/org/repo/pull/7",
			"state": "MERGED",
			"isDraft": false,
			"title": "Done"
		}`, nil
	}

	pr, err := GetPRForBranch("feature/y")
	if err != nil {
		t.Fatalf("expected no error, got: %v", err)
	}
	if pr == nil {
		t.Fatal("expected non-nil *PRSummary")
	}
	if !pr.Merged {
		t.Fatal("expected Merged true for MERGED state")
	}
	if pr.State != "MERGED" {
		t.Fatalf("expected State 'MERGED', got %q", pr.State)
	}
}

func TestGetPRForBranch_FieldListOmitsMerged(t *testing.T) {
	stubGHDeps(t)

	var gotArgs []string
	runGHGetOutput = func(args []string) (string, error) {
		gotArgs = args
		return `{"number":1,"url":"u","state":"OPEN","isDraft":false,"title":"t"}`, nil
	}

	if _, err := GetPRForBranch("feature/x"); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	// The --json value must not include "merged" — it isn't a valid gh field
	// and caused gh to error out ("Unknown JSON field: merged").
	joined := strings.Join(gotArgs, " ")
	if strings.Contains(joined, "merged") {
		t.Fatalf("expected --json field list to omit 'merged', got args: %v", gotArgs)
	}
}

func TestGetPRForBranch_OtherError(t *testing.T) {
	stubGHDeps(t)

	runGHGetOutput = func(args []string) (string, error) {
		return "", &process.VerboseExecError{
			Command: []string{"gh", "pr", "view", "foo"},
			Err:     errors.New("auth required"),
			Stderr:  "gh: authentication required",
		}
	}

	_, err := GetPRForBranch("foo")
	if err == nil {
		t.Fatal("expected error for non-'no PR' failure")
	}
	if !strings.Contains(err.Error(), "gh pr view") {
		t.Fatalf("expected wrapped error to contain 'gh pr view', got: %v", err)
	}
}

// --- CreatePR ---

func TestCreatePR_Success(t *testing.T) {
	stubGHDeps(t)

	runGHGetOutput = func(args []string) (string, error) {
		return "https://github.com/owner/repo/pull/42\n", nil
	}

	url, err := CreatePR("My title", "Body", false)
	if err != nil {
		t.Fatalf("expected no error, got: %v", err)
	}
	if url != "https://github.com/owner/repo/pull/42" {
		t.Fatalf("expected trimmed URL, got %q", url)
	}
}

func TestCreatePR_AlreadyExists_WithURL(t *testing.T) {
	stubGHDeps(t)

	runGHGetOutput = func(args []string) (string, error) {
		return "", &process.VerboseExecError{
			Command: []string{"gh", "pr", "create"},
			Err:     &exec.ExitError{},
			Stderr:  `a pull request for branch "foo" into branch "main" already exists: https://github.com/owner/repo/pull/7`,
		}
	}

	_, err := CreatePR("t", "b", false)
	if err == nil {
		t.Fatal("expected PRAlreadyExistsError")
	}
	var exists *PRAlreadyExistsError
	if !errors.As(err, &exists) {
		t.Fatalf("expected *PRAlreadyExistsError, got: %v", err)
	}
	if exists.Branch != "foo" {
		t.Fatalf("expected Branch 'foo', got %q", exists.Branch)
	}
	if exists.ExistingURL != "https://github.com/owner/repo/pull/7" {
		t.Fatalf("expected ExistingURL extracted, got %q", exists.ExistingURL)
	}
}

func TestCreatePR_AlreadyExists_WithoutURL(t *testing.T) {
	stubGHDeps(t)

	runGHGetOutput = func(args []string) (string, error) {
		return "", &process.VerboseExecError{
			Command: []string{"gh", "pr", "create"},
			Err:     &exec.ExitError{},
			Stderr:  `a pull request for branch "bar" into branch "main" already exists`,
		}
	}

	_, err := CreatePR("t", "b", false)
	if err == nil {
		t.Fatal("expected PRAlreadyExistsError")
	}
	var exists *PRAlreadyExistsError
	if !errors.As(err, &exists) {
		t.Fatalf("expected *PRAlreadyExistsError, got: %v", err)
	}
	if exists.Branch != "bar" {
		t.Fatalf("expected Branch 'bar', got %q", exists.Branch)
	}
	if exists.ExistingURL != "" {
		t.Fatalf("expected empty ExistingURL, got %q", exists.ExistingURL)
	}
}

func TestCreatePR_GhNotInstalled(t *testing.T) {
	stubGHDeps(t)

	runGHGetOutput = func(args []string) (string, error) {
		return "", &process.VerboseExecError{
			Command: []string{"gh", "pr", "create"},
			Err:     exec.ErrNotFound,
		}
	}

	_, err := CreatePR("t", "b", false)
	if err == nil {
		t.Fatal("expected error when gh not installed")
	}
	if !errors.Is(err, ErrGHNotInstalled) {
		t.Fatalf("expected errors.Is(err, ErrGHNotInstalled) to be true, got: %v", err)
	}
}

// --- EnableAutoMergeSquash ---

func TestEnableAutoMergeSquash_Passthrough(t *testing.T) {
	stubGHDeps(t)

	var gotArgs []string
	runGHPassthrough = func(args []string, opts ...process.ExecOption) error {
		gotArgs = args
		return nil
	}

	if err := EnableAutoMergeSquash("https://github.com/owner/repo/pull/42"); err != nil {
		t.Fatalf("expected no error, got: %v", err)
	}

	// Confirm the PR URL is forwarded and the squash/auto flags are present.
	// This is the one piece of arg assembly we assert because it's a passthrough
	// wrapper with no parseable output to verify against.
	joined := strings.Join(gotArgs, " ")
	if !strings.Contains(joined, "pr merge") ||
		!strings.Contains(joined, "https://github.com/owner/repo/pull/42") ||
		!strings.Contains(joined, "--auto") ||
		!strings.Contains(joined, "--squash") {
		t.Fatalf("unexpected args: %v", gotArgs)
	}
}

func TestEnableAutoMergeSquash_ForwardsOpts(t *testing.T) {
	stubGHDeps(t)

	var gotOptCount int
	runGHPassthrough = func(args []string, opts ...process.ExecOption) error {
		gotOptCount = len(opts)
		return nil
	}

	if err := EnableAutoMergeSquash("u", process.WithSilent()); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if gotOptCount != 1 {
		t.Fatalf("expected 1 opt forwarded, got %d", gotOptCount)
	}
}

func TestEnableAutoMergeSquash_WrapsError(t *testing.T) {
	stubGHDeps(t)

	runGHPassthrough = func(args []string, opts ...process.ExecOption) error {
		return errors.New("exit code 1")
	}

	err := EnableAutoMergeSquash("https://github.com/owner/repo/pull/42")
	if err == nil {
		t.Fatal("expected error")
	}
	if !strings.Contains(err.Error(), "gh pr merge") {
		t.Fatalf("expected error to wrap with 'gh pr merge' context, got: %v", err)
	}
}

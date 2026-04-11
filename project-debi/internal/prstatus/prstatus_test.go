package prstatus

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"os/exec"
	"strings"
	"testing"

	"devora/internal/process"
)

func stubPRStatusDeps(t *testing.T) {
	t.Helper()
	origGetGHPRView := getGHPRView
	origGetCurrentBranch := getCurrentBranch
	t.Cleanup(func() {
		getGHPRView = origGetGHPRView
		getCurrentBranch = origGetCurrentBranch
	})
}

// makePRJSON builds a gh pr view JSON string from the given PRData.
func makePRJSON(data PRData) string {
	b, err := json.Marshal(data)
	if err != nil {
		panic(fmt.Sprintf("makePRJSON: %v", err))
	}
	return string(b)
}

func noPRError() error {
	return &process.VerboseExecError{
		Command: []string{"gh", "pr", "view", "--json", "..."},
		Err:     &exec.ExitError{},
		Stderr:  "no pull requests found for branch \"feature/test\"",
	}
}

func ghNotInstalledError() error {
	return &process.VerboseExecError{
		Command: []string{"gh", "pr", "view", "--json", "..."},
		Err:     exec.ErrNotFound,
		Stderr:  "",
	}
}

// --- Test: Open PR (human output) ---

func TestRun_OpenPR_HumanOutput(t *testing.T) {
	stubPRStatusDeps(t)

	getCurrentBranch = func() (string, error) {
		return "feature/my-branch", nil
	}

	autoMerge := json.RawMessage(`null`)
	getGHPRView = func() (string, error) {
		return makePRJSON(PRData{
			Number:      42,
			Title:       "Add new feature",
			URL:         "https://github.com/org/repo/pull/42",
			State:       "OPEN",
			IsDraft:     false,
			HeadRefName: "feature/my-branch",
			StatusCheckRollup: []CheckRun{
				{Name: "build", Status: "COMPLETED", Conclusion: "SUCCESS"},
				{Name: "lint", Status: "COMPLETED", Conclusion: "SUCCESS"},
				{Name: "test", Status: "COMPLETED", Conclusion: "FAILURE"},
			},
			Reviews: []Review{
				{Author: ReviewAuthor{Login: "alice"}, State: "APPROVED"},
			},
			AutoMergeRequest: &autoMerge,
		}), nil
	}

	var buf bytes.Buffer
	err := Run(&buf, false)

	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}

	output := buf.String()

	if !strings.Contains(output, "PR Status") {
		t.Fatal("expected output to contain 'PR Status' header")
	}
	if !strings.Contains(output, "Branch: feature/my-branch") {
		t.Fatalf("expected output to contain branch name, got:\n%s", output)
	}
	if !strings.Contains(output, "PR #42: Add new feature") {
		t.Fatalf("expected output to contain PR number and title, got:\n%s", output)
	}
	if !strings.Contains(output, "open") {
		t.Fatalf("expected output to contain state 'open', got:\n%s", output)
	}
	if !strings.Contains(output, "2/3 passing") {
		t.Fatalf("expected output to contain checks summary '2/3 passing', got:\n%s", output)
	}
	if !strings.Contains(output, "1 failed") {
		t.Fatalf("expected output to contain '1 failed', got:\n%s", output)
	}
	if !strings.Contains(output, "test") {
		t.Fatalf("expected output to contain failed check name 'test', got:\n%s", output)
	}
	if !strings.Contains(output, "1 approved") {
		t.Fatalf("expected output to contain '1 approved', got:\n%s", output)
	}
	if !strings.Contains(output, "https://github.com/org/repo/pull/42") {
		t.Fatalf("expected output to contain URL, got:\n%s", output)
	}
}

// --- Test: Open PR (JSON output) ---

func TestRun_OpenPR_JSONOutput(t *testing.T) {
	stubPRStatusDeps(t)

	getCurrentBranch = func() (string, error) {
		return "feature/my-branch", nil
	}

	autoMerge := json.RawMessage(`{"enabledBy":{"login":"alice"}}`)
	getGHPRView = func() (string, error) {
		return makePRJSON(PRData{
			Number:      42,
			Title:       "Add new feature",
			URL:         "https://github.com/org/repo/pull/42",
			State:       "OPEN",
			IsDraft:     false,
			HeadRefName: "feature/my-branch",
			StatusCheckRollup: []CheckRun{
				{Name: "build", Status: "COMPLETED", Conclusion: "SUCCESS"},
				{Name: "lint", Status: "COMPLETED", Conclusion: "SUCCESS"},
			},
			Reviews: []Review{
				{Author: ReviewAuthor{Login: "bob"}, State: "APPROVED"},
			},
			AutoMergeRequest: &autoMerge,
		}), nil
	}

	var buf bytes.Buffer
	err := Run(&buf, true)

	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}

	var out JSONOutput
	if err := json.Unmarshal(buf.Bytes(), &out); err != nil {
		t.Fatalf("failed to parse JSON output: %v", err)
	}

	if out.Branch != "feature/my-branch" {
		t.Fatalf("expected branch 'feature/my-branch', got %q", out.Branch)
	}
	if out.PR == nil {
		t.Fatal("expected PR field to be non-nil")
	}
	if out.PR.Number != 42 {
		t.Fatalf("expected PR number 42, got %d", out.PR.Number)
	}
	if out.PR.Title != "Add new feature" {
		t.Fatalf("expected PR title 'Add new feature', got %q", out.PR.Title)
	}
	if out.PR.URL != "https://github.com/org/repo/pull/42" {
		t.Fatalf("expected PR URL, got %q", out.PR.URL)
	}
	if out.PR.State != "open" {
		t.Fatalf("expected PR state 'open', got %q", out.PR.State)
	}
	if out.PR.Draft {
		t.Fatal("expected Draft to be false")
	}
	if !out.PR.AutoMerge {
		t.Fatal("expected AutoMerge to be true")
	}
	if out.Checks == nil {
		t.Fatal("expected Checks to be non-nil")
	}
	if out.Checks.Total != 2 {
		t.Fatalf("expected 2 total checks, got %d", out.Checks.Total)
	}
	if out.Checks.Passed != 2 {
		t.Fatalf("expected 2 passed checks, got %d", out.Checks.Passed)
	}
	if out.Reviews == nil {
		t.Fatal("expected Reviews to be non-nil")
	}
	if out.Reviews.Approved != 1 {
		t.Fatalf("expected 1 approved review, got %d", out.Reviews.Approved)
	}
}

// --- Test: Merged PR ---

func TestRun_MergedPR_HumanOutput(t *testing.T) {
	stubPRStatusDeps(t)

	getCurrentBranch = func() (string, error) {
		return "feature/merged-branch", nil
	}
	getGHPRView = func() (string, error) {
		return makePRJSON(PRData{
			Number:      10,
			Title:       "Merged feature",
			URL:         "https://github.com/org/repo/pull/10",
			State:       "MERGED",
			HeadRefName: "feature/merged-branch",
		}), nil
	}

	var buf bytes.Buffer
	err := Run(&buf, false)

	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}

	output := buf.String()
	if !strings.Contains(output, "merged") {
		t.Fatalf("expected output to contain 'merged', got:\n%s", output)
	}
}

func TestRun_MergedPR_JSONOutput(t *testing.T) {
	stubPRStatusDeps(t)

	getCurrentBranch = func() (string, error) {
		return "feature/merged-branch", nil
	}
	getGHPRView = func() (string, error) {
		return makePRJSON(PRData{
			Number:      10,
			Title:       "Merged feature",
			URL:         "https://github.com/org/repo/pull/10",
			State:       "MERGED",
			HeadRefName: "feature/merged-branch",
		}), nil
	}

	var buf bytes.Buffer
	err := Run(&buf, true)

	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}

	var out JSONOutput
	if err := json.Unmarshal(buf.Bytes(), &out); err != nil {
		t.Fatalf("failed to parse JSON output: %v", err)
	}
	if out.PR.State != "merged" {
		t.Fatalf("expected state 'merged', got %q", out.PR.State)
	}
}

// --- Test: Closed PR ---

func TestRun_ClosedPR_HumanOutput(t *testing.T) {
	stubPRStatusDeps(t)

	getCurrentBranch = func() (string, error) {
		return "feature/closed-branch", nil
	}
	getGHPRView = func() (string, error) {
		return makePRJSON(PRData{
			Number:      11,
			Title:       "Closed feature",
			URL:         "https://github.com/org/repo/pull/11",
			State:       "CLOSED",
			HeadRefName: "feature/closed-branch",
		}), nil
	}

	var buf bytes.Buffer
	err := Run(&buf, false)

	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}

	output := buf.String()
	if !strings.Contains(output, "closed") {
		t.Fatalf("expected output to contain 'closed', got:\n%s", output)
	}
}

// --- Test: Draft PR ---

func TestRun_DraftPR_HumanOutput(t *testing.T) {
	stubPRStatusDeps(t)

	getCurrentBranch = func() (string, error) {
		return "feature/draft-branch", nil
	}
	getGHPRView = func() (string, error) {
		return makePRJSON(PRData{
			Number:      12,
			Title:       "Draft feature",
			URL:         "https://github.com/org/repo/pull/12",
			State:       "OPEN",
			IsDraft:     true,
			HeadRefName: "feature/draft-branch",
		}), nil
	}

	var buf bytes.Buffer
	err := Run(&buf, false)

	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}

	output := buf.String()
	if !strings.Contains(output, "draft") {
		t.Fatalf("expected output to contain 'draft', got:\n%s", output)
	}
}

// --- Test: Auto-merge enabled ---

func TestRun_AutoMergeEnabled_HumanOutput(t *testing.T) {
	stubPRStatusDeps(t)

	getCurrentBranch = func() (string, error) {
		return "feature/automerge", nil
	}

	autoMerge := json.RawMessage(`{"enabledBy":{"login":"alice"}}`)
	getGHPRView = func() (string, error) {
		return makePRJSON(PRData{
			Number:           13,
			Title:            "Auto merge feature",
			URL:              "https://github.com/org/repo/pull/13",
			State:            "OPEN",
			HeadRefName:      "feature/automerge",
			AutoMergeRequest: &autoMerge,
		}), nil
	}

	var buf bytes.Buffer
	err := Run(&buf, false)

	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}

	output := buf.String()
	if !strings.Contains(output, "auto-merge enabled") {
		t.Fatalf("expected output to contain 'auto-merge enabled', got:\n%s", output)
	}
}

func TestRun_AutoMergeEnabled_JSONOutput(t *testing.T) {
	stubPRStatusDeps(t)

	getCurrentBranch = func() (string, error) {
		return "feature/automerge", nil
	}

	autoMerge := json.RawMessage(`{"enabledBy":{"login":"alice"}}`)
	getGHPRView = func() (string, error) {
		return makePRJSON(PRData{
			Number:           13,
			Title:            "Auto merge feature",
			URL:              "https://github.com/org/repo/pull/13",
			State:            "OPEN",
			HeadRefName:      "feature/automerge",
			AutoMergeRequest: &autoMerge,
		}), nil
	}

	var buf bytes.Buffer
	err := Run(&buf, true)

	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}

	var out JSONOutput
	if err := json.Unmarshal(buf.Bytes(), &out); err != nil {
		t.Fatalf("failed to parse JSON output: %v", err)
	}
	if !out.PR.AutoMerge {
		t.Fatal("expected AutoMerge to be true in JSON output")
	}
}

// --- Test: No PR found (human output) ---

func TestRun_NoPR_HumanOutput(t *testing.T) {
	stubPRStatusDeps(t)

	getCurrentBranch = func() (string, error) {
		return "feature/no-pr", nil
	}
	getGHPRView = func() (string, error) {
		return "", noPRError()
	}

	var buf bytes.Buffer
	err := Run(&buf, false)

	if err != nil {
		t.Fatalf("expected nil error for no PR, got: %s", err.Error())
	}

	output := buf.String()
	if !strings.Contains(output, "No PR found") {
		t.Fatalf("expected warning message about no PR, got:\n%s", output)
	}
	if !strings.Contains(output, "feature/no-pr") {
		t.Fatalf("expected branch name in no-PR output, got:\n%s", output)
	}
}

// --- Test: No PR found (JSON output) ---

func TestRun_NoPR_JSONOutput(t *testing.T) {
	stubPRStatusDeps(t)

	getCurrentBranch = func() (string, error) {
		return "feature/no-pr", nil
	}
	getGHPRView = func() (string, error) {
		return "", noPRError()
	}

	var buf bytes.Buffer
	err := Run(&buf, true)

	if err != nil {
		t.Fatalf("expected nil error for no PR, got: %s", err.Error())
	}

	var out JSONOutput
	if err := json.Unmarshal(buf.Bytes(), &out); err != nil {
		t.Fatalf("failed to parse JSON output: %v", err)
	}

	if out.Branch != "feature/no-pr" {
		t.Fatalf("expected branch 'feature/no-pr', got %q", out.Branch)
	}
	if out.PR != nil {
		t.Fatal("expected PR to be nil when no PR found")
	}
	if out.Checks != nil {
		t.Fatal("expected Checks to be nil when no PR found")
	}
	if out.Reviews != nil {
		t.Fatal("expected Reviews to be nil when no PR found")
	}
}

// --- Test: Detached HEAD ---

func TestRun_DetachedHEAD(t *testing.T) {
	stubPRStatusDeps(t)

	getCurrentBranch = func() (string, error) {
		return "", &process.VerboseExecError{
			Command: []string{"git", "symbolic-ref", "--short", "HEAD"},
			Err:     errors.New("not a symbolic ref"),
			Stderr:  "fatal: ref HEAD is not a symbolic ref",
		}
	}
	getGHPRView = func() (string, error) {
		t.Fatal("getGHPRView should not be called when detached HEAD")
		return "", nil
	}

	var buf bytes.Buffer
	err := Run(&buf, false)

	if err == nil {
		t.Fatal("expected error for detached HEAD")
	}
	if !strings.Contains(err.Error(), "cannot check PR status: not on a branch") {
		t.Fatalf("expected error message about not on a branch, got: %s", err.Error())
	}
}

// --- Test: gh not installed ---

func TestRun_GhNotInstalled(t *testing.T) {
	stubPRStatusDeps(t)

	getCurrentBranch = func() (string, error) {
		return "feature/test", nil
	}
	getGHPRView = func() (string, error) {
		return "", ghNotInstalledError()
	}

	var buf bytes.Buffer
	err := Run(&buf, false)

	if err == nil {
		t.Fatal("expected error when gh is not installed")
	}
	if !strings.Contains(err.Error(), "gh CLI is required") {
		t.Fatalf("expected error about gh CLI, got: %s", err.Error())
	}
	if !strings.Contains(err.Error(), "https://cli.github.com") {
		t.Fatalf("expected install URL in error, got: %s", err.Error())
	}
}

// --- Test: All checks passing ---

func TestRun_AllChecksPassing_HumanOutput(t *testing.T) {
	stubPRStatusDeps(t)

	getCurrentBranch = func() (string, error) {
		return "feature/all-pass", nil
	}
	getGHPRView = func() (string, error) {
		return makePRJSON(PRData{
			Number:      20,
			Title:       "All pass",
			URL:         "https://github.com/org/repo/pull/20",
			State:       "OPEN",
			HeadRefName: "feature/all-pass",
			StatusCheckRollup: []CheckRun{
				{Name: "build", Status: "COMPLETED", Conclusion: "SUCCESS"},
				{Name: "test", Status: "COMPLETED", Conclusion: "SUCCESS"},
				{Name: "lint", Status: "COMPLETED", Conclusion: "SUCCESS"},
			},
		}), nil
	}

	var buf bytes.Buffer
	err := Run(&buf, false)

	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}

	output := buf.String()
	if !strings.Contains(output, "3/3 passing") {
		t.Fatalf("expected '3/3 passing', got:\n%s", output)
	}
	// Should not contain failure or pending indicators
	if strings.Contains(output, "failed") {
		t.Fatalf("expected no 'failed' in all-pass output, got:\n%s", output)
	}
	if strings.Contains(output, "pending") {
		t.Fatalf("expected no 'pending' in all-pass output, got:\n%s", output)
	}
}

// --- Test: Checks with failures ---

func TestRun_ChecksWithFailures_HumanOutput(t *testing.T) {
	stubPRStatusDeps(t)

	getCurrentBranch = func() (string, error) {
		return "feature/failures", nil
	}
	getGHPRView = func() (string, error) {
		return makePRJSON(PRData{
			Number:      21,
			Title:       "Failures",
			URL:         "https://github.com/org/repo/pull/21",
			State:       "OPEN",
			HeadRefName: "feature/failures",
			StatusCheckRollup: []CheckRun{
				{Name: "build", Status: "COMPLETED", Conclusion: "SUCCESS"},
				{Name: "test", Status: "COMPLETED", Conclusion: "FAILURE"},
				{Name: "deploy", Status: "COMPLETED", Conclusion: "FAILURE"},
			},
		}), nil
	}

	var buf bytes.Buffer
	err := Run(&buf, false)

	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}

	output := buf.String()
	if !strings.Contains(output, "1/3 passing") {
		t.Fatalf("expected '1/3 passing', got:\n%s", output)
	}
	if !strings.Contains(output, "2 failed") {
		t.Fatalf("expected '2 failed', got:\n%s", output)
	}
	if !strings.Contains(output, "test") {
		t.Fatalf("expected failed check 'test' listed, got:\n%s", output)
	}
	if !strings.Contains(output, "deploy") {
		t.Fatalf("expected failed check 'deploy' listed, got:\n%s", output)
	}
}

// --- Test: Checks with pending ---

func TestRun_ChecksWithPending_HumanOutput(t *testing.T) {
	stubPRStatusDeps(t)

	getCurrentBranch = func() (string, error) {
		return "feature/pending", nil
	}
	getGHPRView = func() (string, error) {
		return makePRJSON(PRData{
			Number:      22,
			Title:       "Pending checks",
			URL:         "https://github.com/org/repo/pull/22",
			State:       "OPEN",
			HeadRefName: "feature/pending",
			StatusCheckRollup: []CheckRun{
				{Name: "build", Status: "COMPLETED", Conclusion: "SUCCESS"},
				{Name: "test", Status: "IN_PROGRESS", Conclusion: ""},
				{Name: "deploy", Status: "QUEUED", Conclusion: ""},
			},
		}), nil
	}

	var buf bytes.Buffer
	err := Run(&buf, false)

	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}

	output := buf.String()
	if !strings.Contains(output, "1/3 passing") {
		t.Fatalf("expected '1/3 passing', got:\n%s", output)
	}
	if !strings.Contains(output, "2 pending") {
		t.Fatalf("expected '2 pending', got:\n%s", output)
	}
	if !strings.Contains(output, "test") {
		t.Fatalf("expected pending check 'test' listed, got:\n%s", output)
	}
	if !strings.Contains(output, "deploy") {
		t.Fatalf("expected pending check 'deploy' listed, got:\n%s", output)
	}
}

// --- Test: No checks ---

func TestRun_NoChecks_HumanOutput(t *testing.T) {
	stubPRStatusDeps(t)

	getCurrentBranch = func() (string, error) {
		return "feature/no-checks", nil
	}
	getGHPRView = func() (string, error) {
		return makePRJSON(PRData{
			Number:      23,
			Title:       "No checks",
			URL:         "https://github.com/org/repo/pull/23",
			State:       "OPEN",
			HeadRefName: "feature/no-checks",
		}), nil
	}

	var buf bytes.Buffer
	err := Run(&buf, false)

	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}

	output := buf.String()
	if strings.Contains(output, "Checks:") {
		t.Fatalf("expected no 'Checks:' line when there are no checks, got:\n%s", output)
	}
}

// --- Test: Reviews approved ---

func TestRun_ReviewsApproved_HumanOutput(t *testing.T) {
	stubPRStatusDeps(t)

	getCurrentBranch = func() (string, error) {
		return "feature/approved", nil
	}
	getGHPRView = func() (string, error) {
		return makePRJSON(PRData{
			Number:      24,
			Title:       "Approved PR",
			URL:         "https://github.com/org/repo/pull/24",
			State:       "OPEN",
			HeadRefName: "feature/approved",
			Reviews: []Review{
				{Author: ReviewAuthor{Login: "alice"}, State: "APPROVED"},
				{Author: ReviewAuthor{Login: "bob"}, State: "APPROVED"},
			},
		}), nil
	}

	var buf bytes.Buffer
	err := Run(&buf, false)

	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}

	output := buf.String()
	if !strings.Contains(output, "2 approved") {
		t.Fatalf("expected '2 approved', got:\n%s", output)
	}
}

// --- Test: Reviews changes requested ---

func TestRun_ReviewsChangesRequested_HumanOutput(t *testing.T) {
	stubPRStatusDeps(t)

	getCurrentBranch = func() (string, error) {
		return "feature/changes", nil
	}
	getGHPRView = func() (string, error) {
		return makePRJSON(PRData{
			Number:      25,
			Title:       "Changes requested",
			URL:         "https://github.com/org/repo/pull/25",
			State:       "OPEN",
			HeadRefName: "feature/changes",
			Reviews: []Review{
				{Author: ReviewAuthor{Login: "alice"}, State: "CHANGES_REQUESTED"},
			},
		}), nil
	}

	var buf bytes.Buffer
	err := Run(&buf, false)

	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}

	output := buf.String()
	if !strings.Contains(output, "1 changes requested") {
		t.Fatalf("expected '1 changes requested', got:\n%s", output)
	}
}

// --- Test: No reviews ---

func TestRun_NoReviews_HumanOutput(t *testing.T) {
	stubPRStatusDeps(t)

	getCurrentBranch = func() (string, error) {
		return "feature/no-reviews", nil
	}
	getGHPRView = func() (string, error) {
		return makePRJSON(PRData{
			Number:      26,
			Title:       "No reviews",
			URL:         "https://github.com/org/repo/pull/26",
			State:       "OPEN",
			HeadRefName: "feature/no-reviews",
		}), nil
	}

	var buf bytes.Buffer
	err := Run(&buf, false)

	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}

	output := buf.String()
	if strings.Contains(output, "Reviews:") {
		t.Fatalf("expected no 'Reviews:' line when there are no reviews, got:\n%s", output)
	}
}

// --- Test: Multiple reviews same user (dedup) ---

func TestRun_ReviewDedup_LastWins(t *testing.T) {
	stubPRStatusDeps(t)

	getCurrentBranch = func() (string, error) {
		return "feature/dedup", nil
	}
	getGHPRView = func() (string, error) {
		return makePRJSON(PRData{
			Number:      27,
			Title:       "Dedup reviews",
			URL:         "https://github.com/org/repo/pull/27",
			State:       "OPEN",
			HeadRefName: "feature/dedup",
			Reviews: []Review{
				{Author: ReviewAuthor{Login: "alice"}, State: "CHANGES_REQUESTED"},
				{Author: ReviewAuthor{Login: "alice"}, State: "APPROVED"},
			},
		}), nil
	}

	var buf bytes.Buffer
	err := Run(&buf, false)

	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}

	output := buf.String()
	if !strings.Contains(output, "1 approved") {
		t.Fatalf("expected '1 approved' after dedup, got:\n%s", output)
	}
	if strings.Contains(output, "changes requested") {
		t.Fatalf("expected no 'changes requested' after dedup (last review wins), got:\n%s", output)
	}
}

// --- Test: computeChecksSummary ---

func TestComputeChecksSummary_MixedChecks(t *testing.T) {
	checks := []CheckRun{
		{Name: "build", Status: "COMPLETED", Conclusion: "SUCCESS"},
		{Name: "test", Status: "COMPLETED", Conclusion: "FAILURE"},
		{Name: "lint", Status: "IN_PROGRESS", Conclusion: ""},
		{Name: "deploy", Status: "COMPLETED", Conclusion: "SKIPPED"},
		{Name: "coverage", Status: "COMPLETED", Conclusion: "NEUTRAL"},
	}

	summary := computeChecksSummary(checks)

	if summary.Total != 5 {
		t.Fatalf("expected Total 5, got %d", summary.Total)
	}
	if summary.Passed != 1 {
		t.Fatalf("expected Passed 1, got %d", summary.Passed)
	}
	if summary.Failed != 1 {
		t.Fatalf("expected Failed 1, got %d", summary.Failed)
	}
	if summary.Pending != 1 {
		t.Fatalf("expected Pending 1, got %d", summary.Pending)
	}
	if summary.Skipped != 2 {
		t.Fatalf("expected Skipped 2, got %d", summary.Skipped)
	}
	if len(summary.FailedChecks) != 1 || summary.FailedChecks[0] != "test" {
		t.Fatalf("expected FailedChecks ['test'], got %v", summary.FailedChecks)
	}
	if len(summary.PendingChecks) != 1 || summary.PendingChecks[0] != "lint" {
		t.Fatalf("expected PendingChecks ['lint'], got %v", summary.PendingChecks)
	}
}

func TestComputeChecksSummary_Empty(t *testing.T) {
	summary := computeChecksSummary(nil)

	if summary.Total != 0 {
		t.Fatalf("expected Total 0, got %d", summary.Total)
	}
	if summary.Passed != 0 {
		t.Fatalf("expected Passed 0, got %d", summary.Passed)
	}
	if summary.Failed != 0 {
		t.Fatalf("expected Failed 0, got %d", summary.Failed)
	}
	if summary.Pending != 0 {
		t.Fatalf("expected Pending 0, got %d", summary.Pending)
	}
	if summary.Skipped != 0 {
		t.Fatalf("expected Skipped 0, got %d", summary.Skipped)
	}
}

func TestComputeChecksSummary_NameFallbackToContext(t *testing.T) {
	checks := []CheckRun{
		{Name: "build-check", Context: "", Status: "COMPLETED", Conclusion: "FAILURE", TypeName: "CheckRun"},
		{Name: "", Context: "ci/status", Status: "COMPLETED", Conclusion: "FAILURE", TypeName: "StatusContext"},
	}

	summary := computeChecksSummary(checks)

	if len(summary.FailedChecks) != 2 {
		t.Fatalf("expected 2 failed checks, got %d", len(summary.FailedChecks))
	}
	if summary.FailedChecks[0] != "build-check" {
		t.Fatalf("expected first failed check 'build-check', got %q", summary.FailedChecks[0])
	}
	if summary.FailedChecks[1] != "ci/status" {
		t.Fatalf("expected second failed check 'ci/status', got %q", summary.FailedChecks[1])
	}
}

func TestComputeChecksSummary_PendingStatuses(t *testing.T) {
	checks := []CheckRun{
		{Name: "a", Status: "IN_PROGRESS", Conclusion: ""},
		{Name: "b", Status: "QUEUED", Conclusion: ""},
		{Name: "c", Status: "PENDING", Conclusion: ""},
		{Name: "d", Status: "COMPLETED", Conclusion: ""},
	}

	summary := computeChecksSummary(checks)

	if summary.Pending != 4 {
		t.Fatalf("expected Pending 4, got %d", summary.Pending)
	}
	if len(summary.PendingChecks) != 4 {
		t.Fatalf("expected 4 pending check names, got %d", len(summary.PendingChecks))
	}
}

// --- Test: computeReviewsSummary ---

func TestComputeReviewsSummary_MultipleAuthors(t *testing.T) {
	reviews := []Review{
		{Author: ReviewAuthor{Login: "alice"}, State: "APPROVED"},
		{Author: ReviewAuthor{Login: "bob"}, State: "CHANGES_REQUESTED"},
		{Author: ReviewAuthor{Login: "charlie"}, State: "COMMENTED"},
		{Author: ReviewAuthor{Login: "dave"}, State: "PENDING"},
	}

	summary := computeReviewsSummary(reviews)

	if summary.Approved != 1 {
		t.Fatalf("expected Approved 1, got %d", summary.Approved)
	}
	if summary.ChangesRequested != 1 {
		t.Fatalf("expected ChangesRequested 1, got %d", summary.ChangesRequested)
	}
	if summary.Commented != 1 {
		t.Fatalf("expected Commented 1, got %d", summary.Commented)
	}
	if summary.Pending != 1 {
		t.Fatalf("expected Pending 1, got %d", summary.Pending)
	}
}

func TestComputeReviewsSummary_DuplicateAuthors(t *testing.T) {
	reviews := []Review{
		{Author: ReviewAuthor{Login: "alice"}, State: "CHANGES_REQUESTED"},
		{Author: ReviewAuthor{Login: "bob"}, State: "APPROVED"},
		{Author: ReviewAuthor{Login: "alice"}, State: "APPROVED"},
	}

	summary := computeReviewsSummary(reviews)

	if summary.Approved != 2 {
		t.Fatalf("expected Approved 2 (alice last=approved, bob=approved), got %d", summary.Approved)
	}
	if summary.ChangesRequested != 0 {
		t.Fatalf("expected ChangesRequested 0 (alice's earlier review overwritten), got %d", summary.ChangesRequested)
	}
}

func TestComputeReviewsSummary_Empty(t *testing.T) {
	summary := computeReviewsSummary(nil)

	if summary.Approved != 0 {
		t.Fatalf("expected Approved 0, got %d", summary.Approved)
	}
	if summary.ChangesRequested != 0 {
		t.Fatalf("expected ChangesRequested 0, got %d", summary.ChangesRequested)
	}
	if summary.Commented != 0 {
		t.Fatalf("expected Commented 0, got %d", summary.Commented)
	}
	if summary.Pending != 0 {
		t.Fatalf("expected Pending 0, got %d", summary.Pending)
	}
}

// --- Test: gh error propagated with context ---

func TestRun_GhOtherError_PropagatedWithContext(t *testing.T) {
	stubPRStatusDeps(t)

	getCurrentBranch = func() (string, error) {
		return "feature/test", nil
	}
	getGHPRView = func() (string, error) {
		return "", &process.VerboseExecError{
			Command: []string{"gh", "pr", "view"},
			Err:     errors.New("auth required"),
			Stderr:  "gh: authentication required",
		}
	}

	var buf bytes.Buffer
	err := Run(&buf, false)

	if err == nil {
		t.Fatal("expected error for gh failure")
	}
	// Should be wrapped with context
	if !strings.Contains(err.Error(), "gh pr view") || !strings.Contains(err.Error(), "auth") {
		t.Fatalf("expected error to contain context about the failure, got: %s", err.Error())
	}
}

// --- Test: Auto-merge NOT shown on merged PR ---

func TestRun_AutoMergeOnMergedPR_NotShown(t *testing.T) {
	stubPRStatusDeps(t)

	getCurrentBranch = func() (string, error) {
		return "feature/merged-auto", nil
	}

	autoMerge := json.RawMessage(`{"enabledBy":{"login":"alice"}}`)
	getGHPRView = func() (string, error) {
		return makePRJSON(PRData{
			Number:           30,
			Title:            "Merged with auto-merge",
			URL:              "https://github.com/org/repo/pull/30",
			State:            "MERGED",
			HeadRefName:      "feature/merged-auto",
			AutoMergeRequest: &autoMerge,
		}), nil
	}

	var buf bytes.Buffer
	err := Run(&buf, false)

	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}

	output := buf.String()
	if strings.Contains(output, "auto-merge enabled") {
		t.Fatalf("expected no 'auto-merge enabled' on merged PR, got:\n%s", output)
	}
}

// --- Test: Checks overflow (more than 5 failed) ---

func TestRun_ChecksOverflow_MoreThan5Failed(t *testing.T) {
	stubPRStatusDeps(t)

	getCurrentBranch = func() (string, error) {
		return "feature/overflow", nil
	}

	checks := make([]CheckRun, 8)
	for i := range checks {
		checks[i] = CheckRun{
			Name:       fmt.Sprintf("check-%d", i+1),
			Status:     "COMPLETED",
			Conclusion: "FAILURE",
		}
	}

	getGHPRView = func() (string, error) {
		return makePRJSON(PRData{
			Number:            31,
			Title:             "Overflow",
			URL:               "https://github.com/org/repo/pull/31",
			State:             "OPEN",
			HeadRefName:       "feature/overflow",
			StatusCheckRollup: checks,
		}), nil
	}

	var buf bytes.Buffer
	err := Run(&buf, false)

	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}

	output := buf.String()
	if !strings.Contains(output, "...and 3 more") {
		t.Fatalf("expected '...and 3 more' overflow message, got:\n%s", output)
	}
}

// --- Test: JSON output has empty slices not nil for checks ---

func TestRun_JSONOutput_EmptyCheckSlices(t *testing.T) {
	stubPRStatusDeps(t)

	getCurrentBranch = func() (string, error) {
		return "feature/empty-slices", nil
	}
	getGHPRView = func() (string, error) {
		return makePRJSON(PRData{
			Number:      32,
			Title:       "Empty slices",
			URL:         "https://github.com/org/repo/pull/32",
			State:       "OPEN",
			HeadRefName: "feature/empty-slices",
			StatusCheckRollup: []CheckRun{
				{Name: "build", Status: "COMPLETED", Conclusion: "SUCCESS"},
			},
		}), nil
	}

	var buf bytes.Buffer
	err := Run(&buf, true)

	if err != nil {
		t.Fatalf("expected no error, got: %s", err.Error())
	}

	// Verify that failed_checks and pending_checks are [] not null
	raw := buf.String()
	if !strings.Contains(raw, `"failed_checks": []`) {
		t.Fatalf("expected failed_checks to be [] not null, got:\n%s", raw)
	}
	if !strings.Contains(raw, `"pending_checks": []`) {
		t.Fatalf("expected pending_checks to be [] not null, got:\n%s", raw)
	}
}

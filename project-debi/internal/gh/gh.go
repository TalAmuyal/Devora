// Package gh wraps the GitHub CLI (`gh`) subprocess for the submit and close
// commands. It centralizes JSON parsing, stderr-driven error classification,
// and the stubbable package-level function vars that tests replace.
package gh

import (
	"encoding/json"
	"errors"
	"fmt"
	"os/exec"
	"regexp"
	"strings"

	"devora/internal/process"
)

// RepoInfo is the subset of `gh repo view` JSON we use.
type RepoInfo struct {
	Owner         string
	Name          string
	DefaultBranch string
}

// PRSummary is the subset of `gh pr view` JSON we use.
type PRSummary struct {
	Number  int
	URL     string
	State   string // "OPEN" | "CLOSED" | "MERGED"
	IsDraft bool
	Merged  bool
	Title   string
}

// PRAlreadyExistsError is returned by CreatePR when `gh` reports that a PR for
// the current branch already exists. ExistingURL is extracted from gh's stderr
// when present; otherwise empty.
type PRAlreadyExistsError struct {
	Branch      string
	ExistingURL string
}

func (e *PRAlreadyExistsError) Error() string {
	if e.ExistingURL != "" {
		return fmt.Sprintf("a pull request for branch %q already exists: %s", e.Branch, e.ExistingURL)
	}
	return fmt.Sprintf("a pull request for branch %q already exists", e.Branch)
}

// ErrGHNotInstalled is returned (wrapped) when the `gh` binary isn't on PATH.
// Callers detect with errors.Is.
var ErrGHNotInstalled = errors.New("gh CLI is not installed; see https://cli.github.com")

// --- Stubbable dependencies ---

var (
	runGHGetOutput   = defaultRunGHGetOutput
	runGHPassthrough = defaultRunGHPassthrough
)

func defaultRunGHGetOutput(args []string) (string, error) {
	return process.GetOutput(append([]string{"gh"}, args...))
}

func defaultRunGHPassthrough(args []string, opts ...process.ExecOption) error {
	return process.RunPassthrough(append([]string{"gh"}, args...), opts...)
}

// classifyGHError wraps a gh invocation error so callers can recognize
// "gh not installed" via errors.Is(err, ErrGHNotInstalled). Other errors are
// returned unchanged.
func classifyGHError(err error) error {
	if err == nil {
		return nil
	}
	if errors.Is(err, exec.ErrNotFound) {
		return fmt.Errorf("%w: %w", ErrGHNotInstalled, err)
	}
	return err
}

// --- GetRepo ---

// repoViewPayload is the wire shape of `gh repo view --json defaultBranchRef,name,owner`.
type repoViewPayload struct {
	Name             string `json:"name"`
	Owner            struct {
		Login string `json:"login"`
	} `json:"owner"`
	DefaultBranchRef struct {
		Name string `json:"name"`
	} `json:"defaultBranchRef"`
}

// GetRepo runs `gh repo view --json defaultBranchRef,name,owner` and returns
// the flattened RepoInfo.
func GetRepo() (RepoInfo, error) {
	out, err := runGHGetOutput([]string{
		"repo", "view",
		"--json", "defaultBranchRef,name,owner",
	})
	if err != nil {
		return RepoInfo{}, classifyGHError(err)
	}

	var payload repoViewPayload
	if err := json.Unmarshal([]byte(out), &payload); err != nil {
		return RepoInfo{}, fmt.Errorf("parse gh repo view: %w", err)
	}

	return RepoInfo{
		Owner:         payload.Owner.Login,
		Name:          payload.Name,
		DefaultBranch: payload.DefaultBranchRef.Name,
	}, nil
}

// --- GetPRForBranch ---

// prViewPayload is the wire shape of `gh pr view --json
// number,url,state,isDraft,title`. The PRSummary.Merged field is derived
// from state == "MERGED" (gh does not expose a direct "merged" JSON field).
type prViewPayload struct {
	Number  int    `json:"number"`
	URL     string `json:"url"`
	State   string `json:"state"`
	IsDraft bool   `json:"isDraft"`
	Title   string `json:"title"`
}

// GetPRForBranch runs `gh pr view <branch> --json
// number,url,state,isDraft,title` and returns the parsed PRSummary.
// PRSummary.Merged is derived from state == "MERGED".
// Returns (nil, nil) when gh reports no PR for the branch — detected via the
// stderr substring "no pull requests found", matching the handling in
// internal/prstatus.
func GetPRForBranch(branch string) (*PRSummary, error) {
	out, err := runGHGetOutput([]string{
		"pr", "view", branch,
		"--json", "number,url,state,isDraft,title",
	})
	if err != nil {
		var execErr *process.VerboseExecError
		if errors.As(err, &execErr) {
			if strings.Contains(execErr.Stderr, "no pull requests found") {
				return nil, nil
			}
		}
		return nil, fmt.Errorf("gh pr view %s: %w", branch, classifyGHError(err))
	}

	var payload prViewPayload
	if err := json.Unmarshal([]byte(out), &payload); err != nil {
		return nil, fmt.Errorf("parse gh pr view: %w", err)
	}
	return &PRSummary{
		Number:  payload.Number,
		URL:     payload.URL,
		State:   payload.State,
		IsDraft: payload.IsDraft,
		Merged:  payload.State == "MERGED",
		Title:   payload.Title,
	}, nil
}

// --- CreatePR ---

// prAlreadyExistsRE matches gh's "already exists" error line. gh prints
// messages like:
//   a pull request for branch "foo" into branch "main" already exists: https://github.com/owner/repo/pull/7
// or (when gh omits the URL):
//   a pull request for branch "foo" into branch "main" already exists
// Group 1 captures the branch name; group 2 captures the trailing URL (empty
// when absent).
var prAlreadyExistsRE = regexp.MustCompile(
	`a pull request for branch "([^"]+)".*?already exists(?::\s*(https?://\S+))?`,
)

// CreatePR runs `gh pr create --title <title> --body <body> --assignee @me
// [--draft]`. Returns the PR URL (trimmed stdout). On "already exists" stderr,
// returns *PRAlreadyExistsError.
func CreatePR(title, body string, draft bool) (string, error) {
	args := []string{"pr", "create", "--title", title, "--body", body, "--assignee", "@me"}
	if draft {
		args = append(args, "--draft")
	}

	out, err := runGHGetOutput(args)
	if err != nil {
		var execErr *process.VerboseExecError
		if errors.As(err, &execErr) {
			if existing := parsePRAlreadyExists(execErr.Stderr); existing != nil {
				return "", existing
			}
		}
		return "", fmt.Errorf("gh pr create: %w", classifyGHError(err))
	}
	return strings.TrimSpace(out), nil
}

// parsePRAlreadyExists returns a *PRAlreadyExistsError if stderr matches gh's
// "a pull request for branch ... already exists" pattern; otherwise nil.
func parsePRAlreadyExists(stderr string) *PRAlreadyExistsError {
	// Guard on both substrings before the regex for a cheap pre-check that
	// matches the plan's intent.
	if !strings.Contains(stderr, "a pull request for branch") ||
		!strings.Contains(stderr, "already exists") {
		return nil
	}
	match := prAlreadyExistsRE.FindStringSubmatch(stderr)
	if match == nil {
		// Substrings present but pattern didn't match — still surface as
		// "already exists" without a branch/URL rather than losing the signal.
		return &PRAlreadyExistsError{}
	}
	return &PRAlreadyExistsError{
		Branch:      match[1],
		ExistingURL: match[2],
	}
}

// --- EnableAutoMergeSquash ---

// EnableAutoMergeSquash runs `gh pr merge <prURL> --auto --squash` in
// passthrough mode so the user sees gh's output. Wraps errors with context.
// Callers pass process.WithSilent() to suppress stdout/stderr.
func EnableAutoMergeSquash(prURL string, opts ...process.ExecOption) error {
	if err := runGHPassthrough([]string{"pr", "merge", prURL, "--auto", "--squash"}, opts...); err != nil {
		return fmt.Errorf("gh pr merge --auto --squash: %w", classifyGHError(err))
	}
	return nil
}

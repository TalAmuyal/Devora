// Package wsgit implements the multi-repo flows behind `debi git summary` (a structured per-repo status summary) and workspace-mode `debi git checkout-latest` (a verify-then-update flow across all repos).
// Both fan out per-repo git/gh queries in parallel.
// internal/gitcmd dispatches into this package.
package wsgit

import (
	"errors"
	"path/filepath"

	"devora/internal/gh"
	"devora/internal/workspace"
)

var ErrNotAtWorkspaceRoot = errors.New("not at workspace root")

type PRState string

const (
	PRStateUnknown PRState = ""
	PRStateNone    PRState = "No"
	PRStateOpen    PRState = "open"
	PRStateClosed  PRState = "closed"
	PRStateMerged  PRState = "merged"
)

type RepoStatus struct {
	Name         string
	Branch       string // "" means detached HEAD
	Counts       PorcelainCounts
	BehindOrigin int  // -1 means "couldn't compute"; renderer prints "?"
	FetchFailed  bool // true means the table count is stale
	PRState      PRState
	PRError      error // non-nil means PR lookup failed (auth, network, etc.)
	Err          error // any non-PR error during gather
}

type RepoVerifyResult struct {
	Name     string
	Clean    bool
	Detached bool
	Branch   string // populated when not detached so we can name it in the failure summary
	Err      error
}

var getPRForBranch = gh.GetPRForBranch // A single test seam: wsgit_test.go swaps it in to simulate gh responses without invoking a real `gh` binary

// EnsureAtWorkspaceRoot returns the workspace path when cwd is the exact root of a Devora workspace.
// Path normalization (Abs + EvalSymlinks) is required because workspace.ResolveWorkspaceFromCWD applies the same normalization to each candidate workspace root before returning it; comparing without normalizing the cwd would yield false negatives for symlinked paths.
func EnsureAtWorkspaceRoot(cwd string) (string, error) {
	abs, err := filepath.Abs(cwd)
	if err != nil {
		return "", ErrNotAtWorkspaceRoot
	}
	abs, err = filepath.EvalSymlinks(abs)
	if err != nil {
		return "", ErrNotAtWorkspaceRoot
	}
	_, wsPath, _ := workspace.ResolveWorkspaceFromCWD(abs)
	if wsPath == "" {
		return "", ErrNotAtWorkspaceRoot
	}
	if abs != wsPath {
		return "", ErrNotAtWorkspaceRoot
	}
	return wsPath, nil
}

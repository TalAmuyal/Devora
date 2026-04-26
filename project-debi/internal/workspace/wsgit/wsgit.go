// Package wsgit implements workspace-aware versions of `debi gst` and
// `debi gcl`. When the user runs them at the exact root of a Devora workspace,
// the package fans out per-repo git/gh queries in parallel and renders a
// structured per-repo summary (status) or runs a verify-then-update flow
// (clean). Anywhere else, the CLI falls back to the existing per-repo
// passthrough wrappers in internal/git.
package wsgit

import (
	"errors"
	"path/filepath"

	"devora/internal/gh"
	"devora/internal/workspace"
)

// ErrNotAtWorkspaceRoot is returned by EnsureAtWorkspaceRoot when the cwd is
// not the exact root of a Devora workspace. The CLI dispatcher uses
// errors.Is to detect this and fall back to the per-repo passthrough path.
var ErrNotAtWorkspaceRoot = errors.New("not at workspace root")

// PRState describes the PR status for a repo's current branch as displayed by
// `debi gst`. PRStateUnknown is the zero value and is also used when we cannot
// classify a gh response.
type PRState string

const (
	PRStateUnknown PRState = ""
	PRStateNone    PRState = "No"
	PRStateOpen    PRState = "open"
	PRStateClosed  PRState = "closed"
	PRStateMerged  PRState = "merged"
)

// RepoStatus is the per-repo result of `debi gst` workspace mode. One row per
// repo gets rendered from this struct.
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

// RepoVerifyResult is the per-repo result of `debi gcl` Phase 1 verification.
type RepoVerifyResult struct {
	Name     string
	Clean    bool
	Detached bool
	Branch   string // populated when not detached so we can name it in the failure summary
	Err      error
}

// getPRForBranch is a single test seam: wsgit_test.go swaps it in to simulate
// gh responses without invoking a real `gh` binary.
var getPRForBranch = gh.GetPRForBranch

// EnsureAtWorkspaceRoot returns the workspace path when cwd is the exact root
// of a Devora workspace. Path normalization (Abs + EvalSymlinks) is required
// because workspace.ResolveWorkspaceFromCWD applies the same normalization to
// each candidate workspace root before returning it; comparing without
// normalizing the cwd would yield false negatives for symlinked paths.
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

package git

import (
	"fmt"
	"strings"

	"devora/internal/process"
)

func DefaultBranchName(opts ...process.ExecOption) (string, error) {
	output, err := process.GetOutput(
		[]string{"git", "symbolic-ref", "refs/remotes/origin/HEAD"},
		opts...,
	)
	if err != nil {
		return "", err
	}
	return strings.TrimPrefix(output, "refs/remotes/origin/"), nil
}

// DefaultBranchNameWithFallback resolves origin's default branch via
// `git symbolic-ref refs/remotes/origin/HEAD`; on failure, falls back to
// checking, in order, whether "main" or "master" exists via
// `git rev-parse --verify`, returning the first one found. Returns an error
// if none of these paths succeed.
func DefaultBranchNameWithFallback(opts ...process.ExecOption) (string, error) {
	if branch, err := DefaultBranchName(opts...); err == nil {
		return branch, nil
	}
	for _, candidate := range []string{"main", "master"} {
		if _, err := process.GetOutput(
			[]string{"git", "rev-parse", "--verify", candidate},
			opts...,
		); err == nil {
			return candidate, nil
		}
	}
	return "", fmt.Errorf("could not resolve default branch: origin/HEAD unset and neither main nor master exist")
}

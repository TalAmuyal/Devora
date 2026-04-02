package git

import (
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

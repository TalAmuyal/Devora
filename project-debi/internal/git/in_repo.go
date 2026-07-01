package git

import (
	"errors"

	"devora/internal/process"
)

var ErrNotInGitRepo = errors.New("not in a git repository")

const NotInRepoMessage = "this command must be run from inside a git repository"

func EnsureInRepo(opts ...process.ExecOption) error {
	if _, err := process.GetOutput([]string{"git", "rev-parse", "--git-dir"}, opts...); err != nil {
		return ErrNotInGitRepo
	}
	return nil
}

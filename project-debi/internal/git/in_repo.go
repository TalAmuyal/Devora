package git

import (
	"errors"

	"devora/internal/process"
)

// ErrNotInGitRepo is returned by EnsureInRepo when the current working
// directory is not inside a git repository. The CLI layer translates this
// into a *cli.UsageError with NotInRepoMessage.
var ErrNotInGitRepo = errors.New("not in a git repository")

// NotInRepoMessage is the user-facing message the CLI prints when a command
// requiring a git repository runs outside one.
const NotInRepoMessage = "this command must be run from inside a git repository"

// EnsureInRepo returns nil when the current working directory is inside a
// git repository, and ErrNotInGitRepo otherwise. Implemented via
// `git rev-parse --git-dir` — the same probe git uses internally, robust
// across locales and git versions.
func EnsureInRepo(opts ...process.ExecOption) error {
	if _, err := process.GetOutput([]string{"git", "rev-parse", "--git-dir"}, opts...); err != nil {
		return ErrNotInGitRepo
	}
	return nil
}

package cli

import (
	"errors"
	"os"

	"devora/internal/gitcmd"
)

// runGit adapts gitcmd.Run to the CLI layer: gitcmd's typed usage errors become *UsageError (friendly message, exit 1, no crash log), while PassthroughError and unexpected errors bubble unchanged
func runGit(args []string) error {
	err := gitcmd.Run(os.Stdout, args)
	var bad *gitcmd.BadUsageError
	if errors.As(err, &bad) {
		return &UsageError{Message: bad.Message}
	}
	return err
}

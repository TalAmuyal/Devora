package git

import (
	"fmt"
	"os"
	"strconv"
	"strings"

	"devora/internal/process"
)

// Run a git command with stdin/stdout/stderr connected to the terminal
func passthrough(args ...string) error {
	return process.RunPassthrough(append([]string{"git"}, args...))
}

func fetchOrigin() error {
	return passthrough("fetch", "origin")
}

func pushOriginForce() error {
	return passthrough("push", "origin", "--force")
}

func AddAllCommit(msgWords []string) error {
	message := strings.Join(msgWords, " ")
	if err := passthrough("add", "."); err != nil {
		return err
	}
	return passthrough("commit", "-m", message)
}

func AddAllCommitPush(msgWords []string) error {
	if err := AddAllCommit(msgWords); err != nil {
		return err
	}
	return passthrough("push", "origin")
}

func AddAllAmend() error {
	if err := passthrough("add", "."); err != nil {
		return err
	}
	return passthrough("commit", "--amend", "--no-edit")
}

func AddAllAmendPush() error {
	if err := AddAllAmend(); err != nil {
		return err
	}
	return pushOriginForce()
}

func BranchDeleteCurrent() error {
	output, err := process.GetOutput([]string{"git", "symbolic-ref", "HEAD"})
	if err != nil {
		return err
	}
	branchName := strings.TrimPrefix(output, "refs/heads/")
	if err := passthrough("checkout", "--detach"); err != nil {
		return err
	}
	return passthrough("branch", "-D", branchName)
}

func CheckoutLatest() error {
	if err := fetchOrigin(); err != nil {
		return err
	}
	return CheckoutOriginDefault(nil)
}

func CheckoutOriginDefault(args []string) error {
	branch, err := DefaultBranchName()
	if err != nil {
		return err
	}
	return passthrough(append([]string{"checkout", "origin/" + branch}, args...)...)
}

func RebaseInteractive(args []string) error {
	var n int
	if len(args) == 0 {
		branch, err := DefaultBranchName()
		if err != nil {
			return err
		}
		mergeBase, err := process.GetOutput([]string{"git", "merge-base", "HEAD", "origin/" + branch})
		if err != nil {
			return err
		}
		countStr, err := process.GetOutput([]string{"git", "rev-list", "--count", "HEAD", "^" + mergeBase})
		if err != nil {
			return err
		}
		n, err = strconv.Atoi(countStr)
		if err != nil {
			return fmt.Errorf("failed to parse commit count: %w", err)
		}
	} else {
		var err error
		n, err = strconv.Atoi(args[0])
		if err != nil || n < 1 {
			fmt.Fprintln(os.Stderr, "Invalid argument, see debi git rebase-interactive --help.")
			return &process.PassthroughError{Code: 1}
		}
	}

	if n == 0 {
		fmt.Println("Nothing to rebase.")
		return nil
	}

	return passthrough("rebase", "-i", fmt.Sprintf("HEAD~%d", n))
}

func RebaseLatest(push bool) error {
	if err := fetchOrigin(); err != nil {
		return err
	}
	if err := RebaseOriginDefault(); err != nil {
		return err
	}
	if push {
		return pushOriginForce()
	}
	return nil
}

func RebaseOriginDefault() error {
	branch, err := DefaultBranchName()
	if err != nil {
		return err
	}
	return passthrough("rebase", "origin/"+branch)
}

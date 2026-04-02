package git

import (
	"fmt"
	"os"
	"strconv"
	"strings"

	"devora/internal/process"
)

// passthrough runs a git command with stdin/stdout/stderr connected to the terminal.
func passthrough(args ...string) error {
	return process.RunPassthrough(append([]string{"git"}, args...))
}

func Gaa() error {
	return passthrough("add", ".")
}

func Gaac(args []string) error {
	message := strings.Join(args, " ")
	if err := passthrough("add", "."); err != nil {
		return err
	}
	return passthrough("commit", "-m", message)
}

func Gaacp(args []string) error {
	if err := Gaac(args); err != nil {
		return err
	}
	return passthrough("push", "origin")
}

func Gaaa() error {
	if err := passthrough("add", "."); err != nil {
		return err
	}
	return passthrough("commit", "--amend", "--no-edit")
}

func Gaaap() error {
	if err := Gaaa(); err != nil {
		return err
	}
	return Gpof(nil)
}

func Gb(args []string) error {
	return passthrough(append([]string{"branch"}, args...)...)
}

func Gbd(args []string) error {
	return passthrough(append([]string{"branch", "-D"}, args...)...)
}

func Gbdc() error {
	output, err := process.GetOutput([]string{"git", "symbolic-ref", "HEAD"})
	if err != nil {
		return err
	}
	branchName := strings.TrimPrefix(output, "refs/heads/")
	if err := passthrough("checkout", "--detach"); err != nil {
		return err
	}
	return Gbd([]string{branchName})
}

func Gcl() error {
	if err := Gfo(nil); err != nil {
		return err
	}
	return Gcom(nil)
}

func Gcom(args []string) error {
	branch, err := DefaultBranchName()
	if err != nil {
		return err
	}
	return passthrough(append([]string{"checkout", "origin/" + branch}, args...)...)
}

func Gd(args []string) error {
	return passthrough(append([]string{"diff"}, args...)...)
}

func Gfo(args []string) error {
	return passthrough(append([]string{"fetch", "origin"}, args...)...)
}

func Gg(args []string) error {
	return passthrough(append([]string{"grep"}, args...)...)
}

func Gl(args []string) error {
	return passthrough(append([]string{"log"}, args...)...)
}

func Gpo(args []string) error {
	return passthrough(append([]string{"push", "origin"}, args...)...)
}

func Gpof(args []string) error {
	return passthrough(append([]string{"push", "origin", "--force"}, args...)...)
}

func Gri(args []string) error {
	if len(args) > 0 && (args[0] == "-h" || args[0] == "--help") {
		fmt.Println("Usage: debi gri [N]")
		fmt.Println("Start an interactive rebase for the last N commits.")
		fmt.Println("If N is not provided, all of the commits since the branching started will participate.")
		return nil
	}

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
			fmt.Fprintln(os.Stderr, "Invalid argument, see debi gri --help.")
			return &process.PassthroughError{Code: 1}
		}
	}

	if n == 0 {
		fmt.Println("Nothing to rebase.")
		return nil
	}

	return passthrough("rebase", "-i", fmt.Sprintf("HEAD~%d", n))
}

func Grl() error {
	if err := Gfo(nil); err != nil {
		return err
	}
	return Grom()
}

func Grlp() error {
	if err := Grl(); err != nil {
		return err
	}
	return Gpof(nil)
}

func Grom() error {
	branch, err := DefaultBranchName()
	if err != nil {
		return err
	}
	return passthrough("rebase", "origin/"+branch)
}

func Gst(args []string) error {
	return passthrough(append([]string{"status"}, args...)...)
}

func Gstash(args []string) error {
	return passthrough(append([]string{"stash"}, args...)...)
}

func Gpop(args []string) error {
	return passthrough(append([]string{"stash", "pop"}, args...)...)
}

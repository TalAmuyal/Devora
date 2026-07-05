package gitcmd

import (
	"errors"
	"fmt"
	"io"
	"os"

	"devora/internal/cmdinfo"
	"devora/internal/git"
	"devora/internal/workspace/wsgit"
)

// customCommand is a `debi git` subcommand with Debi-specific behavior.
// Custom names win over real git subcommands, so they are all hyphenated composites that git doesn't define.
type customCommand struct {
	info     cmdinfo.SubCommand
	argsHint string // e.g. "<msg>"; empty when the subcommand takes no args
	run      func(w io.Writer, args []string) error
}

func (c *customCommand) leftColumn() string {
	if c.argsHint == "" {
		return c.info.Name
	}
	return c.info.Name + " " + c.argsHint
}

func (c *customCommand) usageError() *BadUsageError {
	msg := "usage: debi git " + c.info.Name
	if c.argsHint != "" {
		msg += " " + c.argsHint
	}
	return &BadUsageError{Message: msg}
}

// runCustom applies the shared -h/--help short-circuit before delegating, so no custom subcommand can misread a help flag as payload (e.g. committing with the message "-h")
func runCustom(w io.Writer, c *customCommand, args []string) error {
	if len(args) > 0 && (args[0] == "-h" || args[0] == "--help") {
		fmt.Fprintln(w, c.usageError().Message)
		fmt.Fprintln(w, c.info.Description)
		return nil
	}
	return c.run(w, args)
}

// requireRepo gates customs whose probes capture output (default branch, symbolic-ref): a missing repo must become a friendly usage error, not a crash log (see project CLAUDE.md "Precondition handling")
func requireRepo() error {
	if err := ensureInRepo(); err != nil {
		return &BadUsageError{Message: git.NotInRepoMessage}
	}
	return nil
}

var customCommands = []customCommand{
	{
		info:     cmdinfo.SubCommand{Name: "add-all-amend", Description: "Stage all changes and amend the last commit"},
		argsHint: "",
		run: func(w io.Writer, args []string) error {
			if len(args) > 0 {
				return customIndex["add-all-amend"].usageError()
			}
			return git.AddAllAmend()
		},
	},
	{
		info:     cmdinfo.SubCommand{Name: "add-all-amend-push", Description: "Stage all, amend the last commit, and force-push"},
		argsHint: "",
		run: func(w io.Writer, args []string) error {
			if len(args) > 0 {
				return customIndex["add-all-amend-push"].usageError()
			}
			return git.AddAllAmendPush()
		},
	},
	{
		info:     cmdinfo.SubCommand{Name: "add-all-commit", Description: "Stage all changes and commit with message"},
		argsHint: "<msg>",
		run: func(w io.Writer, args []string) error {
			if len(args) == 0 {
				return customIndex["add-all-commit"].usageError()
			}
			return git.AddAllCommit(args)
		},
	},
	{
		info:     cmdinfo.SubCommand{Name: "add-all-commit-push", Description: "Stage all, commit, and push to origin"},
		argsHint: "<msg>",
		run: func(w io.Writer, args []string) error {
			if len(args) == 0 {
				return customIndex["add-all-commit-push"].usageError()
			}
			return git.AddAllCommitPush(args)
		},
	},
	{
		info:     cmdinfo.SubCommand{Name: "branch-delete-current", Description: "Delete the current branch (detaches first)"},
		argsHint: "",
		run: func(w io.Writer, args []string) error {
			if len(args) > 0 {
				return customIndex["branch-delete-current"].usageError()
			}
			if err := requireRepo(); err != nil {
				return err
			}
			return git.BranchDeleteCurrent()
		},
	},
	{
		info:     cmdinfo.SubCommand{Name: "checkout-latest", Description: "Fetch origin and checkout the latest default branch; at a workspace root, verify and update every repo"},
		argsHint: "",
		run: func(w io.Writer, args []string) error {
			if len(args) > 0 {
				return customIndex["checkout-latest"].usageError()
			}
			return runCheckoutLatest(w)
		},
	},
	{
		info:     cmdinfo.SubCommand{Name: "checkout-origin-default", Description: "Checkout origin's default branch"},
		argsHint: "[args]",
		run: func(w io.Writer, args []string) error {
			if err := requireRepo(); err != nil {
				return err
			}
			return git.CheckoutOriginDefault(args)
		},
	},
	{
		info:     cmdinfo.SubCommand{Name: "rebase-interactive", Description: "Interactive rebase of the last N commits (default: all since branching)"},
		argsHint: "[N]",
		run: func(w io.Writer, args []string) error {
			if err := requireRepo(); err != nil {
				return err
			}
			return git.RebaseInteractive(args)
		},
	},
	{
		info: cmdinfo.SubCommand{
			Name:        "rebase-latest",
			Description: "Fetch origin and rebase on the default branch",
			Flags: []cmdinfo.Flag{
				{Name: "--push", Description: "Force-push after rebasing"},
			},
		},
		argsHint: "[--push]",
		run: func(w io.Writer, args []string) error {
			push := false
			for _, arg := range args {
				if arg != "--push" {
					return customIndex["rebase-latest"].usageError()
				}
				push = true
			}
			if err := requireRepo(); err != nil {
				return err
			}
			return git.RebaseLatest(push)
		},
	},
	{
		info:     cmdinfo.SubCommand{Name: "rebase-origin-default", Description: "Rebase on origin's default branch without fetching"},
		argsHint: "",
		run: func(w io.Writer, args []string) error {
			if len(args) > 0 {
				return customIndex["rebase-origin-default"].usageError()
			}
			if err := requireRepo(); err != nil {
				return err
			}
			return git.RebaseOriginDefault()
		},
	},
	{
		// "summary" shadows git-extras' `git summary` when spelled `debi git summary` - a conscious trade-off; the shims (`gsum`) never collide
		info:     cmdinfo.SubCommand{Name: "summary", Description: "Status, branch, and PR summary of all child repos"},
		argsHint: "",
		run: func(w io.Writer, args []string) error {
			if len(args) > 0 {
				return customIndex["summary"].usageError()
			}
			return runSummary(w)
		},
	},
}

var customIndex map[string]*customCommand

func init() {
	// Built in init to avoid an initialization cycle: the run closures refer to customIndex for their usage errors
	customIndex = make(map[string]*customCommand, len(customCommands))
	for i := range customCommands {
		customIndex[customCommands[i].info.Name] = &customCommands[i]
	}
}

func SubCommandInfos() []cmdinfo.SubCommand {
	infos := make([]cmdinfo.SubCommand, len(customCommands))
	for i, c := range customCommands {
		infos[i] = c.info
	}
	return infos
}

// runCheckoutLatest keeps checkout-latest's workspace awareness: at the exact root of a Devora workspace it runs the verify-then-update flow across all repos; anywhere else it updates the current repo only.
// The workspace gate stays (unlike read-only fan-out) because this command rewrites checkouts.
func runCheckoutLatest(w io.Writer) error {
	cwd, err := os.Getwd()
	if err == nil {
		wsPath, wsErr := wsgitEnsureAtWorkspaceRoot(cwd)
		if wsErr == nil {
			return wsgitRunClean(w, wsPath)
		}
		if !errors.Is(wsErr, wsgit.ErrNotAtWorkspaceRoot) {
			return wsErr
		}
	}
	if err := requireRepo(); err != nil {
		return err
	}
	return gitCheckoutLatest()
}

// runSummary renders the multi-repo status table for the current directory.
// Unlike checkout-latest it is read-only, so it uses the same generic child-repo discovery as fan-out instead of requiring a registered workspace root.
func runSummary(w io.Writer) error {
	cwd, err := os.Getwd()
	if err != nil {
		return err
	}
	repos, err := childRepos(cwd)
	if err != nil {
		return err
	}
	if len(repos) == 0 {
		return &BadUsageError{Message: "summary requires a directory whose immediate children include git repositories"}
	}
	return wsgitRunStatusDir(w, cwd)
}

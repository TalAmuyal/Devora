// Package gitcmd implements `debi git`: a namespace that runs Debi's custom git compositions, passes everything else through to the real git, and - when invoked from a directory whose immediate children are git repos - fans read-only subcommands out across those repos.
//
// Dispatch is deliberately minimal: the first argument is treated as the subcommand only when it doesn't start with '-'.
// Any leading git global option (-C, -c, --git-dir, --version, --help, ...) therefore passes through verbatim and git handles it itself.
// This way, we never have to track git's global-option surface.
package gitcmd

import (
	"fmt"
	"io"
	"os"
	"strings"

	"devora/internal/git"
	"devora/internal/process"
	"devora/internal/workspace/wsgit"
)

type BadUsageError struct {
	Message string
}

func (e *BadUsageError) Error() string {
	return e.Message
}

var fanOutOrder = []string{
	"status",
	"diff",
	"log",
	"show",
	"fetch",
	"grep",
	"blame",
	"shortlog",
	"describe",
	"ls-files",
}

var fanOutSubcommands = func() map[string]bool {
	m := make(map[string]bool, len(fanOutOrder))
	for _, name := range fanOutOrder {
		m[name] = true
	}
	return m
}()

// Stubbable seams for tests
var (
	runPassthrough             = process.RunPassthrough
	ensureInRepo               = git.EnsureInRepo
	childRepos                 = git.ChildRepos
	runFanOut                  = fanOut
	gitCheckoutLatest          = git.CheckoutLatest
	wsgitEnsureAtWorkspaceRoot = wsgit.EnsureAtWorkspaceRoot
	wsgitRunClean              = wsgit.RunClean
	wsgitRunStatusDir          = wsgit.RunStatusDir
)

func Run(w io.Writer, args []string) error {
	if len(args) == 0 {
		return &BadUsageError{Message: usageMessage()}
	}

	if cmd, ok := customIndex[args[0]]; ok {
		return runCustom(w, cmd, args[1:])
	}

	// GIT_DIR pins a repo explicitly, so multi-repo fan-out would be wrong
	if strings.HasPrefix(args[0], "-") ||
		os.Getenv("GIT_DIR") != "" ||
		!fanOutSubcommands[args[0]] ||
		ensureInRepo() == nil {
		return passthrough(args)
	}

	repos, err := childRepos(".")
	if err != nil || len(repos) == 0 {
		// Fall through so git prints its normal "not a repository" error
		return passthrough(args)
	}
	return runFanOut(w, args, repos)
}

func passthrough(args []string) error {
	return runPassthrough(append([]string{"git"}, args...))
}

func usageMessage() string {
	maxLeft := 0
	for _, c := range customCommands {
		if len(c.leftColumn()) > maxLeft {
			maxLeft = len(c.leftColumn())
		}
	}

	var b strings.Builder
	b.WriteString("usage: debi git <subcommand> [args]\n\nCustom subcommands:\n")
	for _, c := range customCommands {
		left := c.leftColumn()
		fmt.Fprintf(&b, "  %s%s%s\n", left, strings.Repeat(" ", maxLeft+2-len(left)), c.info.Description)
	}
	b.WriteString("\nAnything else is passed through to git.\n")
	b.WriteString("Read-only subcommands (" + strings.Join(fanOutOrder, ", ") + ")\n")
	b.WriteString("run in every immediate child repo when the current directory is not\nitself inside a repository.")
	return b.String()
}

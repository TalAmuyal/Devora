package cli

import (
	"devora/internal/config"
	"devora/internal/git"
	"devora/internal/health"
	"devora/internal/process"
	"devora/internal/task"
	"devora/internal/tui"
	"devora/internal/workspace"
	"errors"
	"fmt"
	"os"
)

// UsageError represents a user-facing error caused by incorrect CLI usage
// (e.g., missing command, unknown command, missing required argument).
// These should be printed to stderr without crash logging.
type UsageError struct {
	Message string
}

func (e *UsageError) Error() string {
	return e.Message
}

const usageMessage = `usage: debi <command> [args]

Workspace Commands:
  workspace-ui (w)  Open the workspace management UI
  add (a)           Add a repo to the current workspace
  rename (r)        Rename the current terminal session

Health:
  health [flags]        Check Devora dependencies

Git Shortcuts:
  gaa               Stage all changes
  gaac <msg>        Stage all and commit with message
  gaacp <msg>       Stage all, commit, and push to origin
  gaaa              Stage all and amend last commit
  gaaap             Stage all, amend, and force-push
  gb [args]         git branch
  gbd <branch>...   Force-delete branches
  gbdc              Delete current branch (detach first)
  gcl               Fetch origin and checkout default branch
  gcom [args]       Checkout default branch from origin
  gd [args]         git diff
  gfo [args]        Fetch from origin
  gg [args]         git grep
  gl [args]         git log
  gpo [args]        Push to origin
  gpof [args]       Force-push to origin
  gpop [args]       Pop git stash
  gri [N]           Interactive rebase (N commits or since branch)
  grl               Fetch and rebase on default branch
  grlp              Fetch, rebase, and force-push
  grom              Rebase on origin default branch
  gst [args]        git status
  gstash [args]     git stash`

func Run(args []string) error {
	if len(args) < 1 {
		return &UsageError{Message: usageMessage}
	}

	if args[0] == "-h" || args[0] == "--help" {
		fmt.Println(usageMessage)
		return nil
	}

	switch args[0] {
	case "workspace-ui", "w":
		return runWorkspaceUI()
	case "add", "a":
		return runAddRepo()
	case "rename", "r":
		if len(args) < 2 {
			return &UsageError{Message: "usage: debi rename <new-name>"}
		}
		return runRename(args[1])

	case "health":
		return runHealth(args[1:])

	// Git shortcuts — no args
	case "gaa":
		return git.Gaa()
	case "gaaa":
		return git.Gaaa()
	case "gaaap":
		return git.Gaaap()
	case "gbdc":
		return git.Gbdc()
	case "gcl":
		return git.Gcl()
	case "grl":
		return git.Grl()
	case "grlp":
		return git.Grlp()
	case "grom":
		return git.Grom()

	// Git shortcuts — required args
	case "gaac":
		if len(args) < 2 {
			return &UsageError{Message: "usage: debi gaac <msg>"}
		}
		return git.Gaac(args[1:])
	case "gaacp":
		if len(args) < 2 {
			return &UsageError{Message: "usage: debi gaacp <msg>"}
		}
		return git.Gaacp(args[1:])
	case "gbd":
		if len(args) < 2 {
			return &UsageError{Message: "usage: debi gbd <branch>..."}
		}
		return git.Gbd(args[1:])

	// Git shortcuts — optional args
	case "gb":
		return git.Gb(args[1:])
	case "gcom":
		return git.Gcom(args[1:])
	case "gd":
		return git.Gd(args[1:])
	case "gfo":
		return git.Gfo(args[1:])
	case "gg":
		return git.Gg(args[1:])
	case "gl":
		return git.Gl(args[1:])
	case "gpo":
		return git.Gpo(args[1:])
	case "gpof":
		return git.Gpof(args[1:])
	case "gpop":
		return git.Gpop(args[1:])
	case "gri":
		return git.Gri(args[1:])
	case "gst":
		return git.Gst(args[1:])
	case "gstash":
		return git.Gstash(args[1:])

	default:
		return &UsageError{Message: fmt.Sprintf("unknown command: %s", args[0])}
	}
}

func runWorkspaceUI() error {
	profiles := config.GetProfiles()
	showProfileRegistration := len(profiles) == 0
	if !showProfileRegistration {
		config.SetActiveProfile(&profiles[0])
	}

	var repoNames []string
	if !showProfileRegistration {
		var err error
		repoNames, err = config.GetRegisteredRepoNames()
		if err != nil {
			return fmt.Errorf("get repo names: %w", err)
		}
	}

	result, err := tui.RunWorkspaceUI(
		tui.DefaultThemePath(),
		repoNames,
		showProfileRegistration,
	)
	if err != nil {
		return err
	}
	if result == nil {
		return nil
	}

	if result.SelectedWorkspace != nil {
		ws := result.SelectedWorkspace
		sessionName := ws.TaskTitle
		if sessionName == "" {
			sessionName = ws.Name
		}
		return tui.CreateAndAttachSession(ws.Path, sessionName)
	}

	if result.NewWorkspace != nil {
		return tui.CreateAndAttachSession(
			result.NewWorkspace.WorkspacePath,
			result.NewWorkspace.SessionName,
		)
	}

	return nil
}

func runAddRepo() error {
	cwd, err := os.Getwd()
	if err != nil {
		return fmt.Errorf("get working directory: %w", err)
	}

	profile, wsPath, err := workspace.ResolveWorkspaceFromCWD(cwd)
	if err != nil {
		return fmt.Errorf("resolve workspace: %w", err)
	}
	if profile == nil {
		return &UsageError{Message: "not inside a known workspace. Run this command from within a workspace directory"}
	}

	config.SetActiveProfile(profile)

	repoNames, err := config.GetRegisteredRepoNames()
	if err != nil {
		return fmt.Errorf("get repo names: %w", err)
	}
	if len(repoNames) == 0 {
		return &UsageError{Message: "no repos registered in the active profile"}
	}

	return tui.RunAddRepo(
		tui.DefaultThemePath(),
		wsPath,
		repoNames,
	)
}

func runHealth(args []string) error {
	strict := false
	verbose := false
	for _, arg := range args {
		switch arg {
		case "-h", "--help":
			fmt.Println("usage: debi health [--strict] [-v|--verbose]\n\nCheck Devora dependencies and report their status.\n\nFlags:\n  --strict      Exit with code 1 if any dependency (including optional) is missing\n  -v, --verbose  Show dependency locations")
			return nil
		case "--strict":
			strict = true
		case "-v", "--verbose":
			verbose = true
		default:
			return &UsageError{Message: fmt.Sprintf("unknown flag: %s\nusage: debi health [--strict] [-v|--verbose]", arg)}
		}
	}
	return health.Run(os.Stdout, strict, verbose)
}

func runRename(newName string) error {
	_, err := process.GetOutput([]string{"kitty", "@", "set-tab-title", newName})
	if err != nil {
		return err
	}

	cwd, err := os.Getwd()
	if err != nil {
		return nil
	}

	_, wsPath, err := workspace.ResolveWorkspaceFromCWD(cwd)
	if err != nil || wsPath == "" {
		return nil
	}

	taskPath := workspace.GetWorkspaceTaskPath(wsPath)
	err = task.UpdateTitle(newName, taskPath)
	if errors.Is(err, os.ErrNotExist) {
		return nil
	}
	return err
}

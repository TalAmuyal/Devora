package cli

import (
	"devora/internal/config"
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

func Run(args []string) error {
	if len(args) < 1 {
		return &UsageError{Message: "usage: debi <command> [args]\n\nCommands:\n  workspace-ui (w)  Open the workspace management UI\n  add (a)           Add a repo to the current workspace\n  rename (r)        Rename the current terminal session"}
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

package cli

import (
	"devora/internal/completion"
	"devora/internal/config"
	"devora/internal/health"
	"devora/internal/process"
	"devora/internal/prstatus"
	"devora/internal/task"
	"devora/internal/tui"
	"devora/internal/workspace"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
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
		return &UsageError{Message: usageMessage()}
	}

	if args[0] == "-h" || args[0] == "--help" {
		fmt.Println(usageMessage())
		return nil
	}

	cmd, ok := commandIndex[args[0]]
	if !ok {
		return &UsageError{Message: fmt.Sprintf("unknown command: %s", args[0])}
	}

	cmdArgs := args[1:]
	if len(cmdArgs) < cmd.MinArgs {
		return usageErrorForCommand(cmd)
	}

	return cmd.Run(cmdArgs)
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

	startupError := checkBundledAppsInPath(
		os.Getenv("DEVORA_RESOURCES_DIR"),
		os.Getenv("PATH"),
	)

	result, err := tui.RunWorkspaceUI(
		tui.DefaultThemePath(),
		repoNames,
		showProfileRegistration,
		startupError,
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

func runPR(args []string) error {
	subcommand := args[0]
	subArgs := args[1:]

	switch subcommand {
	case "-h", "--help":
		fmt.Println("usage: debi pr <subcommand>\n\nSubcommands:\n  status    Check the status of the PR for the current branch")
		return nil
	case "status":
		return runPRStatus(subArgs)
	default:
		return &UsageError{Message: fmt.Sprintf("unknown pr subcommand: %s\nusage: debi pr <subcommand>", subcommand)}
	}
}

func runPRStatus(args []string) error {
	jsonOutput := false
	for _, arg := range args {
		switch arg {
		case "-h", "--help":
			fmt.Println("usage: debi pr status [--json]\n\nCheck the status of the PR for the current branch.\n\nFlags:\n  --json    Output status as JSON")
			return nil
		case "--json":
			jsonOutput = true
		default:
			return &UsageError{Message: fmt.Sprintf("unknown flag: %s\nusage: debi pr status [--json]", arg)}
		}
	}
	return prstatus.Run(os.Stdout, jsonOutput)
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

func checkBundledAppsInPath(resourcesDir, pathEnv string) string {
	if resourcesDir == "" {
		return ""
	}
	expected := filepath.Join(resourcesDir, "bundled-apps")
	for _, dir := range filepath.SplitList(pathEnv) {
		if filepath.Clean(dir) == filepath.Clean(expected) {
			return ""
		}
	}
	return formatStartupError(expected, pathEnv)
}

func formatStartupError(expectedDir, pathEnv string) string {
	var b strings.Builder
	b.WriteString("Expected directory in PATH:\n")
	b.WriteString("  " + expectedDir + "\n\n")
	b.WriteString("Current PATH:\n")
	for _, entry := range filepath.SplitList(pathEnv) {
		b.WriteString("  " + entry + "\n")
	}
	return b.String()
}

const completionHelp = `Generate shell completion script.

Usage: debi completion <bash|zsh|fish>

Run "debi completion <shell> -h" for shell-specific installation instructions.`

const completionHelpBash = `Generate bash completion script.

To load completions in the current session:
  source <(debi completion bash)

To load completions for every session (macOS):
  debi completion bash > $(brew --prefix)/etc/bash_completion.d/debi

To load completions for every session (Linux):
  debi completion bash > /etc/bash_completion.d/debi`

const completionHelpZsh = `Generate zsh completion script.

To load completions in the current session:
  source <(debi completion zsh)

To load completions for every session, create a completions directory
and add it to fpath in ~/.zshrc (before compinit):
  fpath=(~/.zsh/completions $fpath)
  autoload -U compinit; compinit

Then generate the completion file:
  mkdir -p ~/.zsh/completions
  debi completion zsh > ~/.zsh/completions/_debi

You may need to start a new shell for this setup to take effect.`

const completionHelpFish = `Generate fish completion script.

To load completions in the current session:
  debi completion fish | source

To load completions for every session:
  debi completion fish > ~/.config/fish/completions/debi.fish`

var shellHelp = map[string]string{
	"bash": completionHelpBash,
	"zsh":  completionHelpZsh,
	"fish": completionHelpFish,
}

func runCompletion(args []string) error {
	helpRequested := false
	shell := ""
	for _, arg := range args {
		switch arg {
		case "-h", "--help":
			helpRequested = true
		default:
			if shell == "" {
				shell = arg
			}
		}
	}

	if shell != "" {
		if _, ok := shellHelp[shell]; !ok {
			return &UsageError{Message: fmt.Sprintf("unsupported shell: %s\nusage: debi completion <bash|zsh|fish>", shell)}
		}
	}

	if helpRequested {
		if shell == "" {
			fmt.Println(completionHelp)
		} else {
			fmt.Println(shellHelp[shell])
		}
		return nil
	}

	if shell == "" {
		return &UsageError{Message: "usage: debi completion <bash|zsh|fish>"}
	}

	cmds := CommandInfos()
	switch shell {
	case "bash":
		return completion.GenerateBash(os.Stdout, "debi", cmds)
	case "zsh":
		return completion.GenerateZsh(os.Stdout, "debi", cmds)
	case "fish":
		return completion.GenerateFish(os.Stdout, "debi", cmds)
	default:
		return &UsageError{Message: fmt.Sprintf("unsupported shell: %s\nusage: debi completion <bash|zsh|fish>", shell)}
	}
}

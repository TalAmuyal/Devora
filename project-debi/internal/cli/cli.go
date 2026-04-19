package cli

import (
	closecmd "devora/internal/close"
	"devora/internal/completion"
	"devora/internal/config"
	"devora/internal/credentials"
	"devora/internal/git"
	"devora/internal/health"
	"devora/internal/jsonvalidate"
	"devora/internal/process"
	"devora/internal/prstatus"
	"devora/internal/submit"
	"devora/internal/task"
	// Register the Asana task-tracker provider. The blank import runs the
	// package's init(), which calls tasktracker.Register("asana", New).
	_ "devora/internal/tasktracker/asana"
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

const healthUsage = `usage: debi health [--strict] [-v|--verbose] [-p|--profile <name>]

Check Devora dependencies and report their status. Also reports whether zsh
completion is installed at ~/.zsh/completions/_debi.

Flags:
  --strict               Exit with code 1 if any dependency (including optional) is missing
  -v, --verbose          Show dependency locations
  -p, --profile <name>   Check a specific profile (defaults to CWD-based resolution)`

func runHealth(args []string) error {
	strict := false
	verbose := false
	profile := ""
	for i := 0; i < len(args); i++ {
		arg := args[i]
		switch {
		case arg == "-h" || arg == "--help":
			fmt.Println(healthUsage)
			return nil
		case arg == "--strict":
			strict = true
		case arg == "-v" || arg == "--verbose":
			verbose = true
		case arg == "-p" || arg == "--profile" || strings.HasPrefix(arg, "--profile="):
			val, nextI, err := parseValue(args, i, arg, "--profile")
			if err != nil {
				return err
			}
			profile = val
			i = nextI
		default:
			return &UsageError{Message: fmt.Sprintf("unknown flag: %s\n%s", arg, healthUsage)}
		}
	}
	if _, err := resolveActiveProfile(profile); err != nil {
		return err
	}
	return healthRun(os.Stdout, strict, verbose)
}

func runPR(args []string) error {
	subcommand := args[0]
	subArgs := args[1:]

	switch subcommand {
	case "-h", "--help":
		fmt.Println("usage: debi pr <subcommand>\n\nSubcommands:\n  check     Check the status of the PR for the current branch\n  submit    Commit, create tracker task and GitHub PR\n  close     Complete tracker task, delete branches")
		return nil
	case "check":
		return runPRCheck(subArgs)
	case "submit":
		return runSubmit(subArgs)
	case "close":
		return runClose(subArgs)
	default:
		return &UsageError{Message: fmt.Sprintf("unknown pr subcommand: %s\nusage: debi pr <subcommand>", subcommand)}
	}
}

func runPRCheck(args []string) error {
	jsonOutput := false
	for _, arg := range args {
		switch arg {
		case "-h", "--help":
			fmt.Println("usage: debi pr check [--json]\n\nCheck the status of the PR for the current branch.\n\nFlags:\n  --json    Output status as JSON")
			return nil
		case "--json":
			jsonOutput = true
		default:
			return &UsageError{Message: fmt.Sprintf("unknown flag: %s\nusage: debi pr check [--json]", arg)}
		}
	}
	if err := git.EnsureInRepo(); err != nil {
		if wrapped, ok := handleNotInGitRepo(err); ok {
			return wrapped
		}
		return err
	}
	return prstatus.Run(os.Stdout, jsonOutput)
}

// submitRun is the submit entry point; stubbable for tests.
var submitRun = submit.Run

// closeRun is the close entry point; stubbable for tests.
var closeRun = closecmd.Run

// healthRun is the health entry point; stubbable for tests.
var healthRun = health.Run

// resolveActiveProfile is the profile resolver entry point; stubbable for
// tests so the three handlers (health/submit/close) can verify they invoke it.
var resolveActiveProfile = ResolveActiveProfile

const submitUsage = `usage: debi submit -m <message> [flags]

Commit local changes, create a tracker task (if configured), create a feature
branch, push it, open a GitHub PR, and optionally enable auto-merge. Must be
run from a detached HEAD.

Flags:
  -m, --message <msg>    Commit message, task title, and PR title (required)
  -d, --description <s>  PR body description
      --draft            Create draft PR
  -b, --blocked          Skip auto-merge
  -o, --open-browser     Open PR in browser after creation
      --skip-tracker     Skip tracker task creation even if configured
      --json             Output result as JSON
  -v, --verbose          Show live git/gh subprocess output
  -q, --quiet            Print only the final PR URL
  -h, --help             Show this help`

const closeUsage = `usage: debi close [flags]

Mark the tracker task (if any) as complete, delete the remote branch, return
the working tree to detached HEAD on the default branch, and delete the local
branch.

Flags:
  -t, --task-url <url>   Tracker task URL (overrides branch's stored ID)
      --skip-tracker     Skip marking tracker task as complete
  -y, --force            Skip confirmation prompt for open PRs
  -v, --verbose          Show live git/gh subprocess output
  -q, --quiet            Print only "Closed" on success
  -h, --help             Show this help`

// parseValue returns the value for a flag that may be provided as "--flag=val"
// or "--flag val". It advances the caller's index when the value comes from
// the next arg. flagToken is the token as parsed (e.g., "--message" or
// "--message=value").
func parseValue(args []string, i int, flagToken, flagName string) (string, int, error) {
	if eq := strings.IndexByte(flagToken, '='); eq != -1 {
		return flagToken[eq+1:], i, nil
	}
	if i+1 >= len(args) {
		return "", i, &UsageError{Message: fmt.Sprintf("missing value for %s", flagName)}
	}
	return args[i+1], i + 1, nil
}

// parseSubmitFlags walks args and returns a populated submit.Options.
// Returns *UsageError for invalid usage (unknown flag, missing -m, or both
// --verbose and --quiet).
func parseSubmitFlags(args []string) (submit.Options, bool, error) {
	var opts submit.Options
	for i := 0; i < len(args); i++ {
		arg := args[i]
		switch {
		case arg == "-h" || arg == "--help":
			return opts, true, nil
		case arg == "--draft":
			opts.Draft = true
		case arg == "-b" || arg == "--blocked":
			opts.Blocked = true
		case arg == "-o" || arg == "--open-browser":
			opts.OpenBrowser = true
		case arg == "--skip-tracker":
			opts.SkipTracker = true
		case arg == "--json":
			opts.JSONOutput = true
		case arg == "-v" || arg == "--verbose":
			opts.Verbose = true
		case arg == "-q" || arg == "--quiet":
			opts.Quiet = true
		case arg == "-m" || arg == "--message" || strings.HasPrefix(arg, "--message="):
			val, nextI, err := parseValue(args, i, arg, "--message")
			if err != nil {
				return opts, false, err
			}
			opts.Message = val
			i = nextI
		case arg == "-d" || arg == "--description" || strings.HasPrefix(arg, "--description="):
			val, nextI, err := parseValue(args, i, arg, "--description")
			if err != nil {
				return opts, false, err
			}
			opts.Description = val
			i = nextI
		default:
			return opts, false, &UsageError{Message: fmt.Sprintf("unknown flag: %s\n%s", arg, submitUsage)}
		}
	}
	if opts.Message == "" {
		return opts, false, &UsageError{Message: "submit requires --message\n" + submitUsage}
	}
	if opts.Verbose && opts.Quiet {
		return opts, false, &UsageError{Message: "cannot use both --verbose and --quiet"}
	}
	return opts, false, nil
}

// parseCloseFlags walks args and returns a populated closecmd.Options.
// Returns *UsageError for invalid usage (unknown flag or both --verbose and
// --quiet).
func parseCloseFlags(args []string) (closecmd.Options, bool, error) {
	var opts closecmd.Options
	for i := 0; i < len(args); i++ {
		arg := args[i]
		switch {
		case arg == "-h" || arg == "--help":
			return opts, true, nil
		case arg == "--skip-tracker":
			opts.SkipTracker = true
		case arg == "-y" || arg == "--force":
			opts.Force = true
		case arg == "-v" || arg == "--verbose":
			opts.Verbose = true
		case arg == "-q" || arg == "--quiet":
			opts.Quiet = true
		case arg == "-t" || arg == "--task-url" || strings.HasPrefix(arg, "--task-url="):
			val, nextI, err := parseValue(args, i, arg, "--task-url")
			if err != nil {
				return opts, false, err
			}
			opts.TaskURL = val
			i = nextI
		default:
			return opts, false, &UsageError{Message: fmt.Sprintf("unknown flag: %s\n%s", arg, closeUsage)}
		}
	}
	if opts.Verbose && opts.Quiet {
		return opts, false, &UsageError{Message: "cannot use both --verbose and --quiet"}
	}
	return opts, false, nil
}

// handleCredentialNotFound detects *credentials.NotFoundError, prints the
// error and setup hint to stderr, and returns *UsageError{""} to suppress the
// crash log. Returns (nil, false) when err isn't a NotFoundError so the
// caller can keep classifying.
func handleCredentialNotFound(err error) (error, bool) {
	var nfe *credentials.NotFoundError
	if !errors.As(err, &nfe) {
		return nil, false
	}
	fmt.Fprintln(os.Stderr, err.Error())
	fmt.Fprintln(os.Stderr, credentials.SetupHint(nfe.Provider))
	return &UsageError{Message: ""}, true
}

// handleNotInGitRepo translates git.ErrNotInGitRepo into a *UsageError so the
// CLI exits with a friendly message instead of a crash log. Returns
// (nil, false) when err is not the sentinel.
func handleNotInGitRepo(err error) (error, bool) {
	if errors.Is(err, git.ErrNotInGitRepo) {
		return &UsageError{Message: git.NotInRepoMessage}, true
	}
	return nil, false
}

// runSubmit parses flags, invokes submit.Run, and translates domain sentinels
// into user-facing CLI errors / exit codes.
func runSubmit(args []string) error {
	opts, helpRequested, err := parseSubmitFlags(args)
	if err != nil {
		return err
	}
	if helpRequested {
		fmt.Println(submitUsage)
		return nil
	}

	if err := git.EnsureInRepo(); err != nil {
		if wrapped, ok := handleNotInGitRepo(err); ok {
			return wrapped
		}
		return err
	}

	if _, err := resolveActiveProfile(""); err != nil {
		return err
	}

	err = submitRun(os.Stdout, opts)
	if err == nil {
		return nil
	}

	if errors.Is(err, submit.ErrNotDetached) {
		return &UsageError{Message: err.Error()}
	}
	if wrapped, ok := handleCredentialNotFound(err); ok {
		return wrapped
	}
	return err
}

// runClose parses flags, invokes closecmd.Run, and translates domain sentinels
// into user-facing CLI errors / exit codes.
func runClose(args []string) error {
	opts, helpRequested, err := parseCloseFlags(args)
	if err != nil {
		return err
	}
	if helpRequested {
		fmt.Println(closeUsage)
		return nil
	}

	if err := git.EnsureInRepo(); err != nil {
		if wrapped, ok := handleNotInGitRepo(err); ok {
			return wrapped
		}
		return err
	}

	if _, err := resolveActiveProfile(""); err != nil {
		return err
	}

	err = closeRun(os.Stdout, opts)
	if err == nil {
		return nil
	}

	switch {
	case errors.Is(err, closecmd.ErrDetached):
		return &UsageError{Message: "close cannot run from detached HEAD; checkout a feature branch first"}
	case errors.Is(err, closecmd.ErrProtectedBranch),
		errors.Is(err, closecmd.ErrNoTrackerForURL):
		return &UsageError{Message: err.Error()}
	case errors.Is(err, closecmd.ErrAborted):
		// "→ Aborted" already printed by closecmd.Run. Exit 1 silently.
		return &process.PassthroughError{Code: 1}
	}
	if wrapped, ok := handleCredentialNotFound(err); ok {
		return wrapped
	}
	return err
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

func runUtil(args []string) error {
	subcommand := args[0]
	subArgs := args[1:]

	switch subcommand {
	case "-h", "--help":
		fmt.Println("usage: debi util <subcommand>\n\nSubcommands:\n  json-validate <file|->  Validate a JSON file (use - for stdin)")
		return nil
	case "json-validate":
		return runJSONValidate(subArgs)
	default:
		return &UsageError{Message: fmt.Sprintf("unknown util subcommand: %s\nusage: debi util <subcommand>", subcommand)}
	}
}

func runJSONValidate(args []string) error {
	if len(args) != 1 {
		fmt.Fprintln(os.Stderr, "usage: debi util json-validate <file|->")
		return &process.PassthroughError{Code: 2}
	}

	var reader *os.File
	if args[0] == "-" {
		reader = os.Stdin
	} else {
		f, err := os.Open(args[0])
		if err != nil {
			fmt.Fprintf(os.Stderr, "error: %s\n", err.Error())
			return &process.PassthroughError{Code: 2}
		}
		defer f.Close()
		reader = f
	}

	valid, errMsg, err := jsonvalidate.Validate(reader)
	if err != nil {
		return err
	}

	if valid {
		fmt.Println("Valid JSON")
		return nil
	}

	fmt.Printf("Invalid JSON: %s\n", errMsg)
	return &process.PassthroughError{Code: 1}
}

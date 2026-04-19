# CLI Package Spec

Package: `internal/cli`

## Purpose

Define and dispatch the CLI commands. This is the entry point that wires together all domain packages.

## Commands

The CLI has 3 workspace commands, a health command, PR commands (including `submit` and `close`, also registered as `pr submit` and `pr close`), a util command, 23 git shortcuts, and utility commands:

### Workspace Commands

| Command | Args | Description |
|---------|------|-------------|
| `workspace-ui` | none | Open the workspace management TUI |
| `add` | none | Open the add-repo TUI (must be inside a workspace) |
| `rename` | `<new-name>` (positional, required) | Rename the current terminal session |

### Health

| Command | Args | Description |
|---------|------|-------------|
| `health` | `[--strict] [-v\|--verbose] [-p\|--profile <name>]` | Check Devora dependencies and report their status |

#### health flags

| Flag | Description |
|------|-------------|
| `--strict` | Exit with code 1 if any dependency (including optional) is missing |
| `-v, --verbose` | Show dependency locations |
| `-p, --profile <name>` | Check a specific profile's config (defaults to CWD-based resolution) |

Flag syntax accepts both `--profile=foo` and `--profile foo`.

### PR

| Command | Args | Description |
|---------|------|-------------|
| `pr <subcommand>` | subcommand (required) | Pull request commands. Dispatches to subcommands |

#### PR Subcommands

| Subcommand | Args | Description |
|------------|------|-------------|
| `pr check` | `[--json]` | Same as top-level `check` |
| `pr submit` | same flags as top-level `submit` | Same as top-level `submit` |
| `pr close` | same flags as top-level `close` | Same as top-level `close` |

All three verbs (`check`, `submit`, `close`) are exposed both as top-level commands and as `pr` subcommands. For `submit` and `close`, flag slices (`submitFlags`, `closeFlags`) are declared once at package scope in `commands.go` and referenced by both registrations so the two forms never drift.

#### submit flags

| Flag | Description |
|------|-------------|
| `-m, --message <msg>` | Commit message, task title, and PR title (required) |
| `-d, --description <s>` | PR body description (appended after the tracker prefix) |
| `--draft` | Create draft PR |
| `-b, --blocked` | Skip auto-merge |
| `-o, --open-browser` | Open PR in browser after creation |
| `--skip-tracker` | Skip tracker task creation even if configured |
| `--json` | Output result as a single JSON object |
| `-v, --verbose` | Show live git/gh subprocess output |
| `-q, --quiet` | Print only the final PR URL (no prefix, no styling) |

Flag syntax accepts both `--message=foo` and `--message foo`. `-v` and `-q` are mutually exclusive; passing both returns `*UsageError`. See [submit.md](./submit.md) for the full verbosity-mode table.

#### close flags

| Flag | Description |
|------|-------------|
| `-t, --task-url <url>` | Tracker task URL (overrides `branch.<name>.task-id`) |
| `--skip-tracker` | Skip marking tracker task as complete |
| `-y, --force` | Skip confirmation prompt for open PRs |
| `-v, --verbose` | Show live git/gh subprocess output |
| `-q, --quiet` | Print only `✓ Closed` on success |

`-v` and `-q` are mutually exclusive; passing both returns `*UsageError`. See [close.md](./close.md) for the full verbosity-mode table.

### Utility

| Command | Args | Description |
|---------|------|-------------|
| `util <subcommand>` | subcommand (required) | Developer tool utilities. Dispatches to subcommands |

#### Util Subcommands

| Subcommand | Args | Description |
|------------|------|-------------|
| `json-validate` | `<file\|->` (positional, required) | Validate a JSON file; use `-` for stdin |

### Git Shortcuts

All git shortcuts run git commands in passthrough mode (stdin/stdout/stderr connected to the terminal).
They return `PassthroughError` on non-zero exit.

| Command | Args | Description |
|---------|------|-------------|
| `gaa` | none | `git add .` |
| `gaac` | `<msg>` (required) | `git add . && git commit -m <msg>` |
| `gaacp` | `<msg>` (required) | `gaac` then `git push origin` |
| `gaaa` | none | `git add . && git commit --amend --no-edit` |
| `gaaap` | none | `gaaa` then `gpof` |
| `gb` | `[args]` | `git branch [args]` |
| `gbd` | `<branch>...` (required) | `git branch -D <branch>...` |
| `gbdc` | none | Delete current branch (detach first) |
| `gcl` | none | `gfo` then `gcom` |
| `gcom` | `[args]` | `git checkout origin/<default-branch> [args]` |
| `gd` | `[args]` | `git diff [args]` |
| `gfo` | `[args]` | `git fetch origin [args]` |
| `gg` | `[args]` | `git grep [args]` |
| `gl` | `[args]` | `git log [args]` |
| `gpo` | `[args]` | `git push origin [args]` |
| `gpof` | `[args]` | `git push origin --force [args]` |
| `gpop` | `[args]` | `git stash pop [args]` |
| `gri` | `[N]` | Interactive rebase (N commits or since branch) |
| `grl` | none | `gfo` then `grom` |
| `grlp` | none | `grl` then `gpof` |
| `grom` | none | `git rebase origin/<default-branch>` |
| `gst` | `[args]` | `git status [args]` |
| `gstash` | `[args]` | `git stash [args]` |

### Utility

| Command | Args | Description |
|---------|------|-------------|
| `completion` | `<bash\|zsh\|fish>` (positional, required) | Generate a shell completion script to stdout |

## CLI Framework

Hand-rolled dispatcher using `os.Args`.
No external CLI framework (cobra, etc.) is needed.

Commands are defined in a data-driven registry (`[]Command` slice in `commands.go`). Each entry has `Name`, `Alias`, `Description`, `ArgsHint`, `Group`, `MinArgs`, `Run`, `Flags`, `ValidArgs`, and `SubCommands` fields. A `commandIndex` map provides O(1) lookup by name or alias. The usage message is generated from the registry.

Shared command metadata types (`Command`, `Flag`, `SubCommand`) are extracted to the `internal/cmdinfo` package to break a circular import between `cli` and `completion`. The `cmdinfo.Command` struct includes a `ValidArgs` field (`[]string`) for commands that accept a fixed set of positional arguments (e.g., `completion` accepts `bash`, `zsh`, `fish`) and a `SubCommands` field (`[]SubCommand`) for commands that dispatch to named sub-commands (e.g., `pr` dispatches to `check`, `util` dispatches to `json-validate`). Each `SubCommand` has its own `Name`, `Description`, `Flags`, `ValidArgs`, and `CompletesFiles` fields. The `CommandInfos()` function converts the internal registry to `[]cmdinfo.Command` for use by external packages.

Shell completion scripts are generated by the `internal/completion` package, which uses `text/template` to produce bash, zsh, and fish scripts from the command metadata. Commands with `Flags` or `ValidArgs` get second-level completion blocks that offer their ValidArgs, Flags, and universal `-h`/`--help` flags together. Commands with `SubCommands` get second-level completion (sub-command names) and third-level completion (sub-command-specific flags or file completion). Commands with neither (e.g., git shortcuts) get no subcommand completion.

## UsageError

```go
type UsageError struct {
    Message string
}
```

Represents a user-facing error caused by incorrect CLI usage (e.g., missing command, unknown command, missing required argument). In `main.go`, `UsageError` is printed to stderr without crash logging, unlike other errors which go through `crash.HandleError`.

## Function

### Run

```go
func Run(args []string) error
```

`args` is `os.Args[1:]` (program name already stripped).

Behavior:
1. If `args` is empty, return a `UsageError` containing the usage message.
2. Match `args[0]` against known commands and aliases.
3. Dispatch to the appropriate handler function.
4. For unknown commands, return an error with the command name.

### Usage Message

```
usage: debi <command> [args]

Workspace Commands:
  workspace-ui          Open the workspace management UI
  add                   Add a repo to the current workspace
  rename <new-name>     Rename the current terminal session

Health:
  health [flags]        Check Devora dependencies

PR:
  pr <subcommand>       Pull request commands
  pr check [flags]      Check the status of the PR for the current branch
  pr submit [flags]     Commit, create tracker task and GitHub PR (from detached HEAD)
  pr close [flags]      Complete tracker task, delete branches, return to detached HEAD

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
  gstash [args]     git stash

Utility:
  util <subcommand>           Developer utility commands
  completion <bash|zsh|fish>  Generate shell completion script
```

## Profile Resolution

### ResolveActiveProfile

```go
func ResolveActiveProfile(explicitProfile string) (string, error)
```

Shared helper used by `runHealth`, `runSubmit`, and `runClose` to resolve and register an active profile before the domain layer reads config. Implemented in `internal/cli/profile.go`.

Resolution order, stopping at the first hit:

1. `explicitProfile != ""` — look up by name in `config.GetProfiles()`. An unknown name returns a `*UsageError` (`"profile not found: <name>"`) without modifying active-profile state.
2. CWD-based — `workspace.ResolveWorkspaceFromCWD(cwd)`. When the CWD falls inside a profile's workspaces root, that profile becomes active.
3. No match — leave `config.activeProfile` nil so reads fall back to global config.

A failure from `os.Getwd` is treated as "no CWD match" (case 3 fallback), because an unreadable CWD is not a fatal error for a command that can still run against global config.

On success (cases 1 and 2), `config.SetActiveProfile` is called. The returned string is the resolved profile name (empty when case 3).

Three package-level stubbable vars support test injection:

```go
var (
    resolveFromCWD = workspace.ResolveWorkspaceFromCWD
    getProfiles    = config.GetProfiles
    getCWD         = os.Getwd
)
```

The handlers reach the resolver through `resolveActiveProfile`, a stubbable var pointing at `ResolveActiveProfile`, so tests can assert each handler invokes it.

Only `runHealth` accepts an explicit `--profile <name>`; `runSubmit` and `runClose` always call `ResolveActiveProfile("")`.

## Command Handlers

### workspace-ui

```go
func runWorkspaceUI() error
```

1. Check if `$DEVORA_RESOURCES_DIR/bundled-apps` is in PATH (only when `DEVORA_RESOURCES_DIR` is set, i.e., running from the app bundle). If missing, set `startupError` to a diagnostic message describing the issue.
2. Load profiles from config.
3. If no profiles exist, set `showProfileRegistration = true`. Skip to step 5 (`repoNames` remains `nil`).
4. If profiles exist, set the first profile as active and load repo names.
5. Launch the workspace TUI via `tui.RunWorkspaceUI(themePath, repoNames, showProfileRegistration, startupError)`. The `themePath` is obtained from `tui.DefaultThemePath()`.
5. After the TUI exits, handle the result:
   - If `result.SelectedWorkspace` is set, derive `sessionName` from `ws.TaskTitle` (falling back to `ws.Name` if empty), then delegate to `tui.CreateAndAttachSession(ws.Path, sessionName)` to create and attach a terminal session.
   - If `result.NewWorkspace` is set, delegate to `tui.CreateAndAttachSession(ws.WorkspacePath, ws.SessionName)`.
   - If the user quit without selection (`result == nil`), return nil.

### add

```go
func runAddRepo() error
```

1. Detect the current workspace from CWD (using `workspace.ResolveWorkspaceFromCWD`).
2. If not inside a workspace, return an error: `"not inside a known workspace. Run this command from within a workspace directory"`.
3. Set the detected profile as active.
4. Get registered repo names. If none, return a `UsageError`: `"no repos registered in the active profile"`.
5. Launch the add-repo TUI via `tui.RunAddRepo(themePath, wsPath, repoNames)`.

### rename

```go
func runRename(newName string) error
```

Renames the current Kitty tab and, if running inside a workspace, updates the task title.

1. Rename the Kitty tab via `process.GetOutput([]string{"kitty", "@", "set-tab-title", newName})`.
2. Detect the current workspace from CWD (using `workspace.ResolveWorkspaceFromCWD`).
3. Attempt to update the task title via `task.UpdateTitle(newName, taskPath)`. If the task file does not exist, the rename still succeeds.

The workspace detection is best-effort: if the CWD cannot be resolved or the task file is absent, the rename still succeeds (only the tab title is changed).

### util

```go
func runUtil(args []string) error
```

Dispatches on `args[0]`:
- `-h` or `--help`: prints the subcommand list and returns nil.
- `"json-validate"`: delegates to `runJSONValidate(args[1:])`.
- Unknown subcommand: returns a `UsageError`.

If `args` is empty, returns a `UsageError`.

### util json-validate

```go
func runJSONValidate(args []string) error
```

Validates a JSON file or stdin input.

1. If `args` does not contain exactly one element, print `"usage: debi util json-validate <file|->"` to stderr and return `PassthroughError{Code: 2}`.
2. If the argument is `"-"`, read from stdin.
3. Otherwise, open the file at the given path. If the file cannot be opened, print the error to stderr and return `PassthroughError{Code: 2}`.
4. Delegate to `jsonvalidate.Validate(reader)`.
5. If valid, print `"Valid JSON"` to stdout and return nil (exit 0).
6. If invalid, print `"Invalid JSON: <errMsg>"` to stdout and return `PassthroughError{Code: 1}`.

Exit codes: 0 = valid, 1 = invalid JSON, 2 = usage or file error.

### health

```go
func runHealth(args []string) error
```

Parses the provided args for flags:
- `-h` or `--help`: prints usage information and returns nil.
- `--strict`: enables strict mode.
- `-v` or `--verbose`: enables verbose mode (shows dependency locations).
- `-p` or `--profile <name>` (also `--profile=<name>`): selects a specific profile. Parsed via the shared `parseValue` helper.
- Any other argument: returns a `UsageError` with the unknown flag and usage hint.

Before delegating to the domain layer, calls `ResolveActiveProfile(profile)` (see below). A `*UsageError` from an unknown profile name short-circuits before `health.Run` runs.

Delegates to `health.Run(os.Stdout, strict, verbose)`.

In addition to required and optional dependencies, the health check also reports whether the zsh completion file exists at `~/.zsh/completions/_debi`. The check is optional: missing completion prints a ✗ and an install hint (`run: debi completion zsh > ~/.zsh/completions/_debi`) but does not affect the exit code in default mode. Under `--strict`, a missing completion file causes exit code 1 (same as any other optional dependency).

### completion

```go
func runCompletion(args []string) error
```

Generates a shell completion script to stdout. Supported shells: `bash`, `zsh`, `fish`. For unsupported shells, returns a `UsageError`.

Supports `-h`/`--help`:
- `debi completion -h` prints general help with usage and a hint to run shell-specific help.
- `debi completion <shell> -h` prints shell-specific installation instructions (flag position is flexible).
- If an invalid shell is provided alongside `-h`, the invalid shell error takes precedence.

Without a help flag and without a shell argument, returns a `UsageError` with the usage line.

Delegates to `completion.GenerateBash`, `completion.GenerateZsh`, or `completion.GenerateFish` based on the shell argument.

### pr

```go
func runPR(args []string) error
```

Dispatches on `args[0]`:
- `-h` or `--help`: prints the subcommand list and returns nil.
- `"check"`: delegates to `runPRCheck(args[1:])`.
- `"submit"`: delegates to `runSubmit(args[1:])`.
- `"close"`: delegates to `runClose(args[1:])`.
- Unknown subcommand: returns a `UsageError`.

If `args` is empty, returns a `UsageError`.

### check (also `pr check`)

```go
func runPRCheck(args []string) error
```

Parses the provided args for flags:
- `-h` or `--help`: prints usage information and returns nil.
- `--json`: enables JSON output mode.
- Any other argument: returns a `UsageError` with the unknown flag and usage hint.

After flag parsing (so `-h` still works outside a git repo), calls `git.EnsureInRepo()`. A `git.ErrNotInGitRepo` result is translated via `handleNotInGitRepo` to `*UsageError{Message: git.NotInRepoMessage}` so the command exits 1 with a friendly message instead of crashing.

Delegates to `prstatus.Run(os.Stdout, jsonOutput)`. The underlying domain package is still named `prstatus` because the CLI rename from `pr status` to `pr check` is a presentation-layer change; the package that implements the check continues to be `internal/prstatus`.

### submit (also `pr submit`)

```go
func runSubmit(args []string) error
```

1. `parseSubmitFlags(args)` walks the args and populates a `submit.Options`. Accepts `-h`/`--help` (prints usage and returns nil), `--draft`, `-b`/`--blocked`, `-o`/`--open-browser`, `--skip-tracker`, `--json`, `-v`/`--verbose`, `-q`/`--quiet`, `-m`/`--message <val>`, `-d`/`--description <val>`. Both `--flag=value` and `--flag value` forms are supported. Unknown flags, missing `--message`, and combining `--verbose` with `--quiet` all return `*UsageError`.
2. Calls `git.EnsureInRepo()`. `git.ErrNotInGitRepo` is translated via `handleNotInGitRepo` to `*UsageError{Message: git.NotInRepoMessage}`; any other error bubbles up.
3. Calls `ResolveActiveProfile("")` so profile-scoped config (notably `task-tracker.provider`) is visible to `submit.Run`. Errors from the resolver propagate.
4. Calls `submitRun(os.Stdout, opts)` (a stubbable var pointing at `submit.Run`).
5. Error translation:
   - `errors.Is(err, submit.ErrNotDetached)` -> `*UsageError{Message: err.Error()}`.
   - `errors.As(err, &*credentials.NotFoundError)` -> prints the error message and `credentials.SetupHint(provider)` to stderr, returns `*UsageError{Message: ""}` (suppresses the crash log; main.go prints nothing and exits 1).
   - Any other error bubbles up to `main.go`, which runs `crash.HandleError` unless the error is a `*process.PassthroughError`.

### close (also `pr close`)

```go
func runClose(args []string) error
```

1. `parseCloseFlags(args)` walks the args and populates a `closecmd.Options`. Accepts `-h`/`--help` (prints usage and returns nil), `--skip-tracker`, `-y`/`--force`, `-v`/`--verbose`, `-q`/`--quiet`, `-t`/`--task-url <val>`. Unknown flags and combining `--verbose` with `--quiet` return `*UsageError`. Both `--task-url=value` and `--task-url value` forms are supported.
2. Calls `git.EnsureInRepo()`. `git.ErrNotInGitRepo` is translated via `handleNotInGitRepo` to `*UsageError{Message: git.NotInRepoMessage}`; any other error bubbles up.
3. Calls `ResolveActiveProfile("")` so profile-scoped config (notably `task-tracker.provider`) is visible to `closecmd.Run`. Errors from the resolver propagate.
4. Calls `closeRun(os.Stdout, opts)` (a stubbable var pointing at `closecmd.Run`).
5. Error translation:
   - `errors.Is(err, closecmd.ErrDetached)` -> `*UsageError{Message: err.Error()}`.
   - `errors.Is(err, closecmd.ErrProtectedBranch)` -> `*UsageError{Message: err.Error()}` (the error wraps the branch name).
   - `errors.Is(err, closecmd.ErrNoTrackerForURL)` -> `*UsageError{Message: err.Error()}`.
   - `errors.Is(err, closecmd.ErrAborted)` -> `*process.PassthroughError{Code: 1}`. The `→ Aborted` line was already printed by `closecmd.Run`, and passthrough-error exits skip the crash log and the usage-error print.
   - `errors.As(err, &*credentials.NotFoundError)` -> prints the error and `credentials.SetupHint` to stderr, returns `*UsageError{Message: ""}`.
   - Any other error bubbles up.

## Exit Codes

| Situation | Exit |
|-----------|-----:|
| Success | 0 |
| Any `*cli.UsageError` (including the `{Message: ""}` "already printed" form) | 1 |
| PR command (`submit`/`close`/`check`) run outside a git repository (`git.ErrNotInGitRepo`) | 1 (friendly `NotInRepoMessage`, no crash log) |
| `submit`/`close` domain sentinel (not-detached, protected branch, aborted, `--task-url` without tracker, missing required flag) | 1 |
| `*credentials.NotFoundError` | 1 |
| `gh.PRAlreadyExistsError`, other domain errors | 1 (crash log printed) |
| `*process.PassthroughError{Code: N}` (raised by a git/gh subprocess via `internal/process`) | `N` (subprocess exit code propagates transparently) |

`PassthroughError.Code` still propagates unchanged when git/gh subprocesses raise it; submit and close do not intercept it.

## Binary Name

The built binary should be named `debi`.

The `mise.toml` build task should produce `debi`:
```toml
[tasks.build]
run = "go build -o debi ."
```

## Integration with main.go

```go
func main() {
    defer func() {
        if r := recover(); r != nil {
            crash.HandlePanic(r)
            os.Exit(1)
        }
    }()
    if err := cli.Run(os.Args[1:]); err != nil {
        var usageErr *cli.UsageError
        if errors.As(err, &usageErr) {
            fmt.Fprintln(os.Stderr, usageErr.Message)
            os.Exit(1)
        }
        var ptErr *process.PassthroughError
        if errors.As(err, &ptErr) {
            os.Exit(ptErr.Code)
        }
        crash.HandleError(err)
        os.Exit(1)
    }
}
```

## Testing

- Test that unknown commands return an appropriate error.
- Test that missing arguments for `rename` return an error.
- Test that empty args print usage.
- Test that `gaac`, `gaacp`, and `gbd` without required args return `UsageError`.
- Test that all 23 git commands are recognized (do not return "unknown command").
- Test that `health` is recognized as a command.
- Test that `health` with an unknown flag returns a `UsageError`.
- Test that `completion` is recognized as a command.
- Test that `completion` without arguments returns a `UsageError`.
- Test that `completion` with an unsupported shell returns a `UsageError`.
- Test that `completion -h` and `completion --help` print general help (contains usage line and shell-specific hint).
- Test that `completion <shell> -h` prints shell-specific installation instructions (table-driven for bash/zsh/fish, both flag positions).
- Test that `completion <invalid-shell> -h` returns a `UsageError` mentioning "unsupported shell".
- Test that `pr` without a subcommand returns a `UsageError`.
- Test that `pr` with an unknown subcommand returns a `UsageError`.
- Test that `pr check` is recognized and dispatches correctly.
- Test that `check --json` enables JSON output mode.
- Test that `check` with an unknown flag returns a `UsageError`.
- Test that `util` without a subcommand returns a `UsageError`.
- Test that `util` with an unknown subcommand returns a `UsageError`.
- Test that `util json-validate` is recognized and dispatches correctly.
- Test that `util json-validate` without an argument returns `PassthroughError{Code: 2}`.
- Test that `submit` without `--message` returns a `UsageError`.
- Test that `submit` accepts both `-m value` and `--message=value` forms.
- Test that `submit` and `close` map their domain sentinels to the correct `UsageError` / `PassthroughError` forms.
- Test that `submit` and `close` translate `*credentials.NotFoundError` by printing the setup hint and returning `*UsageError{Message: ""}`.
- Test that `pr submit`, `pr close`, and `pr check` return `*UsageError{Message: git.NotInRepoMessage}` when run from a directory that is not inside a git repository (and never invoke `resolveActiveProfile` or the domain runner).
- Command handler integration tests are better handled at a higher level (testing the actual TUI behavior or workspace operations).

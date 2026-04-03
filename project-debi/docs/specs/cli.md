# CLI Package Spec

Package: `internal/cli`

## Purpose

Define and dispatch the CLI commands. This is the entry point that wires together all domain packages.

## Commands

The CLI has 3 workspace commands (each with a hidden short alias), a health command, and 23 git shortcuts:

### Workspace Commands

| Command | Alias | Args | Description |
|---------|-------|------|-------------|
| `workspace-ui` | `w` | none | Open the workspace management TUI |
| `add` | `a` | none | Open the add-repo TUI (must be inside a workspace) |
| `rename` | `r` | `<new-name>` (positional, required) | Rename the current terminal session |

### Health

| Command | Args | Description |
|---------|------|-------------|
| `health` | `[--strict] [-v\|--verbose]` | Check Devora dependencies and report their status |

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

## CLI Framework

Hand-rolled dispatcher using `os.Args`.
No external CLI framework (cobra, etc.) is needed.
Git shortcut commands are implemented in the `internal/git` package and dispatched via `switch` cases.

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
  gstash [args]     git stash
```

## Command Handlers

### workspace-ui / w

```go
func runWorkspaceUI() error
```

1. Load profiles from config.
2. If no profiles exist, set `showProfileRegistration = true`. Skip to step 4 (`repoNames` remains `nil`).
3. If profiles exist, set the first profile as active and load repo names.
4. Launch the workspace TUI via `tui.RunWorkspaceUI(themePath, repoNames, showProfileRegistration)`. The `themePath` is obtained from `tui.DefaultThemePath()`.
5. After the TUI exits, handle the result:
   - If `result.SelectedWorkspace` is set, derive `sessionName` from `ws.TaskTitle` (falling back to `ws.Name` if empty), then delegate to `tui.CreateAndAttachSession(ws.Path, sessionName)` to create and attach a terminal session.
   - If `result.NewWorkspace` is set, delegate to `tui.CreateAndAttachSession(ws.WorkspacePath, ws.SessionName)`.
   - If the user quit without selection (`result == nil`), return nil.

### add / a

```go
func runAddRepo() error
```

1. Detect the current workspace from CWD (using `workspace.ResolveWorkspaceFromCWD`).
2. If not inside a workspace, return an error: `"not inside a known workspace. Run this command from within a workspace directory"`.
3. Set the detected profile as active.
4. Get registered repo names. If none, return a `UsageError`: `"no repos registered in the active profile"`.
5. Launch the add-repo TUI via `tui.RunAddRepo(themePath, wsPath, repoNames)`.

### rename / r

```go
func runRename(newName string) error
```

Renames the current Kitty tab and, if running inside a workspace, updates the task title.

1. Rename the Kitty tab via `process.GetOutput([]string{"kitty", "@", "set-tab-title", newName})`.
2. Detect the current workspace from CWD (using `workspace.ResolveWorkspaceFromCWD`).
3. Attempt to update the task title via `task.UpdateTitle(newName, taskPath)`. If the task file does not exist, the rename still succeeds.

The workspace detection is best-effort: if the CWD cannot be resolved or the task file is absent, the rename still succeeds (only the tab title is changed).

### health

```go
func runHealth(args []string) error
```

Parses the provided args for flags:
- `-h` or `--help`: prints usage information and returns nil.
- `--strict`: enables strict mode.
- `-v` or `--verbose`: enables verbose mode (shows dependency locations).
- Any other argument: returns a `UsageError` with the unknown flag and usage hint.

Delegates to `health.Run(os.Stdout, strict, verbose)`.

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
- Command handler integration tests are better handled at a higher level (testing the actual TUI behavior or workspace operations).

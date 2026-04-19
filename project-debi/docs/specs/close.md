# Close Package Spec

Package: `internal/close` (package declaration: `closecmd`)

The directory is `internal/close/`, but the package declaration is `closecmd` so callers' source never visually shadows the Go builtin `close`. Import as:

```go
closecmd "devora/internal/close"
```

## Purpose

Implement `debi close`: mark the tracker task (when one exists) as complete, delete the remote branch, return the working tree to detached HEAD on `origin/<default>`, and delete the local branch. Must be invoked from a non-protected feature branch.

## Profile Resolution

The CLI layer calls `cli.ResolveActiveProfile("")` before dispatching to `closecmd.Run`, so profile-scoped config (such as `task-tracker.provider`) resolved from the current working directory is visible. See [cli.md](./cli.md) and [health.md](./health.md) for the resolution order. There is no `--profile` flag on `close`; CWD is the sole selector.

## Stability

Stable public API: `Run`, `Options`, `ErrDetached`, `ErrProtectedBranch`, `ErrAborted`, `ErrNoTrackerForURL`.

## Dependencies

- `devora/internal/git` -- branch detection, protected-branch check, config helpers, push/fetch/checkout/delete helpers, default-branch resolution
- `devora/internal/gh` -- `gh pr view <branch>` wrapper
- `devora/internal/tasktracker` -- pluggable task-tracker interface
- `charm.land/lipgloss/v2` -- colored progress output
- `golang.org/x/sync/errgroup` -- concurrent task completion and remote-branch deletion

## Types

### Options

```go
type Options struct {
    TaskURL     string // optional; overrides branch.<name>.task-id. Requires a configured task-tracker.
    SkipTracker bool   // treat as no tracker configured, even if one is.
    Force       bool   // skip the "PR is still open" confirmation prompt.
    Verbose     bool   // show live git/gh subprocess output.
    Quiet       bool   // print only "✓ Closed" on success.
}
```

`Verbose` and `Quiet` are mutually exclusive; the CLI layer returns `*cli.UsageError` when both are set. See [Verbosity Modes](#verbosity-modes) for the behavior of each.

## Verbosity Modes

`Run` selects one of three output modes based on `Options`:

| Mode | Trigger | Subprocess output | Progress lines | Final output |
|---|---|---|---|---|
| Quiet | `Quiet: true` | Silenced (via `process.WithSilent()`). | Routed to `io.Discard`. | `✓ Closed` on `w`. |
| Verbose | `Verbose: true` | Live (passthrough stdout/stderr). | All checkmark lines. | `✓ Closed`. |
| Normal | default | Silenced. | Concise checkmarks: `✓ Marked task as complete` (if tracker), `✓ Deleted remote branch` (or `→ Remote branch already deleted`), `✓ Fetched origin`, `✓ Returned to detached HEAD`, `✓ Deleted local branch`. | `✓ Closed`. |

The open-PR confirmation prompt (`! PR #42 is still open: <url>` + `Are you sure you want to close this work item? [y/N]:`) always prints regardless of mode — user consent is never suppressed. `--force` skips the prompt.

## Sentinels

```go
var (
    ErrDetached        = errors.New("close cannot run from detached HEAD")
    ErrProtectedBranch = errors.New("close cannot run on protected branch")
    ErrAborted         = errors.New("aborted")
    ErrNoTrackerForURL = errors.New("--task-url requires a configured task-tracker")
)
```

The CLI layer detects these via `errors.Is`.

## Stubbable Dependencies

```go
var (
    getCurrentBranchOrDetached = defaultGetCurrentBranchOrDetached // wraps git.CurrentBranchOrDetached
    isProtectedBranch          = git.IsProtectedBranch
    defaultBranchName          = defaultDefaultBranchName           // wraps git.DefaultBranchNameWithFallback
    getBranchConfig            = defaultGetBranchConfig             // wraps git.GetBranchConfig
    hasRemoteBranch            = defaultHasRemoteBranch             // wraps git.HasRemoteBranch
    hasLocalBranch             = defaultHasLocalBranch              // wraps git.HasLocalBranch
    deleteRemoteBranch         = git.DeleteRemoteBranch
    deleteLocalBranch          = git.DeleteLocalBranch
    fetchOrigin                = git.FetchOrigin
    checkoutDetach             = git.CheckoutDetach

    getGHPRForBranch = gh.GetPRForBranch

    newTracker = tasktracker.NewForActiveProfile

    readLine = defaultReadLine // reads one line from io.Reader; used by confirm
)
```

Trampoline `defaultXxx` functions exist for helpers that take variadic `process.ExecOption` args the close layer does not use.

## Functions

### Run

```go
func Run(w io.Writer, opts Options) error
```

Behavior:

1. Resolve the current branch via `getCurrentBranchOrDetached()`. On detached HEAD, return `ErrDetached`. If the branch is protected (`git.IsProtectedBranch`), return `ErrProtectedBranch` wrapped with the branch name.
2. Resolve tracker. `opts.SkipTracker` forces `tracker = nil`. Tracker construction errors propagate.
3. Resolve the task id (via `resolveTaskID`):
   - When `opts.TaskURL` is set: require a tracker; otherwise return `ErrNoTrackerForURL`. Then `tracker.ParseTaskURL(opts.TaskURL)`.
   - Otherwise: `git.GetBranchConfig(branch, "task-id")`. When tracker is present and the id is empty, print a yellow warning (non-fatal).
4. Unless `opts.Force`: fetch PR state via `gh.GetPRForBranch(branch)`. When the PR is `OPEN`, print a red `!` line with the PR number/URL and prompt for confirmation via `confirm`. A negative response prints `→ Aborted` and returns `ErrAborted`. A PR-lookup error prints a yellow warning and proceeds.
5. Resolve the default branch via `defaultBranchName()` (uses `DefaultBranchNameWithFallback`).
6. Run the best-effort parallel phase via an `errgroup.Group`:
   - When `tracker != nil && taskID != ""`: `tracker.CompleteTask(taskID)`. Errors print a yellow warning; success prints a green confirmation.
   - When `hasRemoteBranch(branch)`: `deleteRemoteBranch(branch)` with the same warn-on-error pattern. When the remote branch is gone, print a cyan "already deleted" line.
   Goroutines always return `nil`; `g.Wait()`'s return is intentionally ignored. Writes to `w` are serialized through a small `syncedWriter` (mutex-guarded) to avoid races.
7. Return to detached HEAD (via `returnToDetachedHead`):
   - When `branch == defBranch`: print a yellow "already on default, skipping deletion" line and return `nil`.
   - Otherwise: `fetchOrigin()` (fatal on error), `checkoutDetach("origin/"+defBranch)` (fatal on error), and when `hasLocalBranch(branch)`: `deleteLocalBranch(branch)` (non-fatal warning).
8. Print a green `✓ Closed` line.

### confirm

```go
func confirm(w io.Writer, message string) (bool, error)
```

Prints `message` to `w`, reads one line from stdin via `readLine`, and returns `true` iff the trimmed-lowercased response equals `"y"` or `"yes"`. Any other response (including empty/Enter) returns `false`. Stdin read errors are wrapped and propagated.

## Error Handling

| Condition | Behavior | CLI translation | Exit |
|-----------|----------|-----------------|-----:|
| Detached HEAD | Returns `ErrDetached` | Wrapped as `*cli.UsageError` with the sentinel message | 1 |
| Protected branch | Returns wrapped `ErrProtectedBranch` | Wrapped as `*cli.UsageError` with the wrapped message | 1 |
| `--task-url` without tracker | Returns `ErrNoTrackerForURL` | Wrapped as `*cli.UsageError` with the sentinel message | 1 |
| User declines open-PR prompt | Prints `→ Aborted`; returns `ErrAborted` | Wrapped as `*process.PassthroughError{Code: 1}` so no additional output appears | 1 |
| `*credentials.NotFoundError` surfaced by tracker | Returns the wrapped error | CLI prints error and `credentials.SetupHint`, wraps as `*cli.UsageError{""}` to suppress crash log | 1 |
| `fetchOrigin` or `checkoutDetach` failure | Returns wrapped error | Bubbles to `main.go` crash handler; subprocess `*process.PassthroughError` propagates | 1 or subprocess code |
| `tracker.CompleteTask` failure | Warning printed; non-fatal | -- | 0 |
| `deleteRemoteBranch` failure | Warning printed; non-fatal | -- | 0 |
| `deleteLocalBranch` failure | Warning printed; non-fatal | -- | 0 |
| `gh.GetPRForBranch` error (without `opts.Force`) | Warning printed; proceeds | -- | 0 |

`*process.PassthroughError` raised by any git or gh subprocess propagates untouched via `main.go`.

## Output

```
! PR #42 is still open: https://github.com/owner/repo/pull/42
Are you sure you want to close this work item? [y/N]: y
✓ Marked task as complete
✓ Deleted remote branch
<git fetch output>
<git checkout output>
✓ Closed
```

Cyan for progress, green for success, yellow for warnings, red for the `!` attention marker. Styles use the Catppuccin Mocha palette, matching `internal/prstatus`.

## Test Coverage

`internal/close/close_test.go` covers:

- Detached HEAD returns `ErrDetached`.
- Protected branch returns wrapped `ErrProtectedBranch`.
- `--task-url` without tracker returns `ErrNoTrackerForURL`; with tracker, uses `ParseTaskURL`.
- Task id from `branch.<name>.task-id` is passed to `CompleteTask`.
- Missing task id with tracker configured prints a warning and skips completion.
- `SkipTracker` bypasses the tracker entirely.
- Open PR + `--force` proceeds without prompting.
- Open PR without `--force` plus affirmative response proceeds.
- Open PR without `--force` plus negative response aborts with `ErrAborted` and prints `→ Aborted`.
- PR-check errors print a warning and proceed.
- `CompleteTask` failure and `deleteRemoteBranch` failure are non-fatal.
- Remote branch already gone prints "already deleted".
- Current branch already equals default branch: skips fetch/checkout/delete with a warning.
- `fetchOrigin` / `checkoutDetach` failure is fatal.
- `deleteLocalBranch` failure is non-fatal.
- `confirm`: accepts `y`/`Y`/`yes`/`Yes`; rejects `n`/empty/other; stdin read errors propagate.
- Open-PR prompt output contains the PR number and URL.

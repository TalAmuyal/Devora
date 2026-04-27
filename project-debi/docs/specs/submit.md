# Submit Package Spec

Package: `internal/submit`

## Purpose

Implement `debi pr submit`: commit local changes, create a tracker task (when a task-tracker is configured), create a feature branch, push it, open a GitHub PR via the `gh` CLI, and, depending on config and flags, enable auto-merge. Must be invoked from a detached HEAD.

## Profile Resolution

The CLI layer calls `cli.ResolveActiveProfile("")` before dispatching to `submit.Run`, so profile-scoped config (such as `task-tracker.provider`) resolved from the current working directory is visible. See [cli.md](./cli.md) and [health.md](./health.md) for the resolution order. There is no `--profile` flag on `submit`; CWD is the sole selector.

## Stability

Stable public API: `Run`, `Options`, `JSONOutput`, `ErrNotDetached`.

## Dependencies

- `devora/internal/git` -- branch creation, commit, push, config helpers, detached-HEAD detection
- `devora/internal/gh` -- `gh` CLI wrapper (`GetRepo`, `CreatePR`, `EnableAutoMergeSquash`, `GetPRForBranch`)
- `devora/internal/tasktracker` -- pluggable task-tracker interface
- `devora/internal/config` -- branch-prefix resolution
- `devora/internal/process` -- passthrough execution of the default browser opener
- `devora/internal/style` -- shared Catppuccin Mocha palette + pre-built lipgloss styles for colored progress output
- `golang.org/x/sync/errgroup` -- concurrent pre-fetch when both WhoAmI and GetRepo are needed

## Types

### Options

```go
type Options struct {
    Message        string // required: commit message, task title, PR title.
    Description    string // optional: appended to PR body.
    Draft          bool   // create PR as draft.
    Blocked        bool   // explicit --blocked: force-skip auto-merge regardless of config.
    ForceAutoMerge bool   // explicit --auto-merge: force auto-merge on regardless of config.
    OpenBrowser    bool   // open the PR URL in the default browser on success.
    JSONOutput     bool   // emit a single JSON object instead of human output.
    SkipTracker    bool   // treat as no tracker configured, even if one is.
    Verbose        bool   // show live git/gh subprocess output.
    Quiet          bool   // print only the final PR URL on stdout.
}
```

`Message` is required. `Description`, when non-empty, is appended to the PR body after a blank line. When a tracker is configured, the PR body starts with `tracker.PRBodyPrefix(task.ID)`; the description follows after a blank line. If no tracker or description is present, the body is empty.

`Verbose` and `Quiet` are mutually exclusive; the CLI layer returns `*cli.UsageError` when both are set. See [Verbosity Modes](#verbosity-modes) for the behavior of each.

## Verbosity Modes

`Run` selects one of four output modes based on `Options`:

| Mode | Trigger | Subprocess output | Progress lines | Final output |
|---|---|---|---|---|
| JSON | `JSONOutput: true` | Silenced (via `process.WithSilent()`). | Routed to `io.Discard`. | Single JSON object on `w`. Warnings on `stderr`. |
| Quiet | `Quiet: true` | Silenced. | Routed to `io.Discard`. | Plain PR URL on `w`, no prefix / no ANSI. Warnings on `stderr`. |
| Verbose | `Verbose: true` | Live (passthrough stdout/stderr). | `→ Staging and committing...`, `✓ Committed`, etc. | `✓ PR created: <url>` + `Branch:` + `Task:` lines. |
| Normal | default | Silenced. | Concise checkmarks: `✓ Committed`, `✓ Task created: <url>`, `✓ Pushed <branch>`, `✓ Auto-merge enabled`. | `✓ PR created` followed by the PR URL on its own line. |

JSON mode takes precedence over every other option. In JSON and Quiet modes, warning messages that would pollute `stdout` (e.g., auto-merge failure, browser-open failure, "PR already exists") are routed to `stderr` instead so the machine-parseable `stdout` stays clean.

### JSONOutput

```go
type JSONOutput struct {
    Number     int    `json:"number"`
    URL        string `json:"url"`
    Title      string `json:"title"`
    Branch     string `json:"branch"`
    BaseBranch string `json:"base_branch"`
    Draft      bool   `json:"draft"`
    TaskURL    string `json:"task_url,omitempty"`
}
```

The shape emitted when `Options.JSONOutput` is true. `Number` and `Title` come from `gh pr view <url>`; when that post-create lookup fails, `Number` is zero and `Title` falls back to `Options.Message`. `TaskURL` is omitted when no tracker task was created.

## Sentinels

### ErrNotDetached

```go
var ErrNotDetached = errors.New("submit requires detached HEAD")
```

Returned when `Run` is called while on a named branch. Wrapped with the current branch name for user-facing output. The CLI layer detects it via `errors.Is` and translates to `*cli.UsageError`.

## Stubbable Dependencies

The package exposes these package-level variables for test injection:

```go
var (
    getCurrentBranchOrDetached = defaultCurrentBranchOrDetached // wraps git.CurrentBranchOrDetached
    addAllAndCommit            = git.AddAllAndCommit
    generateBranchName         = git.GenerateBranchName
    createAndCheckoutBranch    = git.CreateAndCheckoutBranch
    setBranchConfig            = git.SetBranchConfig
    pushSetUpstream            = git.PushSetUpstream

    getGHRepo         = gh.GetRepo
    createGHPR        = gh.CreatePR
    enableGHAutoMerge = gh.EnableAutoMergeSquash

    newTracker = tasktracker.NewForActiveProfile

    getBranchPrefix            = config.GetBranchPrefix
    getRepoAutoMerge           = defaultGetRepoAutoMerge            // wraps git.GetRepoConfigBool("devora.pr.auto-merge")
    getAutoMergeDefaultForRepo = config.GetAutoMergeDefaultForRepo
    openBrowser                = defaultOpenBrowser  // `open` on darwin, `xdg-open` otherwise
    getGHPRView                = defaultGetGHPRView  // wraps gh.GetPRForBranch for JSON post-fetch
)
```

`defaultOpenBrowser` runs `open <url>` on macOS and `xdg-open <url>` elsewhere, in passthrough mode.

## Functions

### Run

```go
func Run(w io.Writer, opts Options) error
```

Entry point for the submit flow.

Behavior:

1. Validate `opts.Message` is non-empty; otherwise return `errors.New("--message is required")`.
2. In JSON mode, route progress output to `io.Discard` so `w` only carries the final JSON payload.
3. Detached-HEAD guard: call `getCurrentBranchOrDetached()`. If a branch is returned, wrap `ErrNotDetached` with the branch name and return it.
4. Stage and commit: print a cyan progress line, then `addAllAndCommit(opts.Message)` in passthrough mode, then a green confirmation.
5. Resolve tracker via `newTracker()`. `opts.SkipTracker` forces `tracker = nil`. Tracker construction errors propagate.
6. Pre-fetch phase -- only calls what is needed:
   - When tracker is present and `JSONOutput` is set: run `tracker.WhoAmI()` and `getGHRepo()` concurrently via `errgroup`.
   - When only tracker is present: call `tracker.WhoAmI()` directly.
   - When only JSON output is set: call `getGHRepo()` directly.
   - Otherwise: skip.
7. When tracker is present, call `tracker.CreateTask(CreateTaskRequest{Title: opts.Message, AssigneeGID: whoami})`. Print a green "Task created: <url>" line.
8. Derive the branch name from `generateBranchName(getBranchPrefix("feature"), opts.Message)`. Create and checkout the branch. When a task was created, call `setBranchConfig(branch, "task-id", task.ID)`. Push with `pushSetUpstream`.
9. Compose the PR body: `tracker.PRBodyPrefix(task.ID)` first (when a task exists), then `"\n\n"` + `opts.Description` (when description is non-empty). Either component may be absent.
10. Call `createGHPR(opts.Message, body, opts.Draft)`. On `*gh.PRAlreadyExistsError`, print a yellow warning (with existing URL when available) and return the wrapped error.
11. Post-PR ops: if `shouldEnableAutoMerge(opts)` returns true, call `enableGHAutoMerge(prURL)`. The helper applies the precedence `--blocked` (off) > `--auto-merge` (on) > per-repo `devora.pr.auto-merge` git config > profile then global `pr.auto-merge` > default on. Errors from `enableGHAutoMerge` print a yellow warning and do not fail the command.
12. Emit output:
    - JSON mode: build `JSONOutput`, call `getGHPRView(prURL)` for `Number`/`Title`; tolerate failure (fallback fields). Write marshaled JSON to `w`.
    - Human mode: print a green "PR created" line, the branch, and (when present) the task URL.
13. When `opts.OpenBrowser`, call `openBrowser(prURL)`; failures print a yellow warning.

The git-config key used to store the task id is `task-id` (provider-agnostic). The provider-specific identifier lives only in the stored value.

## Error Handling

| Condition | Behavior | CLI translation | Exit |
|-----------|----------|-----------------|-----:|
| `opts.Message == ""` | Returns `errors.New("--message is required")` | Reached only when bypassing the CLI; CLI flag parser already emits `*cli.UsageError` for this. | 1 |
| Not on detached HEAD | Returns `ErrNotDetached` wrapped with branch name | Wrapped as `*cli.UsageError` with the wrapped message | 1 |
| `newTracker()` error (bad config) | Returns wrapped `"resolve task-tracker: ..."` | Bubbles to `main.go` crash handler | 1 |
| `*credentials.NotFoundError` surfaced by tracker | Returns the wrapped error unchanged | CLI prints error and `credentials.SetupHint`, wraps as `*cli.UsageError{""}` to suppress crash log | 1 |
| `*gh.PRAlreadyExistsError` | Prints yellow warning to `w`; returns wrapped `"create PR: ..."` | Bubbles to `main.go` crash handler | 1 |
| `gh.ErrGHNotInstalled` (wrapped) | Returns wrapped error | Bubbles to `main.go` crash handler | 1 |
| `gh.EnableAutoMergeSquash` error | Prints yellow warning to progress sink; continues | -- | 0 |
| `openBrowser` error | Prints yellow warning to progress sink; continues | -- | 0 |
| Any other error (git push, commit, etc.) | Wrapped with context and returned | Bubbles to `main.go` crash handler, or `PassthroughError` propagates if raised by a subprocess | 1 or subprocess code |

`*process.PassthroughError` raised by any git or gh subprocess propagates untouched via `main.go`, so subprocess exit codes remain transparent.

## Output

### Human mode

```
→ Staging and committing...
<git add/commit output>
✓ Committed
✓ Task created: https://app.asana.com/1/<workspace>/task/<id>
<git checkout/push output>
✓ PR created: https://github.com/<owner>/<repo>/pull/42
  Branch: feature-add-user-login
  Task:   https://app.asana.com/1/<workspace>/task/<id>
```

Lines are styled via lipgloss using the shared pre-built styles from `internal/style`: green for success, cyan for progress, yellow for warnings, red for errors.

### JSON mode

```json
{
  "number": 42,
  "url": "https://github.com/owner/repo/pull/42",
  "title": "add user login",
  "branch": "feature-add-user-login",
  "base_branch": "main",
  "draft": false,
  "task_url": "https://app.asana.com/1/<workspace>/task/<id>"
}
```

`task_url` is omitted when no task was created.

## Auto-merge configuration

`pr.auto-merge` resolves with per-repo > profile > global > built-in default (`true`) precedence. The per-repo layer lives in git's local config under the key `devora.pr.auto-merge` and is shared across all linked worktrees of a clone (including bare clones). The profile/global layers live in the standard Devora JSON config. When the key is unset or of the wrong type at a given layer, resolution falls through to the next layer.

Per-invocation flags always win over every layer: `--blocked` forces off, `--auto-merge` forces on, and the two are mutually exclusive at the CLI layer.

The three scopes can be managed via the `debi pr auto-merge <enable|disable|reset|show> [--scope=repo|profile|global] [--json]` command; see [cli.md](./cli.md) for its flags, verbs, output, and error conditions.

At the package level this is implemented by plumbing a `getRepoAutoMerge` callback (reading `devora.pr.auto-merge` via `git.GetRepoConfigBool`) into `config.GetAutoMergeDefaultForRepo`, so the `config` package stays free of dependencies on `internal/git`.

## Test Coverage

`internal/submit/submit_test.go` covers:

- Missing `--message` returns an error.
- Non-detached HEAD returns `ErrNotDetached` (and propagates lookup errors from `getCurrentBranchOrDetached`).
- No-tracker happy path: skips task creation, body omits prefix, branch config is not set.
- No-tracker path with description: body is the description alone.
- Tracker happy path: task is created, branch config records `task-id`, PR body contains `PRBodyPrefix` + description, `setBranchConfig` sees the provider-supplied task id.
- Tracker path without description: body is just the prefix.
- `SkipTracker` bypasses an otherwise-configured tracker.
- `newTracker()` error propagates.
- `--draft` is forwarded to `CreatePR`.
- Auto-merge matrix:
  - `--blocked` skips auto-merge; default path (no flags, no config) enables it.
  - Config `pr.auto-merge: false` with no flags skips auto-merge; with `--auto-merge` enables it.
  - Config `pr.auto-merge: true` with `--blocked` skips auto-merge; with no flags enables it.
  - Config unset with `--auto-merge` enables it; with no flags enables it.
  - CLI rejects `--blocked` + `--auto-merge` together with a `*UsageError` naming both flags.
- `PRAlreadyExistsError` with and without extracted URL: warning is printed, wrapped error returned.
- Auto-merge failure is non-fatal (warning printed, command returns nil).
- JSON mode: happy path; empty `task_url` when no tracker; tolerates `getGHPRView` failure by emitting minimal payload.
- `OpenBrowser` calls the default opener when set and not when unset.
- Pre-fetch phase: runs `WhoAmI` and `GetRepo` concurrently when both are needed, or individually, or not at all, as appropriate.
- `GetBranchPrefix` called with `"feature"` fallback.
- Commit receives the message verbatim; branch creation and push happen in order.
- Human summary includes URL, branch, and (when present) task URL.
- `CreateTask` failure propagates.

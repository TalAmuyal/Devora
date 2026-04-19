# TUI Operations

Package: `internal/tui`

This spec documents the TUI's business logic and data orchestration -- everything the TUI does that involves complex logic, domain rules, or concurrency. For UI structure, page state machine, styling, and rendering details, see [tui.md](tui.md).

## Dependencies

- `devora/internal/config` -- profile access, workspaces root, default app, timeout
- `devora/internal/workspace` -- workspace enumeration, state queries, git operations, worktree creation/deletion, CLAUDE.md management
- `devora/internal/terminal` -- backend creation, session listing, session creation and attachment
- `devora/internal/task` -- task file creation

---

## Workspace Data Types

### WorkspaceCategory

```go
type WorkspaceCategory int

const (
    CategoryActiveWithSession WorkspaceCategory = iota
    CategoryActiveNoSession
    CategoryInactive
    CategoryInvalid
)
```

The four categories form a strict priority order used for sorting: `ActiveWithSession` (0) > `ActiveNoSession` (1) > `Inactive` (2) > `Invalid` (3).

### WorkspaceInfo

```go
type WorkspaceInfo struct {
    Path            string
    Name            string                   // filepath.Base(Path)
    Category        WorkspaceCategory
    TaskTitle       string                   // from task.json; empty for inactive/invalid
    Repos           []string                 // directory names within the workspace
    RepoGitStatuses map[string]RepoGitStatus // keyed by repo name
    InvalidReason   string                   // non-empty only for CategoryInvalid
}
```

### RepoGitStatus

```go
type RepoGitStatus struct {
    Branch  string // current branch name, or "HEAD" for detached state
    IsClean bool   // true if `git status --porcelain` output is empty
}
```

### NewTaskResult

```go
type NewTaskResult struct {
    RepoNames []string
    TaskName  string
}
```

Produced by the new-task form after validation. `RepoNames` contains the user's selected repos; `TaskName` is the trimmed task name.

### WorkspaceReadyResult

```go
type WorkspaceReadyResult struct {
    WorkspacePath string
    SessionName   string
}
```

Returned as part of `AppResult` when a new workspace is created. The CLI uses this to call `CreateAndAttachSession`.

---

## Workspace Data Gathering

`gatherWorkspaceInfos()` is the core data collection function. It runs as a background Bubble Tea command triggered by `loadWorkspacesCmd`. The function performs three categories of work concurrently.

### Parallel Architecture

```
goroutine 1 (session listing)     goroutine 0 (main)         goroutine N (git queries)
  |                                  |                           |
  | backend.ListSessions()           | workspace.GetWorkspaces   |
  |                                  | skip locked               |
  |                                  | GetWorkspaceRepos each    |
  |                                  |                           | GetRepoBranch(repo)
  |                                  |                           | IsRepoClean(repo)
  |                                  |                           |
  v                                  v                           v
  sessions []Session                 pending []pendingWS         gitCh <- gitResult
         \                            |                         /
          +------ merge phase --------+------------------------+
                                      |
                                      v
                                 []WorkspaceInfo
```

### Error Handling

`gatherWorkspaceInfos` returns `([]WorkspaceInfo, error)`. Errors from `config.GetWorkspacesRootPath()` and `workspace.GetWorkspaces()` are fatal and returned to the caller. The caller (`loadWorkspacesCmd`) surfaces these as `workspaceLoadErrorMsg`, which `AppModel.Update` handles by clearing the loading state and showing an error notification.

Per-workspace errors are handled gracefully:
- `workspace.GetWorkspaceRepos()` failure: the workspace is included with an empty repo list.
- `workspace.GetSessionWorkingDirectory()` failure: falls back to the workspace path for session matching.
- Git query errors (branch/clean): the repo is silently excluded from the status map.

### Step 1: Start Session Listing

A goroutine creates the terminal backend (via `terminal.NewBackend()`) and calls `backend.ListSessions()`. This runs concurrently with workspace scanning.

### Step 2: Scan Workspaces

On the calling goroutine:

1. Read the workspaces root from `config.GetWorkspacesRootPath()`. Returns error on failure.
2. Call `workspace.GetWorkspaces(root)` to enumerate all `ws-N` directories. Returns error on failure.
3. For each workspace, check `workspace.IsWorkspaceLocked()`. Locked workspaces are skipped entirely.
4. For each unlocked workspace, call `workspace.GetWorkspaceRepos()` to list repo directories. On failure, the workspace is included with an empty repo list.

### Step 3: Parallel Git Queries

For every repo in every unlocked workspace, spawn a goroutine that:
- Calls `workspace.GetRepoBranch(repoPath)` to get the current branch.
- Calls `workspace.IsRepoClean(repoPath)` to check for uncommitted changes.
- Sends results to a shared `gitCh` channel.

Results are collected into a per-workspace map of `RepoGitStatus` structs. Repos that produce errors are silently excluded from the status map.

### Step 4: Wait and Merge

1. Wait for the git channel to close (all git goroutines complete).
2. Wait for the session-listing goroutine to complete.
3. Build a `sessionPaths` map from session `RootPath` values for O(1) lookup.

### Step 5: Category Assignment

For each workspace, determine the category using this decision tree:

```
Is initialized?
  No  -> CategoryInvalid (InvalidReason = "Not initialized")
  Yes -> Has task.json?
           No  -> CategoryInactive
           Yes -> Read task title from task.json
                  Compute working directory (see Working Directory Resolution)
                  Is working directory in sessionPaths?
                    Yes -> CategoryActiveWithSession
                    No  -> CategoryActiveNoSession
```

### Working Directory Resolution

The working directory for session matching is determined by calling `workspace.GetSessionWorkingDirectory(wsPath)` (see [workspace.md](workspace.md#getsessionworkingdirectory)):

- If the workspace has exactly **one** repo: returns the repo path within the workspace.
- Otherwise (zero or multiple repos): returns the workspace path.

If `GetSessionWorkingDirectory` fails, the workspace path is used as the fallback.

---

## Workspace Creation Flow

`createWorkspaceCmd(result NewTaskResult, progressCh chan<- tea.Msg)` runs as a Bubble Tea command after the user submits the new-task form. It reports progress through `progressCh`, which is read by `listenCreation` commands chained via `AppModel.Update`. The command itself returns either `creationDoneMsg` or `creationErrorMsg` as its final message; all intermediate progress goes through the channel.

### Progress Architecture

When `NewTaskResult` is received:
1. `AppModel.Update` creates a buffered channel (`make(chan tea.Msg, 20)`) and stores it as `m.creationCh`.
2. It returns `tea.Batch(createWorkspaceCmd(..., ch), listenCreation(ch), spinnerTick())` — the spinner tick starts the animated progress indicator.
3. `createWorkspaceCmd` runs in a goroutine, sending `creationStatusMsg`, `repoAddedMsg`, and `repoReadyMsg` through the channel as work progresses. It closes the channel on exit.
4. `listenCreation(ch)` reads one message from the channel and returns it to Bubble Tea:
   ```go
   func listenCreation(ch <-chan tea.Msg) tea.Cmd {
       return func() tea.Msg {
           msg, ok := <-ch
           if !ok {
               return nil
           }
           return msg
       }
   }
   ```
5. Each progress message handler in `AppModel.Update` schedules another `listenCreation(m.creationCh)` to read the next message.
6. Terminal messages (`creationDoneMsg`, `creationErrorMsg`) are returned directly from the command (not through the channel), so no further listening is needed.

### Step 1: Search for Reusable Workspace

```go
wsPath, _ := workspace.SearchAvailableWorkspace(result.RepoNames, workspacesRoot)
```

Sends `creationStatusMsg{text: "Resolving workspace..."}` before this step.

Searches for an unlocked, inactive workspace whose repo set matches `result.RepoNames` exactly (order-independent). If found, reuses it -- skips directory creation entirely and proceeds to Step 3.

### Step 2: Create New Workspace (when no reusable workspace found)

1. **Create directory**: Sends `creationStatusMsg{text: "Creating workspace directory..."}`. Calls `workspace.CreateWorkspaceDirectory(workspacesRoot)` to allocate the next `ws-N` directory.
2. **Acquire lock**: `workspace.LockWorkspace(wsPath)` acquires an exclusive advisory lock. Released via `defer unlock.Close()`.
3. **Write CLAUDE.md**: If more than one repo is selected (`len(result.RepoNames) > 1`), write a workspace-level `CLAUDE.md` via `workspace.WriteWorkspaceCLAUDEMD`. Returns `creationErrorMsg` on failure.
4. **Announce repos**: Sends `creationStatusMsg{text: "Preparing worktrees..."}`, then sends `repoAddedMsg{name}` for each repo.
5. **Parallel worktree creation**: For each repo in `result.RepoNames`, spawn a goroutine calling `workspace.MakeAndPrepareWorkTree(wsPath, repoName, repoName)`. The `worktreeDirName` equals the `repoName` (no postfix). Collect results via an internal channel. As each worktree completes, send `repoReadyMsg{name}` through the progress channel. If any worktree creation fails, return `creationErrorMsg` immediately.
6. **Mark initialized**: `workspace.MarkInitialized(wsPath)` creates the `initialized` marker file. Returns `creationErrorMsg` on failure.

### Step 3: Create Task

Sends `creationStatusMsg{text: "Creating task..."}`. Writes `task.json` via `task.Create(result.TaskName, taskPath)` where `taskPath` is `workspace.GetWorkspaceTaskPath(wsPath)`.

### Result

On success, return `creationDoneMsg{workspacePath, sessionName}` where `sessionName` is the task name. This triggers `tea.Quit` with an `AppResult` containing a `WorkspaceReadyResult`.

On any error in Steps 1-3, return `creationErrorMsg{err}`. The creation page displays the error.

---

## Deactivation Blocking Rules

Before deactivating a workspace, `handleDeactivateRequest()` checks the workspace state and three blocking conditions. Violations are surfaced as error notifications (4-second ephemeral banners). The checks are evaluated in order; the first violation blocks the deactivation.

### Precondition: Workspace Must Be Active

Deactivation is only available for active workspaces. If the selected workspace is inactive, the notification "Workspace is already inactive" is shown. If the selected workspace is invalid, the notification "Cannot deactivate an invalid workspace" is shown. In both cases, the deactivation is rejected without checking the blocking rules below.

### Rule 1: Active Session

```
Cannot deactivate: workspace has an active terminal session
```

Triggered when `info.Category == CategoryActiveWithSession`.

### Rule 2: Branch Not Detached

```
Cannot deactivate: repo '<name>' is not on HEAD
```

For each repo (in alphabetical order), if `status.Branch != "HEAD"`, the workspace cannot be deactivated.

### Rule 3: Uncommitted Changes

```
Cannot deactivate: repo '<name>' has uncommitted changes
```

For each repo (in alphabetical order), if `!status.IsClean`, the workspace cannot be deactivated.

### When All Checks Pass

The workspace list calls `workspace.DeactivateWorkspace(info.Path)` directly (no confirmation page). On success, emits `deactivateCompleteMsg`, which triggers a workspace list refresh and a success notification. On error, emits `deactivateErrorMsg`, which triggers an error notification.

---

## Delete Blocking Rules

Before navigating to the delete confirmation page, `handleDeleteRequest()` checks the workspace state and three blocking conditions on the selected workspace. Violations are surfaced as error notifications (4-second ephemeral banners). The checks are evaluated in order; the first violation blocks the delete.

### Precondition: Workspace Must Be Inactive or Invalid

Deletion is only available for inactive or invalid workspaces. If the selected workspace is active, the notification "Deactivate the workspace first" is shown. The deletion is rejected without checking the blocking rules below.

### Rule 1: Active Session

```
Cannot delete: workspace has an active terminal session
```

Triggered when `info.Category == CategoryActiveWithSession`. A workspace with a running terminal session must not be deleted.

### Rule 2: Branch Not Detached

```
Cannot delete: repo '<name>' is not on HEAD
```

For each repo (in alphabetical order), if `status.Branch != "HEAD"`, the workspace cannot be deleted. Worktrees are created in detached HEAD state; a non-HEAD branch indicates work has been done that may not be merged.

### Rule 3: Uncommitted Changes

```
Cannot delete: repo '<name>' has uncommitted changes
```

For each repo (in alphabetical order), if `!status.IsClean`, the workspace cannot be deleted. Dirty repos may have unsaved work.

### When All Checks Pass

The workspace list emits `requestDeleteMsg{info}`, which transitions to the `PageDeleteConfirm` page. The user confirms with `y` or cancels with `n`/`escape`. Confirmation calls `workspace.DeleteWorkspace(info.Path)`, which removes all git worktrees (in parallel) and deletes the workspace directory (see [workspace.md](workspace.md)).

---

## Add Repo Flow

The add-repo feature runs as a **standalone** `tea.Program`, separate from the main `AppModel`. It is invoked from the CLI `add` command, not from within the main TUI.

### Entry Point

```go
func RunAddRepo(themePath string, workspacePath string, repoNames []string) error
```

Creates an `AddRepoModel` and runs it in its own Bubble Tea program.

### Form Fields

The form has three focus stops, cycled with `tab`/`shift+tab`:

1. **Repo list** (`fieldAddRepoList`): A `ListModel` displaying registered repo names. The user selects one repo.
2. **Postfix input** (`fieldAddRepoPostfix`): An optional text input. If provided, the worktree directory will be named `<repoName>-<postfix>` instead of just `<repoName>`.
3. **Submit button** (`fieldAddRepoSubmit`).

### Collision Checking

Before creating the worktree, the submit handler checks if the target directory already exists:

```go
targetPath := filepath.Join(workspacePath, worktreeDirName)
```

If the path exists on disk, an error is shown: `"Directory '<name>' already exists. Use a different postfix."` The user must provide a different postfix.

### Worktree Creation

On submit (when validation passes):

1. Call `workspace.MakeAndPrepareWorkTree(workspacePath, repoName, worktreeDirName)`. On error, emit `addRepoErrorMsg`.
2. Call `workspace.EnsureWorkspaceCLAUDEMD(workspacePath)` -- writes `CLAUDE.md` if the workspace now has more than one repo and does not already have one. On error, emit `addRepoErrorMsg` (the error message notes that the worktree was created successfully).
3. On success, emit `addRepoDoneMsg` which triggers `tea.Quit`.

---

## Profile Cycling

`cycleProfileCmd()` returns a Bubble Tea command that switches to the next profile in the list.

### Precondition

Requires at least 2 profiles. If `len(config.GetProfiles()) < 2`, returns `nil` (no-op).

### Behavior

1. Get the list of profiles from `config.GetProfiles()`.
2. Find the index of the currently active profile by comparing `RootPath`.
3. Compute the next index: `(currentIdx + 1) % len(profiles)`.
4. Call `config.SetActiveProfile(&profiles[nextIdx])`.
5. Return `profileActivatedMsg{}`.

If no profile is currently active, activates the first profile (`profiles[0]`).

### Effect on AppModel

When `AppModel` receives `profileActivatedMsg`:

1. Re-reads `repoNames` from `config.GetRegisteredRepoNames()`.
2. Updates `profileName` from `config.GetActiveProfile()`.
3. Triggers a full workspace reload via `loadWorkspacesCmd()`.
4. Navigates to `PageWorkspaceList`.

---

## Session Attachment

`CreateAndAttachSession` is a post-TUI bridge function. It is not a Bubble Tea component -- it runs after the TUI exits and creates/attaches a terminal session for the selected or newly created workspace.

```go
func CreateAndAttachSession(wsPath, sessionName string) error
```

### Steps

1. **Create backend**: `terminal.NewBackend()`.
2. **Read default app**: `config.GetDefaultTerminalApp("shell")`. The `"shell"` fallback is a reserved sentinel interpreted by the terminal layer as "launch a bare login/interactive shell" (see [terminal.md](terminal.md)).
3. **Read timeout**: `config.TerminalSessionCreationTimeoutSeconds(3)`.
4. **Determine working directory**: Call `workspace.GetSessionWorkingDirectory(wsPath)`.
5. **Create and attach**: Delegate to `terminal.CreateAndAttach(backend, sessionName, workDir, app, timeout)`.

The `terminal.CreateAndAttach` function first attempts to attach to an existing session at the working directory. If none exists, it creates a new session and polls until it appears (with timeout). See [terminal.md](terminal.md) for details.

---

## Workspace Filtering and Sorting

### FilterMode

```go
type FilterMode int

const (
    FilterActive   FilterMode = iota + 1
    FilterInactive
    FilterAll
)
```

The default filter mode is `FilterActive`. Users switch modes with keys `1` (Active), `2` (Inactive), `3` (All).

### Filter Logic

| Mode | Included Categories |
|------|-------------------|
| `FilterActive` | `CategoryActiveWithSession`, `CategoryActiveNoSession` |
| `FilterInactive` | `CategoryInactive` only |
| `FilterAll` | All categories |

### Sort Order

In `FilterAll` mode, workspaces are sorted by category priority using `sort.SliceStable`:

```
CategoryActiveWithSession (0) < CategoryActiveNoSession (1) < CategoryInactive (2) < CategoryInvalid (3)
```

Within the same category, the original order from `gatherWorkspaceInfos` is preserved (stable sort). In `FilterActive` and `FilterInactive` modes, no additional sorting is applied -- items appear in the order returned by `gatherWorkspaceInfos`.

The sort logic is implemented inline in `filteredWorkspaces()` within `WorkspaceListModel`.

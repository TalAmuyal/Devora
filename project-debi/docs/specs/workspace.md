# Workspace Package Spec

Package: `internal/workspace`

## Overview

The workspace package manages workspace directories, their lifecycle states, git worktree operations, and file locking. A workspace is a directory under a profile's `workspaces/` root containing git worktrees for development tasks.

## Dependencies

- `devora/internal/config` -- profile access, registered repos, prepare command
- `devora/internal/git` -- `DefaultBranchName` utility (shared with git shortcuts)
- `devora/internal/process` -- shell command execution
- `golang.org/x/sync/errgroup` -- parallel worktree removal in `DeleteWorkspace`

## Constants

```go
const LockFileName = ".creation-lock"
const InitializedMarkerName = "initialized"
const TaskFileName = "task.json"
const ClaudeMDFileName = "CLAUDE.md"
const WorkspacePrefix = "ws-"
```

```go
const WorkspaceCLAUDEMDContent = "This is a workspace with multiple repositories.\n" +
    "Run `find . -maxdepth 2 -name .git | sed 's|/\\.git$||'` to see which repos are available for work or reference.\n"
```

## Workspace Directory Structure

```
<workspaces-root>/ws-N/
  |- .creation-lock    (advisory lock file)
  |- CLAUDE.md         (workspace context for AI agents; multi-repo only)
  |- initialized       (marker file)
  |- task.json         (task metadata, indicates active task)
  |- repo-a/           (git worktree)
  |- repo-b/           (git worktree)
```

## Workspace States

A workspace has exactly one of three states, determined by the presence of marker files:

| State | Condition |
|-------|-----------|
| **Active** | `initialized` marker exists AND `task.json` exists |
| **Inactive** | `initialized` marker exists AND `task.json` does NOT exist |
| **Invalid** | `initialized` marker does NOT exist |

Locked workspaces (those with `.creation-lock` held by another process) are excluded from all filtered query results (`GetActiveWorkspaces`, `GetInactiveWorkspaces`, `GetInvalidWorkspaces`, `SearchAvailableWorkspace`).

---

## File Locking

Advisory file locking on the `.creation-lock` file within a workspace directory. Uses `syscall.Flock` (Unix-only; acceptable since Devora targets macOS/Linux).

### LockWorkspace

```go
func LockWorkspace(workspacePath string) (io.Closer, error)
```

Acquires an exclusive advisory lock on `<workspacePath>/.creation-lock`.

- Opens (or creates) the lock file with `os.O_WRONLY|os.O_CREATE` and mode `0666`.
- Calls `syscall.Flock(fd, syscall.LOCK_EX)` to acquire the exclusive lock. Blocks until the lock is available.
- Returns the open `*os.File` as an `io.Closer`. The caller releases the lock by calling `Close()` on the returned value.
- On error (open or flock), closes the file descriptor and returns the error.

### IsWorkspaceLocked

```go
func IsWorkspaceLocked(workspacePath string) bool
```

Non-blocking check for whether the workspace's `.creation-lock` is currently held by another process.

- If the lock file does not exist, returns `false`.
- Opens the lock file, attempts `syscall.Flock(fd, syscall.LOCK_EX|syscall.LOCK_NB)`.
- If the flock call succeeds (lock acquired), immediately unlocks (`syscall.LOCK_UN`), closes the file, and returns `false`.
- If the flock call fails with `syscall.EWOULDBLOCK`, closes the file and returns `true`.
- On any other error, closes the file and returns `false` (treat inability to check as unlocked).

---

## Path Helpers

### GetWorkspaceRepoPath

```go
func GetWorkspaceRepoPath(workspacePath string, repoName string) string
```

Returns `filepath.Join(workspacePath, repoName)`.

### GetWorkspaceTaskPath

```go
func GetWorkspaceTaskPath(workspacePath string) string
```

Returns `filepath.Join(workspacePath, TaskFileName)`.

---

## State Query Functions

### IsInitialized

```go
func IsInitialized(workspacePath string) bool
```

Returns `true` if the `initialized` marker file exists at `getInitializationMarkerPath(workspacePath)`. Uses `os.Stat`; returns `false` on any error.

### HasTask

```go
func HasTask(workspacePath string) bool
```

Returns `true` if `task.json` exists at `GetWorkspaceTaskPath(workspacePath)`. Uses `os.Stat`; returns `false` on any error.

### IsActive

```go
func IsActive(workspacePath string) bool
```

Returns `IsInitialized(workspacePath) && HasTask(workspacePath)`.

### IsInactive

```go
func IsInactive(workspacePath string) bool
```

Returns `IsInitialized(workspacePath) && !HasTask(workspacePath)`.

### IsInvalid

```go
func IsInvalid(workspacePath string) bool
```

Returns `!IsInitialized(workspacePath)`.

---

## Workspace Enumeration

### GetWorkspaces

```go
func GetWorkspaces(workspacesRoot string) ([]string, error)
```

Lists all `ws-N` directories under the given root. Returns absolute paths.

- If the root directory does not exist, returns an empty slice and `nil` error.
- Reads directory entries from `workspacesRoot`.
- Filters to entries that are directories and whose names start with `"ws-"`.
- Returns the absolute paths of matching directories (i.e., `filepath.Join(workspacesRoot, entry.Name())`).
- Order is not guaranteed.

### GetWorkspaceRepos

```go
func GetWorkspaceRepos(workspacePath string) ([]string, error)
```

Returns the names of non-dot-prefixed directories within the workspace. These represent the git worktrees (repos) in the workspace.

- Reads directory entries from `workspacePath`.
- Filters to entries that are directories and whose names do NOT start with `"."`.
- Returns the directory names (not full paths).
- Order is not guaranteed.

---

## Filtered Queries

These functions query workspaces from the active profile's workspaces root (obtained via `config.GetWorkspacesRootPath()`). They exclude locked workspaces.

### GetActiveWorkspaces

```go
func GetActiveWorkspaces() ([]string, error)
```

Returns paths of workspaces that are active (initialized AND has task) and NOT locked.

- Calls `GetWorkspaces(config.GetWorkspacesRootPath())`.
- Filters each workspace: `!IsWorkspaceLocked(ws) && IsActive(ws)`.

### GetInactiveWorkspaces

```go
func GetInactiveWorkspaces() ([]string, error)
```

Returns paths of workspaces that are inactive (initialized AND no task) and NOT locked.

- Calls `GetWorkspaces(config.GetWorkspacesRootPath())`.
- Filters each workspace: `!IsWorkspaceLocked(ws) && IsInactive(ws)`.

### GetInvalidWorkspaces

```go
func GetInvalidWorkspaces() ([]string, error)
```

Returns paths of workspaces that are invalid (not initialized) and NOT locked.

- Calls `GetWorkspaces(config.GetWorkspacesRootPath())`.
- Filters each workspace: `!IsWorkspaceLocked(ws) && IsInvalid(ws)`.

---

## Workspace Search

### SearchAvailableWorkspace

```go
func SearchAvailableWorkspace(repos []string, workspacesRoot string) (string, error)
```

Finds an unlocked, inactive workspace whose set of repos matches `repos` exactly.

- Calls `GetWorkspaces(workspacesRoot)`.
- For each workspace, checks:
  1. `!IsWorkspaceLocked(ws)`
  2. `IsInactive(ws)`
  3. `GetWorkspaceRepos(ws)` returns a set of names equal to `repos` (order-independent comparison)
- Returns the path of the first matching workspace.
- Returns an empty string and `nil` error if no match is found.

The set comparison: convert both `repos` and the result of `GetWorkspaceRepos` into sorted slices (or use a map-based comparison) to check exact equality regardless of order.

---

## Workspace Creation

### CreateWorkspaceDirectory

```go
func CreateWorkspaceDirectory(workspacesRoot string) (string, error)
```

Creates the next available `ws-N` directory under `workspacesRoot`.

- Starts from N=1, increments until finding a name where `filepath.Join(workspacesRoot, fmt.Sprintf("ws-%d", n))` does not exist.
- Creates the directory with `os.MkdirAll` (creates intermediate directories if needed, but checks existence first via the incrementing loop to avoid overwriting).
- Returns the absolute path to the created directory.
- Returns an error if directory creation fails.

### MarkInitialized

```go
func MarkInitialized(workspacePath string) error
```

Creates the `initialized` marker file in the workspace directory.

- Creates an empty file at `getInitializationMarkerPath(workspacePath)` using `os.Create` (or `os.WriteFile` with empty content and `0666` mode).

---

## CLAUDE.md Management

### WriteWorkspaceCLAUDEMD

```go
func WriteWorkspaceCLAUDEMD(workspacePath string) error
```

Writes the static CLAUDE.md template to the workspace root.

- Writes `WorkspaceCLAUDEMDContent` to `filepath.Join(workspacePath, ClaudeMDFileName)`.
- Uses `os.WriteFile` with mode `0666`.
- Overwrites any existing file.

### EnsureWorkspaceCLAUDEMD

```go
func EnsureWorkspaceCLAUDEMD(workspacePath string) error
```

Writes CLAUDE.md only if it does not already exist AND the workspace has more than one repo.

- Checks if `filepath.Join(workspacePath, ClaudeMDFileName)` exists. If it does, returns `nil` (no-op).
- Calls `GetWorkspaceRepos(workspacePath)`. If the count is <= 1, returns `nil`.
- Otherwise, calls `WriteWorkspaceCLAUDEMD(workspacePath)`.

---

## Git Operations

### MakeAndPrepareWorkTree

```go
func MakeAndPrepareWorkTree(workspacePath string, repoName string, worktreeDirName string) error
```

Creates a git worktree in the workspace and prepares it for use.

Steps (executed sequentially):

1. **Resolve repo path**: Look up `repoName` in `config.GetRegisteredRepos()` by matching `filepath.Base()`. Returns an error if no matching repo is found.
2. **Get default branch**: Call `git.DefaultBranchName(process.WithCwd(repoPath))` to get the branch name.
3. **Fetch**: Run `git fetch origin <branch>` in the source repo directory.
   ```
   process.GetOutput([]string{"git", "fetch", "origin", branch}, process.WithCwd(repoPath))
   ```
4. **Create worktree**: Run `git worktree add --detach <targetPath> origin/<branch>` in the source repo directory.
   ```
   targetPath = filepath.Join(workspacePath, worktreeDirName)
   process.GetOutput([]string{"git", "worktree", "add", "--detach", targetPath, "origin/" + branch}, process.WithCwd(repoPath))
   ```
   The target path's parent directory should be created if it doesn't exist (use `os.MkdirAll` on the parent before this step).
5. **Trust mise**: If `mise` is found on the PATH (via `exec.LookPath`), run `mise trust` in the worktree directory. If `mise` is not installed, log a warning to stderr and to a log file at `/tmp/devora-debi-error-TIMESTAMP.log` (timestamp format `20060102_150405`), then continue without error.
   ```
   handleMiseTrust(targetPath)
   ```
6. **Prepare**: If `config.GetPrepareCommand()` returns a non-nil value, run that command in the worktree directory.
   ```
   process.GetShellOutput(prepareCommand, process.WithCwd(targetPath))
   ```
   The prepare command comes from the config resolution chain as a string. Execute it as a shell command (via `process.GetShellOutput` or split into args as appropriate based on the process package API).

Returns an error if any step fails, propagating the underlying error. The exception is step 5 (Trust mise): if `mise` is not installed, a warning is logged and execution continues.

### GetRepoBranch

```go
func GetRepoBranch(repoPath string) (string, error)
```

Returns the current branch name of a git repo, or `"HEAD"` if in detached HEAD state.

- Runs `git rev-parse --abbrev-ref HEAD` in `repoPath` via `process.GetOutput`.
- Returns the trimmed output string.

### IsRepoClean

```go
func IsRepoClean(repoPath string) (bool, error)
```

Returns `true` if the repo has no uncommitted changes.

- Runs `git status --porcelain` in `repoPath` via `process.GetOutput`.
- Returns `true` if the output is an empty string, `false` otherwise.

### IsGitRepo

```go
func IsGitRepo(path string) bool
```

Returns `true` if the given path is inside a git repository.

- Runs `git rev-parse --git-dir` in `path` via `process.GetOutput`.
- Returns `true` if the command succeeds, `false` if it returns an error.

---

## Workspace Deactivation

### DeactivateWorkspace

```go
func DeactivateWorkspace(workspacePath string) error
```

Transitions a workspace from Active to Inactive by removing `task.json`.

- Calls `os.Remove` on `GetWorkspaceTaskPath(workspacePath)`.
- If `task.json` does not exist (`os.IsNotExist`), returns `nil` (idempotent).
- Returns an error if the removal fails for any other reason.

The workspace's git worktrees, `initialized` marker, and all other files are preserved.

---

## Workspace Deletion

### DeleteWorkspace

```go
func DeleteWorkspace(workspacePath string) error
```

Removes all git worktrees in the workspace and deletes the workspace directory.

Steps:

1. Call `GetWorkspaceRepos(workspacePath)` to get all repo names.
2. For each repo, run `git worktree remove .` in the repo's directory (`GetWorkspaceRepoPath(workspacePath, repoName)`) **in parallel** using `errgroup.Group`.
   ```go
   g, _ := errgroup.WithContext(context.Background())
   for _, repo := range repos {
       g.Go(func() error {
           _, err := process.GetOutput(
               []string{"git", "worktree", "remove", "."},
               process.WithCwd(GetWorkspaceRepoPath(workspacePath, repo)),
           )
           return err
       })
   }
   if err := g.Wait(); err != nil {
       return err
   }
   ```
3. After all worktree removals succeed, call `os.RemoveAll(workspacePath)` to delete the workspace directory.
4. Returns an error if any worktree removal fails (the errgroup collects the first error) or if `os.RemoveAll` fails.

---

## Workspace Detection from CWD

### ResolveWorkspaceFromCWD

```go
func ResolveWorkspaceFromCWD(cwd string) (*config.Profile, string, error)
```

Determines whether the given working directory is inside a known workspace. Used by the `add` command to detect the current workspace context.

Steps:

1. Resolve `cwd` to an absolute path (use `filepath.Abs` and `filepath.EvalSymlinks`).
2. Call `config.GetProfiles()` to get all registered profiles.
3. For each profile:
   a. Compute the workspaces root: `config.WorkspacesRootForProfile(profile)`.
   b. Resolve the workspaces root to an absolute path.
   c. Check if the resolved `cwd` is under the workspaces root using `filepath.Rel` -- if `filepath.Rel(workspacesRoot, resolvedCWD)` succeeds and does NOT start with `".."`, the CWD is under this root.
   d. Extract the first path component from the relative path (the `ws-N` directory name).
   e. Construct `workspacePath = filepath.Join(workspacesRoot, workspaceName)`.
   f. Verify the path is a directory and `IsInitialized(workspacePath)` is `true`.
   g. If all checks pass, return the profile (as `*config.Profile`) and the workspace path.
4. If no profile matches, return `nil, "", nil` (no error, just not found).

---

## Session Working Directory

### GetSessionWorkingDirectory

```go
func GetSessionWorkingDirectory(workspacePath string) (string, error)
```

Returns the working directory that should be used for a terminal session attached to this workspace.

- Calls `GetWorkspaceRepos(workspacePath)`.
- If there is exactly one repo, returns `GetWorkspaceRepoPath(workspacePath, repoName)`.
- Otherwise (zero or multiple repos), returns `workspacePath`.

---

## Concurrency

All exported functions in this package are safe to call concurrently. They do not mutate shared state; they operate on the filesystem and run git commands.

Callers are expected to use goroutines and `errgroup` for parallelism. Specific patterns:

- **Parallel worktree creation**: The caller (TUI workspace creation flow) launches multiple `MakeAndPrepareWorkTree` calls as concurrent goroutines.
- **Parallel repo status checks**: The caller (TUI workspace list gathering) launches multiple `GetRepoBranch` and `IsRepoClean` calls concurrently for all repos across all workspaces.
- **Parallel worktree removal**: `DeleteWorkspace` handles this internally using `errgroup`.

---

## File Organization

The workspace package is split across two files:

| File | Contents |
|------|----------|
| `workspace.go` | Constants, locking, path helpers, state queries, enumeration, filtered queries, search, creation, CLAUDE.md management, deletion, CWD resolution, session working directory |
| `worktree.go` | Git operations: `MakeAndPrepareWorkTree`, `GetRepoBranch`, `IsRepoClean`, `IsGitRepo` |

---

## Testing

### Unit Tests (workspace_test.go)

Use `t.TempDir()` to create isolated workspace structures. No git repos needed.

**State detection tests:**
- Workspace with both `initialized` and `task.json` -> Active
- Workspace with `initialized` only -> Inactive
- Workspace with neither -> Invalid
- Workspace with `task.json` only (no `initialized`) -> Invalid

**Enumeration tests:**
- `GetWorkspaces` with empty directory -> empty result
- `GetWorkspaces` with mix of `ws-N` dirs and other dirs -> only `ws-N` returned
- `GetWorkspaceRepos` returns only non-dot-prefixed directories
- `GetWorkspaceRepos` excludes files (non-directories)
- `GetWorkspaceRepos` excludes `.creation-lock` and other dot-prefixed entries

**Filtered query tests:**
- Mock the active profile's workspaces root by setting up the config state
- Verify `GetActiveWorkspaces` returns only active, unlocked workspaces
- Verify `GetInactiveWorkspaces` returns only inactive, unlocked workspaces
- Verify `GetInvalidWorkspaces` returns only invalid, unlocked workspaces

**Search tests:**
- `SearchAvailableWorkspace` with matching inactive workspace -> returns it
- `SearchAvailableWorkspace` with no match -> returns empty string
- `SearchAvailableWorkspace` skips locked workspaces
- `SearchAvailableWorkspace` skips active workspaces
- `SearchAvailableWorkspace` with repos in different order still matches (set equality)

**Creation tests:**
- `CreateWorkspaceDirectory` in empty root -> creates `ws-1`
- `CreateWorkspaceDirectory` with `ws-1` existing -> creates `ws-2`
- `CreateWorkspaceDirectory` with gaps (e.g., `ws-1` and `ws-3` exist) -> creates `ws-2`
- `MarkInitialized` creates the marker file

**CLAUDE.md tests:**
- `WriteWorkspaceCLAUDEMD` creates file with expected content
- `EnsureWorkspaceCLAUDEMD` creates file when >1 repo and file absent
- `EnsureWorkspaceCLAUDEMD` does not create file when <=1 repo
- `EnsureWorkspaceCLAUDEMD` does not overwrite existing file

**Locking tests:**
- `LockWorkspace` acquires lock; `IsWorkspaceLocked` returns `true` from another check
- After lock is released (closer called), `IsWorkspaceLocked` returns `false`
- `IsWorkspaceLocked` on non-existent lock file returns `false`

**Session working directory tests:**
- Single repo -> returns repo path
- Multiple repos -> returns workspace path
- Zero repos -> returns workspace path

**ResolveWorkspaceFromCWD tests:**
- CWD inside a workspace -> returns profile and workspace path
- CWD not inside any workspace -> returns nil
- CWD inside workspaces root but not in a specific workspace -> returns nil
- CWD inside an uninitialized workspace -> returns nil

### Integration Tests (worktree_test.go)

These tests create real git repositories in temp directories.

**GetRepoBranch:**
- Repo on a branch -> returns branch name
- Repo in detached HEAD -> returns `"HEAD"`

**IsRepoClean:**
- Clean repo -> `true`
- Repo with uncommitted changes -> `false`

**IsGitRepo:**
- Git repo directory -> `true`
- Non-git directory -> `false`

**MakeAndPrepareWorkTree:**
- Creates worktree at expected path
- Worktree is in detached HEAD state
- Worktree is on the correct commit (origin/main)

**DeleteWorkspace:**
- Removes all worktrees and the workspace directory
- Verify worktree directory no longer exists
- Verify source repo no longer lists the worktree

---

## Dependencies on Config Package

The workspace package calls the following functions from `internal/config`. These must be available:

| Function | Used by |
|----------|---------|
| `config.GetWorkspacesRootPath() (string, error)` | `GetActiveWorkspaces`, `GetInactiveWorkspaces`, `GetInvalidWorkspaces` |
| `config.GetRegisteredRepos() ([]string, error)` | `getRepoPath` |
| `config.GetPrepareCommand() *string` | `MakeAndPrepareWorkTree` |
| `config.GetProfiles() []config.Profile` | `ResolveWorkspaceFromCWD` |
| `config.WorkspacesRootForProfile(p *config.Profile) string` | `ResolveWorkspaceFromCWD` |

## Dependencies on Process Package

| Function | Used by |
|----------|---------|
| `process.GetOutput(command []string, opts ...process.ExecOption) (string, error)` | All git operations |
| `process.GetShellOutput(command string, opts ...process.ExecOption) (string, error)` | `MakeAndPrepareWorkTree` (prepare command) |
| `process.WithCwd(cwd string) process.ExecOption` | All git operations |

---


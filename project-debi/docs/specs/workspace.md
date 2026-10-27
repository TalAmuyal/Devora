# Workspace Package Spec

Package: `internal/workspace`

## Overview

The workspace package detects and inspects workspaces on disk. A workspace is a directory under a profile's `workspaces/` root containing git worktrees for development work.
Workspace creation, activation, and deletion are owned by the Devora app; `debi` only reads workspaces (for profile resolution from the CWD and for the multi-repo `git` commands in `wsgit`).

## Dependencies

- `devora/internal/config` -- profiles and their workspaces roots
- `devora/internal/process` -- git command execution

## Workspace Directory Structure

```
<workspaces-root>/ws-N/
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

## State Query Functions

### IsInitialized

```go
func IsInitialized(workspacePath string) bool
```

Returns `true` if the `initialized` marker file exists at `getInitializationMarkerPath(workspacePath)`. Uses `os.Stat`; returns `false` on any error.

## Workspace Enumeration

### GetWorkspaceRepos

```go
func GetWorkspaceRepos(workspacePath string) ([]string, error)
```

Returns the names of non-dot-prefixed directories within the workspace. These represent the git worktrees (repos) in the workspace.

- Reads directory entries from `workspacePath`.
- Filters to entries that are directories and whose names do NOT start with `"."`.
- Returns the directory names (not full paths).
- Order is not guaranteed.

## Git Operations

### IsRepoClean

```go
func IsRepoClean(repoPath string) (bool, error)
```

Returns `true` if the repo has no uncommitted changes.

- Runs `git status --porcelain` in `repoPath` via `process.GetOutput`.
- Returns `true` if the output is an empty string, `false` otherwise.

## Workspace Detection from CWD

### ResolveWorkspaceFromCWD

```go
func ResolveWorkspaceFromCWD(cwd string) (*config.Profile, string, error)
```

Determines whether the given working directory is inside a known workspace. Used to detect the current workspace context from the CWD (e.g. by profile resolution and the git shortcuts).

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

## Testing

**`workspace_test.go`** (uses `t.TempDir()`; no git repos needed):
- `GetWorkspaceRepos` returns only non-dot-prefixed directories and excludes files.
- `ResolveWorkspaceFromCWD`: CWD inside a workspace returns the profile and workspace path; CWD outside any workspace, at the workspaces root itself, or inside an uninitialized workspace returns nil.

**`worktree_test.go`** (creates real git repositories):
- `IsRepoClean`: clean repo -> `true`; repo with uncommitted changes -> `false`.

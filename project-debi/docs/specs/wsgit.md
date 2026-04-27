# WSGit Package Spec

Package: `internal/workspace/wsgit`

## Purpose

Implement workspace-aware variants of `debi gst` and `debi gcl`. When the user runs either command at the exact root of a Devora workspace (`ws-N/`), the package fans out per-repo git/gh queries in parallel and renders a structured per-repo summary (`gst`) or runs a verify-then-update flow (`gcl`). Anywhere else (including inside any single repo, even one that lives inside a workspace), the CLI dispatcher falls back to the existing per-repo passthrough wrappers in `internal/git`.

## Dependencies

- `devora/internal/workspace` -- `ResolveWorkspaceFromCWD`, `GetWorkspaceRepos`, `IsRepoClean`
- `devora/internal/git` -- `CurrentBranchOrDetached`, `DefaultBranchNameWithFallback`
- `devora/internal/gh` -- `GetPRForBranch`, `ErrGHNotInstalled`, `PRSummary`
- `devora/internal/process` -- `GetOutput`, `GetOutputRaw`, `WithCwd`, `WithContext`, `WithExtraEnv`, `PassthroughError`
- `devora/internal/style` -- shared Catppuccin Mocha palette + pre-built lipgloss styles for colored output
- `charm.land/lipgloss/v2` -- `lipgloss.Style` type for the per-cell padding helper and the inline `style.Green.Bold(true)` composition for merged-PR display

## Types

### RepoStatus

```go
type RepoStatus struct {
    Name         string
    Branch       string // "" means detached HEAD
    Counts       PorcelainCounts
    BehindOrigin int  // -1 means "couldn't compute"; renderer prints "?"
    FetchFailed  bool // true means counts may be stale; renderer suffixes BEHIND with "*"
    PRState      PRState
    PRError      error // non-nil means PR lookup failed (auth, network, etc.)
    Err          error // any non-PR error during gather
}
```

The per-repo result of `RunStatus`. One row of the table is rendered from each value.

### PRState

```go
type PRState string

const (
    PRStateUnknown PRState = ""
    PRStateNone    PRState = "No"
    PRStateOpen    PRState = "open"
    PRStateClosed  PRState = "closed"
    PRStateMerged  PRState = "merged"
)
```

`PRStateUnknown` is the zero value; it also signals that we couldn't classify a gh response.

### RepoVerifyResult

```go
type RepoVerifyResult struct {
    Name     string
    Clean    bool
    Detached bool
    Branch   string // populated when not detached so the failure summary can name it
    Err      error
}
```

The per-repo result of `RunClean` Phase 1 verification.

### PorcelainCounts

```go
type PorcelainCounts struct {
    Staged    int
    Unstaged  int
    Untracked int
}
```

Aggregate counts derived from `git status --porcelain=v1 -z` output.

## Sentinels

### ErrNotAtWorkspaceRoot

```go
var ErrNotAtWorkspaceRoot = errors.New("not at workspace root")
```

Returned by `EnsureAtWorkspaceRoot` when the caller's CWD is not the exact root of a Devora workspace. The CLI dispatcher checks for this with `errors.Is` and falls back to the per-repo passthrough path in `internal/git`.

## Functions

### EnsureAtWorkspaceRoot

```go
func EnsureAtWorkspaceRoot(cwd string) (string, error)
```

Returns the workspace path when `cwd` is the exact root of a Devora workspace; returns `ErrNotAtWorkspaceRoot` otherwise.

Behavior:
1. Resolve `cwd` via `filepath.Abs` then `filepath.EvalSymlinks`. Any failure returns `ErrNotAtWorkspaceRoot`.
2. Call `workspace.ResolveWorkspaceFromCWD(abs)`. An empty result returns `ErrNotAtWorkspaceRoot`.
3. Compare the resolved CWD to the resolved workspace path with strict equality. Any inequality (i.e., the caller is inside a sub-directory of the workspace, such as a repo) returns `ErrNotAtWorkspaceRoot`.

The same normalization (`Abs` + `EvalSymlinks`) is required because `ResolveWorkspaceFromCWD` normalizes its candidate paths the same way; comparing without normalizing would yield false negatives for symlinked paths.

### RunStatus

```go
func RunStatus(w io.Writer, wsPath string) error
```

Implements workspace-mode `debi gst`.

1. Enumerate repos via `workspace.GetWorkspaceRepos(wsPath)`.
2. Spawn one goroutine per repo. Each goroutine populates a `RepoStatus`:
   - Best-effort `git fetch origin` (see Concurrent-fetch hardening below). On failure, mark `FetchFailed` and continue.
   - `git.CurrentBranchOrDetached` to get the branch (or `""` for detached HEAD).
   - `git status --porcelain=v1 -z` via `process.GetOutputRaw`, parsed by `parsePorcelain`.
   - `git.DefaultBranchNameWithFallback` and, on success, `git rev-list --count HEAD..origin/<MAIN>` to compute `BehindOrigin`. `BehindOrigin` is `-1` when `origin/<MAIN>` does not exist or `rev-list` fails.
   - `gh.GetPRForBranch(branch)` to fill `PRState`. Skipped when the branch is detached or when `gh` was reported missing by any goroutine.
3. After `wg.Wait()`, sort the slice alphabetically by `Name`.
4. Render the aligned table, any per-repo fetch warnings, an optional `gh` footnote, and the SUMMARY block.
5. Return `nil`. `gst` is a read-only summary; per-repo errors are surfaced inline rather than as command failure.

### RunClean

```go
func RunClean(w io.Writer, wsPath string) error
```

Implements workspace-mode `debi gcl` as a two-phase verify-then-update flow.

1. Enumerate repos via `workspace.GetWorkspaceRepos(wsPath)` and print a workspace banner.
2. **Phase 1 (verify)**: spawn one goroutine per repo and populate a `RepoVerifyResult` with:
   - `workspace.IsRepoClean(repoPath)` for `Clean`.
   - `git.CurrentBranchOrDetached` for `Branch` / `Detached`.
3. After `wg.Wait()`, sort alphabetically. If any repo is dirty, on a branch, or had a verify error, render the failure table, print an aborted summary, and return `&process.PassthroughError{Code: 1}`. No git mutations have run.
4. **Phase 2 (update)**: spawn one goroutine per repo. Each goroutine runs:
   - `git fetch origin` (with hardening; see below).
   - `git.DefaultBranchNameWithFallback` to resolve `<MAIN>`.
   - `git checkout origin/<MAIN>` (captured, so parallel output doesn't interleave).
5. Sort, render per-repo `✓` / `✗` rows, then the `Done — N of M repos updated` summary. Return `&process.PassthroughError{Code: 1}` if any Phase 2 step failed; otherwise `nil`.

Phase 2 calls `git fetch` and `git checkout` directly (rather than calling `git.Gcl`) so output can be captured per repo. This keeps `internal/git` purely passthrough.

## Output Format

### gst

```
WORKSPACE  ws-1 (3 repos)

  REPO       BRANCH      STAGE  UNSTG  UNTRK  BEHIND  PR
  repo-a     HEAD            .      .      .       0  No
  repo-b     feature/x       2      1      .       3  open
  repo-c     HEAD            .      .      .       ?  —

WARN  repo-c: fetch failed (using cached data; counts may be stale)

gh not installed; PR status unavailable

SUMMARY  1 dirty, 2 detached, 1 fetch-failed, 1 of 3 with branch info
         PRs: 1 open, 0 closed, 0 merged, 2 none
```

Column semantics:

| Column | Meaning |
|--------|---------|
| REPO | Repo directory name |
| BRANCH | Current branch, or `HEAD` when detached |
| STAGE | Staged file count (`.` for zero) |
| UNSTG | Unstaged file count (`.` for zero) |
| UNTRK | Untracked file count (`.` for zero) |
| BEHIND | Commits behind `origin/<MAIN>`. `?` when unknown; numeric value with `*` suffix when the row's fetch failed (count may be stale) |
| PR | `No` / `open` / `closed` / `merged`; `—` when `gh` is missing; `?` when the PR query errored for this repo |

The fetch-failure WARN line and the `gh not installed` footnote are only printed when applicable. The SUMMARY block is always printed.

### gcl (verify failure)

```
WORKSPACE  ws-1 (3 repos)

Phase 1: verify all repos clean and detached…
  ✓  repo-a
  ✗  repo-b  on branch feature/x
  ✗  repo-c  dirty

✗  Aborted — 2 of 3 repos failed verification. No changes made.
   Fix the listed repos and re-run `debi gcl`.
```

### gcl (success)

```
WORKSPACE  ws-1 (3 repos)

Phase 2: fetch + checkout origin/<default> in parallel…
  ✓  repo-a  checked out origin/main
  ✓  repo-b  checked out origin/main
  ✓  repo-c  checked out origin/main

✓  Done — 3 of 3 repos updated.
```

## Parallel Pattern

Both `RunStatus` and `RunClean` use a fixed `sync.WaitGroup` plus a pre-allocated `[]Result` indexed by repo position. Each goroutine writes to its own slot, so no channel or mutex is needed for the result slice. `RunStatus` shares an `atomic.Bool` (`ghMissing`) across goroutines: the first goroutine that observes `gh.ErrGHNotInstalled` flips it, and subsequent goroutines short-circuit the gh call. After `wg.Wait()`, results are sorted alphabetically before rendering.

## Counting Semantics for Porcelain

`parsePorcelain` consumes `git status --porcelain=v1 -z` (NUL-separated records):

- `??` → counted as `Untracked`.
- Conflict pair (`U` on either side, or `AA`, or `DD`) → counted once as `Unstaged`.
- Otherwise: when the index status `X` is not `' '` and not `'?'`, count `Staged`; when the worktree status `Y` is not `' '` and not `'?'`, count `Unstaged`. A file modified in both index and worktree (e.g. `MM`) therefore counts in both.
- Renames and copies (`R*` / `C*`) emit a second NUL-separated path entry (the original name); the trailing path is consumed but not counted again.
- The trailing empty record after the final NUL is dropped, and any partial trailing record without a terminating NUL is ignored.

## Concurrent-fetch Hardening

Per-repo `git fetch origin` calls (in both `gst` and `gcl` Phase 2) are hardened to keep parallel fan-out reliable:

- `process.WithExtraEnv("GIT_TERMINAL_PROMPT=0", "GIT_SSH_COMMAND=ssh -o BatchMode=yes")` — never prompt the user from a goroutine.
- `process.WithContext(ctx)` with a 30s timeout (`fetchTimeout`) per repo — a hung remote can't stall the whole summary.

`internal/process` exposes `WithExtraEnv`, `WithContext`, and `GetOutputRaw` (used for the raw NUL-separated porcelain) to support these calls.

## Edge Cases

| Condition | Behavior |
|-----------|----------|
| `gh` not installed | First goroutine that hits `gh.ErrGHNotInstalled` sets the shared `ghMissing` flag; all rows render `—` for PR; a single `gh not installed; PR status unavailable` footnote is printed |
| Detached HEAD | `gh pr view` is skipped entirely; PR cell shows `No`; BRANCH cell shows `HEAD` in muted style |
| Fetch failure for a repo | `FetchFailed=true`; BEHIND value is suffixed with `*`; a per-repo WARN line is printed |
| `origin/<MAIN>` missing | `BehindOrigin` stays at `-1`; BEHIND cell shows `?` |
| PR lookup error (non-`ErrGHNotInstalled`) | `PRError` populated; PR cell shows `?` |
| Workspace with zero repos | Header and SUMMARY still render; the table body is empty |
| `gcl` Phase 1 fails for any repo | Phase 2 does not run; returns `&process.PassthroughError{Code: 1}` |
| `gcl` Phase 2 fails for any repo | Other repos still attempt; returns `&process.PassthroughError{Code: 1}` after rendering |

## File Organization

| File | Contents |
|------|----------|
| `wsgit.go` | Package doc, `RepoStatus`, `RepoVerifyResult`, `PRState`, `ErrNotAtWorkspaceRoot`, `EnsureAtWorkspaceRoot`, the `getPRForBranch` test seam |
| `porcelain.go` | `PorcelainCounts`, `parsePorcelain`, `isConflictPair` |
| `status.go` | `RunStatus`, `gatherRepoStatus`, `bestEffortFetch`, behind helper, table rendering |
| `clean.go` | `RunClean`, Phase 1 verify, Phase 2 fetch+checkout, rendering |

## Testing

`wsgit_test.go` and `porcelain_test.go` exercise the package against real temp git repos. The single test seam declared in `wsgit.go`:

```go
var getPRForBranch = gh.GetPRForBranch
```

lets tests stub out `gh` without invoking the real binary.

Critical cases covered:

- `EnsureAtWorkspaceRoot`: workspace root → returns path; sub-directory inside the workspace → `ErrNotAtWorkspaceRoot`; arbitrary path → `ErrNotAtWorkspaceRoot`; symlinked workspace root → resolves correctly.
- `parsePorcelain`: untracked, staged-only, unstaged-only, both-modified (`MM`), conflict pairs (`UU`/`AA`/`DD`), rename with second path entry, partial trailing record without NUL, empty input.
- `RunStatus`: alphabetical sort regardless of input order; first `gh.ErrGHNotInstalled` flips the shared flag and prevents subsequent gh calls; fetch failure sets `FetchFailed` and the row's BEHIND gets the `*` suffix; detached HEAD skips gh and yields PR `No`.
- `RunClean`: Phase 1 detects dirty / on-branch / verify-error and aborts with `*PassthroughError{Code:1}` before any fetch; Phase 2 success path returns `nil`; Phase 2 failure path returns `*PassthroughError{Code:1}` while still attempting every repo.

# Git Package Spec

Package: `internal/git`

## Purpose

Provides git shortcut command implementations and shared git utilities. All commands run in passthrough mode (stdin/stdout/stderr connected to the terminal).

`Gst` and `Gcl` here remain pure per-repo passthrough wrappers; the workspace-aware variants (parallel multi-repo summary for `debi gst`, two-phase verify-then-update for `debi gcl`) live in `internal/workspace/wsgit` and are dispatched by the CLI based on the caller's CWD. See [wsgit.md](./wsgit.md).

## Dependencies

- `devora/internal/process` -- command execution (`RunPassthrough`, `GetOutput`)

## Minimum git version

Some helpers rely on git 2.18+ (released 2018-06-21). Specifically, `GetBranchConfig` uses `git config --get --default "" ...`, which exits 0 with empty output when the key is missing; earlier git versions exit non-zero.

## Utility Functions

### DefaultBranchName

```go
func DefaultBranchName(opts ...process.ExecOption) (string, error)
```

Returns the default branch name for the repository by querying `git symbolic-ref refs/remotes/origin/HEAD` and stripping the `refs/remotes/origin/` prefix.

Also used by `internal/workspace` to avoid duplicating the logic.

### DefaultBranchNameWithFallback

```go
func DefaultBranchNameWithFallback(opts ...process.ExecOption) (string, error)
```

Resolves origin's default branch via `DefaultBranchName`; on failure, falls back to `git rev-parse --verify main` and then `git rev-parse --verify master`, returning the first that succeeds. Returns an error when `origin/HEAD` is unset and neither `main` nor `master` exists.

Used by `internal/close` so that close can proceed even when `origin/HEAD` is not configured.

### EnsureInRepo

```go
var ErrNotInGitRepo = errors.New("not in a git repository")

const NotInRepoMessage = "this command must be run from inside a git repository"

func EnsureInRepo(opts ...process.ExecOption) error
```

Precondition probe for commands that require a git repository. Runs `git rev-parse --git-dir` and returns `ErrNotInGitRepo` when the command fails; returns `nil` when the current working directory (or the directory provided via `process.WithCwd`) is inside a git repository.

`NotInRepoMessage` is the user-facing string the CLI layer prints when it translates `ErrNotInGitRepo` into a `*cli.UsageError`.

Used by `runSubmit`, `runClose`, and `runPRCheck` in `internal/cli` to short-circuit with a friendly error instead of letting a downstream git invocation crash with a native "not a git repository" stderr.

### Submit / close helpers (`submit_close.go`)

Separate file from `commands.go` because these helpers return captured output or structured results rather than pure passthrough. Used by `internal/submit` and `internal/close`.

| Function | Purpose |
|----------|---------|
| `CurrentBranchOrDetached(opts ...process.ExecOption) (string, error)` | Returns the current branch name, or `""` when HEAD is detached. Uses `git symbolic-ref --short HEAD`; the "ref HEAD is not a symbolic ref" stderr is treated as `("", nil)`. |
| `IsProtectedBranch(branch string) bool` | Reports whether the branch is in `ProtectedBranches`. |
| `ProtectedBranches []string` | Fixed list: `main`, `master`, `develop`. Exported so tests can reference it directly. |
| `AddAllAndCommit(message string, opts ...process.ExecOption) error` | Passthrough `git add .` then `git commit -m <message>`. Opts forwarded to both calls. |
| `CreateAndCheckoutBranch(name string, opts ...process.ExecOption) error` | Passthrough `git checkout -b <name>`. |
| `PushSetUpstream(branch string, opts ...process.ExecOption) error` | Passthrough `git push --set-upstream origin <branch>`. |
| `DeleteRemoteBranch(branch string, opts ...process.ExecOption) error` | Passthrough `git push origin --delete <branch>`. |
| `DeleteLocalBranch(branch string, opts ...process.ExecOption) error` | Passthrough `git branch -D <branch>`. |
| `HasLocalBranch(branch string, opts ...process.ExecOption) bool` | True when `git rev-parse --verify <branch>` succeeds. |
| `HasRemoteBranch(branch string, opts ...process.ExecOption) bool` | True when `git ls-remote --exit-code --heads origin <branch>` succeeds. |
| `SetBranchConfig(branch, key, value string, opts ...process.ExecOption) error` | Writes `branch.<branch>.<key>=<value>` via `git config --local`. |
| `GetBranchConfig(branch, key string, opts ...process.ExecOption) (string, error)` | Reads `branch.<branch>.<key>` via `git config --get --default "" ...`. Returns `""` (not an error) when the key is missing. Requires git 2.18+. |
| `GetRepoConfigBool(key string, opts ...process.ExecOption) (*bool, error)` | Reads a local git config boolean via `git config --type=bool --get <key>`. Returns `(nil, nil)` when the key is absent or when the stored value cannot be parsed as a bool (so callers can treat wrong-type entries as "unset"). Because git stores local config in `$GIT_COMMON_DIR/config`, values are shared across all linked worktrees automatically. |
| `SetRepoConfigBool(key string, value bool, opts ...process.ExecOption) error` | Writes a local git config boolean via `git config --type=bool --local <key> <value>` (`"true"` or `"false"`). |
| `UnsetRepoConfig(key string, opts ...process.ExecOption) error` | Clears a local git config key via `git config --local --unset <key>`. Git exits 5 when the key was already unset; this function treats that exit as `nil` so callers can invoke it idempotently. |
| `FetchOrigin(opts ...process.ExecOption) error` | Passthrough `git fetch origin`. |
| `CheckoutDetach(ref string, opts ...process.ExecOption) error` | Passthrough `git checkout <ref>`. Callers typically pass `origin/<default>` to land in detached HEAD. |
| `GenerateBranchName(prefix, taskName string) string` | Produces `<prefix>-<slug>` where `<slug>` is a dash-separated, lowercase, ASCII-alphanumeric distillation of `taskName`. Non-alphanumerics collapse to a single `-`; leading/trailing dashes trimmed; capped at 70 characters with trailing `-` trimmed after truncation. When the distilled slug is empty (e.g., all non-ASCII input), falls back to the constant `"unnamed"`. |

All passthrough helpers accept variadic `process.ExecOption`; callers pass `process.WithSilent()` to route stdout/stderr to `io.Discard` (used by submit/close in Normal and Quiet modes).

## Command Functions

All command functions run git commands in passthrough mode via an unexported `passthrough` helper. They return `*process.PassthroughError` on non-zero exit.

### Simple Passthrough Commands

| Function | Git Command |
|----------|-------------|
| `Gaa()` | `git add .` |
| `Gb(args)` | `git branch [args]` |
| `Gbd(args)` | `git branch -D [args]` |
| `Gd(args)` | `git diff [args]` |
| `Gfo(args)` | `git fetch origin [args]` |
| `Gg(args)` | `git grep [args]` |
| `Gl(args)` | `git log [args]` |
| `Gpo(args)` | `git push origin [args]` |
| `Gpof(args)` | `git push origin --force [args]` |
| `Gpop(args)` | `git stash pop [args]` |
| `Gst(args)` | `git status [args]` |
| `Gstash(args)` | `git stash [args]` |

### Compound Commands

#### Gaac

```go
func Gaac(args []string) error
```

Joins `args` into a commit message, then runs `git add .` followed by `git commit -m <message>`.

#### Gaacp

```go
func Gaacp(args []string) error
```

Runs `Gaac(args)` then `git push origin`.

#### Gaaa

```go
func Gaaa() error
```

Runs `git add .` then `git commit --amend --no-edit`.

#### Gaaap

```go
func Gaaap() error
```

Runs `Gaaa()` then `Gpof(nil)`.

#### Gbdc

```go
func Gbdc() error
```

Deletes the current branch by:
1. Getting current branch name via `git symbolic-ref HEAD`
2. Running `git checkout --detach`
3. Running `git branch -D <branch-name>`

#### Gcl

```go
func Gcl() error
```

Runs `Gfo(nil)` then `Gcom(nil)`.

#### Gcom

```go
func Gcom(args []string) error
```

Checks out the default branch from origin: `git checkout origin/<default-branch> [args]`.

#### Gri

```go
func Gri(args []string) error
```

Interactive rebase.

- With `-h` or `--help`: prints usage and returns nil.
- With no args: determines commit count since divergence from `origin/<default-branch>` using `git merge-base` and `git rev-list --count`, then runs `git rebase -i HEAD~<N>`. If N is 0, prints "Nothing to rebase." and returns nil.
- With a numeric arg N (>= 1): runs `git rebase -i HEAD~<N>`.
- With an invalid arg: prints error to stderr and returns `PassthroughError{Code: 1}`.

#### Grl

```go
func Grl() error
```

Runs `Gfo(nil)` then `Grom()`.

#### Grlp

```go
func Grlp() error
```

Runs `Grl()` then `Gpof(nil)`.

#### Grom

```go
func Grom() error
```

Runs `git rebase origin/<default-branch>`.

## File Organization

| File | Contents |
|------|----------|
| `default_branch.go` | `DefaultBranchName`, `DefaultBranchNameWithFallback` |
| `commands.go` | All 23 command functions and the `passthrough` helper |
| `submit_close.go` | Submit/close helpers (`CurrentBranchOrDetached`, `IsProtectedBranch`, `GenerateBranchName`, branch-config helpers, etc.) |
| `in_repo.go` | `EnsureInRepo`, `ErrNotInGitRepo`, `NotInRepoMessage` |

## Testing

### default_branch_test.go

Integration tests using a real temp git repo:
- `DefaultBranchName`: repo with `refs/remotes/origin/HEAD` pointing to `main` returns `"main"`.
- `DefaultBranchNameWithFallback`: falls back to `main` when `origin/HEAD` is unset; falls back to `master` when only `master` exists; errors when neither `main` nor `master` exists and `origin/HEAD` is unset.

### commands_test.go

- Test `Gri` with invalid argument returns `*PassthroughError` with code 1.
- Test `Gri` with zero argument returns `*PassthroughError`.
- Test `Gri` with negative argument returns `*PassthroughError`.
- Test `Gri` with `--help` flag prints usage and returns nil.
- Test `Gri` with `-h` flag prints usage and returns nil.

### submit_close_test.go

Table-driven unit tests for `GenerateBranchName` (ASCII, mixed case, trailing dashes, 70-character cap, empty input, all-non-ASCII input falls back to `"unnamed"`), plus temp-repo integration tests for `CurrentBranchOrDetached`, `IsProtectedBranch`, `SetBranchConfig`/`GetBranchConfig` (including the `--default ""` missing-key behavior), `HasLocalBranch`, and `HasRemoteBranch`.

Repo-config helper tests:
- `GetRepoConfigBool` returns `(nil, nil)` when the key is unset.
- Round-trip: `SetRepoConfigBool(key, true/false)` → `GetRepoConfigBool` returns the matching `*bool`.
- A wrong-type stored value (e.g., `maybe`) falls through to `(nil, nil)` without error.
- `UnsetRepoConfig` clears a previously set value; subsequent `GetRepoConfigBool` returns `(nil, nil)`.
- `UnsetRepoConfig` is idempotent: calling it when no value is set returns `nil`.
- Values written from the main worktree are visible from a linked worktree added via `git worktree add`.
- Round-trip works against a bare clone (`git clone --bare`).

### in_repo_test.go

Integration tests for `EnsureInRepo`:
- Returns `ErrNotInGitRepo` when invoked with a `process.WithCwd` pointing at a temp directory that is not inside a git repository.
- Returns `nil` when invoked with a `process.WithCwd` pointing at a freshly `git init`'d temp directory.

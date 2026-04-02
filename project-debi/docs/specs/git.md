# Git Package Spec

Package: `internal/git`

## Purpose

Provides git shortcut command implementations and shared git utilities. All commands run in passthrough mode (stdin/stdout/stderr connected to the terminal).

## Dependencies

- `devora/internal/process` -- command execution (`RunPassthrough`, `GetOutput`)

## Utility Functions

### DefaultBranchName

```go
func DefaultBranchName(opts ...process.ExecOption) (string, error)
```

Returns the default branch name for the repository by querying `git symbolic-ref refs/remotes/origin/HEAD` and stripping the `refs/remotes/origin/` prefix.

Also used by `internal/workspace` to avoid duplicating the logic.

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
| `default_branch.go` | `DefaultBranchName` utility |
| `commands.go` | All 23 command functions and the `passthrough` helper |

## Testing

### default_branch_test.go

Integration test using a real temp git repo:
- Set up a git repo with a commit, remote, and `refs/remotes/origin/HEAD` pointing to `main`.
- Verify `DefaultBranchName` returns `"main"`.

### commands_test.go

- Test `Gri` with invalid argument returns `*PassthroughError` with code 1.
- Test `Gri` with zero argument returns `*PassthroughError`.
- Test `Gri` with negative argument returns `*PassthroughError`.
- Test `Gri` with `--help` flag prints usage and returns nil.
- Test `Gri` with `-h` flag prints usage and returns nil.

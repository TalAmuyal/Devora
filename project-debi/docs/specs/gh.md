# GH Package Spec

Package: `internal/gh`

## Purpose

Wrap the GitHub CLI (`gh`) subprocess for the submit and close commands. Centralizes JSON parsing, stderr-driven error classification ("gh not installed", "PR already exists", "no PR for branch"), and the stubbable package-level function vars that tests replace.

Note: `internal/prstatus` predates this package and still calls `gh` directly through `internal/process`. The two do not conflict.

## Stability

Stable public API: `RepoInfo`, `PRSummary`, `PRAlreadyExistsError`, `ErrGHNotInstalled`, `GetRepo`, `GetPRForBranch`, `CreatePR`, `EnableAutoMergeSquash`.

## Dependencies

- `devora/internal/process` -- subprocess execution; `VerboseExecError` for stderr inspection

## Types

### RepoInfo

```go
type RepoInfo struct {
    Owner         string
    Name          string
    DefaultBranch string
}
```

Flattened result of `gh repo view --json defaultBranchRef,name,owner`.

### PRSummary

```go
type PRSummary struct {
    Number  int
    URL     string
    State   string // "OPEN" | "CLOSED" | "MERGED"
    IsDraft bool
    Merged  bool
    Title   string
}
```

Parsed from `gh pr view ... --json number,url,state,isDraft,title`. `Merged` is derived from `State == "MERGED"` — `gh pr view` does not expose a direct `merged` JSON field, and requesting it produced an `Unknown JSON field: "merged"` error in production.

### PRAlreadyExistsError

```go
type PRAlreadyExistsError struct {
    Branch      string
    ExistingURL string
}
```

Returned by `CreatePR` when `gh` reports that a PR for the current branch already exists. `Branch` and `ExistingURL` are extracted from gh's stderr via a regex; either may be empty when gh's message shape changes.

`Error()` returns `a pull request for branch "<branch>" already exists` and includes `: <url>` when `ExistingURL` is set.

## Sentinels

### ErrGHNotInstalled

```go
var ErrGHNotInstalled = errors.New("gh CLI is not installed; see https://cli.github.com")
```

Returned (wrapped) when the `gh` binary isn't on `$PATH`. Detection uses `errors.Is(err, exec.ErrNotFound)`, which drills through `*process.VerboseExecError.Unwrap()` automatically. Callers detect with `errors.Is(err, gh.ErrGHNotInstalled)`.

## Stubbable Dependencies

```go
var (
    runGHGetOutput   = defaultRunGHGetOutput   // process.GetOutput("gh", args...)
    runGHPassthrough = defaultRunGHPassthrough // process.RunPassthrough("gh", args..., opts...)
)
```

`defaultRunGHGetOutput` captures stdout; `defaultRunGHPassthrough` streams stdin/stdout/stderr to the terminal and accepts variadic `process.ExecOption` (forwarded to `process.RunPassthrough`) so callers can pass `process.WithSilent()` to suppress output. Tests replace both to simulate any gh response or error condition without invoking the real CLI.

## Functions

### GetRepo

```go
func GetRepo() (RepoInfo, error)
```

Runs `gh repo view --json defaultBranchRef,name,owner`. Returns the flattened `RepoInfo`.

- Invocation error: classified via `classifyGHError`. On `exec.ErrNotFound`, returns a wrapped `ErrGHNotInstalled`. Otherwise returns the underlying error unchanged.
- JSON parse error: returns `fmt.Errorf("parse gh repo view: %w", err)`.

### GetPRForBranch

```go
func GetPRForBranch(branch string) (*PRSummary, error)
```

Runs `gh pr view <branch> --json number,url,state,isDraft,title`. `PRSummary.Merged` is derived from `state == "MERGED"`.

- Returns `(nil, nil)` when gh's stderr contains `"no pull requests found"` -- the idiomatic "no PR" signal, matching the pattern used in `internal/prstatus`.
- Invocation error (other than "no PR"): wrapped as `fmt.Errorf("gh pr view %s: %w", branch, classifyGHError(err))`.
- JSON parse error: returns `fmt.Errorf("parse gh pr view: %w", err)`.

### CreatePR

```go
func CreatePR(title, body string, draft bool) (string, error)
```

Runs `gh pr create --title <title> --body <body> --assignee @me` plus `--draft` when `draft` is true. Self-assignment via `--assignee @me` is always included so a separate round-trip is never needed.

Returns the trimmed stdout (the PR URL) on success.

- Stderr matches the "already exists" pattern: returns `*PRAlreadyExistsError` populated from the regex captures. The check first verifies both `"a pull request for branch"` and `"already exists"` are present as a cheap guard, then runs the regex:

  ```
  a pull request for branch "<branch>".*?already exists(?::\s*(<url>))?
  ```

  When the substrings match but the regex does not, `PRAlreadyExistsError{}` is returned empty rather than losing the signal.
- Other invocation error: wrapped as `fmt.Errorf("gh pr create: %w", classifyGHError(err))`.

### EnableAutoMergeSquash

```go
func EnableAutoMergeSquash(prURL string, opts ...process.ExecOption) error
```

Runs `gh pr merge <prURL> --auto --squash` in passthrough mode. Variadic `process.ExecOption` is threaded into the underlying `runGHPassthrough`; callers pass `process.WithSilent()` to suppress stdout/stderr (used by submit in Normal and Quiet modes). Errors are wrapped as `fmt.Errorf("gh pr merge --auto --squash: %w", classifyGHError(err))`. Submit calls this best-effort: failures print a warning and do not fail the command.

## Error Classification

`classifyGHError(err)`:

- `nil` -> `nil`.
- `errors.Is(err, exec.ErrNotFound)` -> `fmt.Errorf("%w: %w", ErrGHNotInstalled, err)`.
- Otherwise -> `err` unchanged.

The double-`%w` form lets callers detect `ErrGHNotInstalled` via `errors.Is` while preserving the original `exec.ErrNotFound` chain for deeper inspection.

## Test Coverage

`internal/gh/gh_test.go` covers:

- `GetRepo` happy path parses the payload.
- `GetRepo` malformed JSON returns a parse error.
- `GetRepo` "gh not installed" via `exec.ErrNotFound` returns wrapped `ErrGHNotInstalled`.
- `GetPRForBranch` stderr "no pull requests found" returns `(nil, nil)`.
- `GetPRForBranch` happy path parses the payload.
- `GetPRForBranch` other errors are wrapped.
- `CreatePR` happy path returns trimmed stdout.
- `CreatePR` "already exists" with URL populates `*PRAlreadyExistsError` correctly.
- `CreatePR` "already exists" without URL still populates branch name.
- `CreatePR` "gh not installed" returns wrapped `ErrGHNotInstalled`.
- `EnableAutoMergeSquash` runs the expected args in passthrough mode.
- `EnableAutoMergeSquash` wraps errors with context.

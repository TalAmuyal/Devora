# PR Status Package Spec

Package: `internal/prstatus`

Exposed as the `debi pr check` CLI command. The package name kept its historical `prstatus` form because it's an internal implementation detail; the public-facing CLI verb is `check`.

## Purpose

Check and report on the GitHub PR status for the current branch. The current branch is determined via `git symbolic-ref`, and the PR data is fetched by invoking the `gh` CLI (`gh pr view --json ...`). Results are printed either as human-readable colored output or as structured JSON.

## Dependencies

- `devora/internal/process` -- `VerboseExecError` for detecting "no PR" vs other errors
- `devora/internal/style` -- shared Catppuccin Mocha palette + pre-built lipgloss styles for colored output
- `charm.land/lipgloss/v2` -- `lipgloss.Style` type for the local summary-style variable
- `encoding/json` -- parsing `gh` JSON output and producing JSON output
- `errors` -- error wrapping and unwrapping
- `fmt` -- formatted output
- `io` -- `io.Writer` for output destination
- `strings` -- string manipulation

## Types

### PRData

```go
type PRData struct {
    Number            int              `json:"number"`
    Title             string           `json:"title"`
    URL               string           `json:"url"`
    State             string           `json:"state"`
    IsDraft           bool             `json:"isDraft"`
    HeadRefName       string           `json:"headRefName"`
    ReviewDecision    string           `json:"reviewDecision"`
    AutoMergeRequest  *json.RawMessage `json:"autoMergeRequest"`
    StatusCheckRollup []CheckRun       `json:"statusCheckRollup"`
    Reviews           []Review         `json:"reviews"`
}
```

The parsed JSON response from `gh pr view --json`. `AutoMergeRequest` is a raw JSON pointer -- a non-nil value means auto-merge is enabled.

### CheckRun

```go
type CheckRun struct {
    Name       string `json:"name"`
    Context    string `json:"context"`
    Status     string `json:"status"`
    Conclusion string `json:"conclusion"`
    TypeName   string `json:"__typename"`
}
```

A single CI check or status context from the PR's `statusCheckRollup`. `Name` is used for `CheckRun` type entries and `Context` for `StatusContext` type entries.

### Review

```go
type Review struct {
    Author ReviewAuthor `json:"author"`
    State  string       `json:"state"`
}
```

A single review on the PR.

### ReviewAuthor

```go
type ReviewAuthor struct {
    Login string `json:"login"`
}
```

The author of a review.

### ChecksSummary

```go
type ChecksSummary struct {
    Total         int
    Passed        int
    Failed        int
    Pending       int
    Skipped       int
    FailedChecks  []string
    PendingChecks []string
}
```

Aggregated summary of all CI checks. `FailedChecks` and `PendingChecks` contain the display names of the respective checks.

### ReviewsSummary

```go
type ReviewsSummary struct {
    Approved         int
    ChangesRequested int
    Commented        int
    Pending          int
}
```

Aggregated summary of all reviews, deduplicated by author.

### JSONOutput

```go
type JSONOutput struct {
    Branch  string             `json:"branch"`
    PR      *JSONOutputPR      `json:"pr,omitempty"`
    Checks  *JSONOutputChecks  `json:"checks,omitempty"`
    Reviews *JSONOutputReviews `json:"reviews,omitempty"`
}
```

Top-level JSON output structure. When no PR is found, `PR`, `Checks`, and `Reviews` are omitted.

### JSONOutputPR

```go
type JSONOutputPR struct {
    Number    int    `json:"number"`
    Title     string `json:"title"`
    URL       string `json:"url"`
    State     string `json:"state"`
    Draft     bool   `json:"draft"`
    AutoMerge bool   `json:"auto_merge"`
}
```

### JSONOutputChecks

```go
type JSONOutputChecks struct {
    Total         int      `json:"total"`
    Passed        int      `json:"passed"`
    Failed        int      `json:"failed"`
    Pending       int      `json:"pending"`
    Skipped       int      `json:"skipped"`
    FailedChecks  []string `json:"failed_checks"`
    PendingChecks []string `json:"pending_checks"`
}
```

### JSONOutputReviews

```go
type JSONOutputReviews struct {
    Approved         int `json:"approved"`
    ChangesRequested int `json:"changes_requested"`
    Commented        int `json:"commented"`
    Pending          int `json:"pending"`
}
```

## Stubbable Dependencies

The package exposes two package-level variables for test injection:

```go
var getGHPRView = defaultGetGHPRView
var getCurrentBranch = defaultGetCurrentBranch
```

`getGHPRView` calls `gh pr view --json ...` via `process.GetOutput`. `getCurrentBranch` calls `git symbolic-ref --short HEAD` via `process.GetOutput`. Tests can replace both to simulate any combination of branch state and PR data without calling real CLIs.

## Functions

### Run

```go
func Run(w io.Writer, jsonOutput bool) error
```

Main entry point for the PR status check.

Behavior:
1. Call `getCurrentBranch()` to get the current branch name. If it fails (detached HEAD), return an error: `"cannot check PR status: not on a branch"`.
2. Call `getGHPRView()` to fetch PR data as JSON. If the error is a `*process.VerboseExecError` whose stderr indicates no PR was found, print a warning message and return nil. If the error indicates `gh` is not installed, return an error: `"gh CLI is required for PR status. Install it: https://cli.github.com"`. For other errors, return them with context.
3. Parse the JSON response into a `PRData` struct. If parsing fails, return a wrapped parse error.
4. Compute `ChecksSummary` via `computeChecksSummary` and `ReviewsSummary` via `computeReviewsSummary`.
5. If `jsonOutput` is true, construct a `JSONOutput` struct and write indented JSON to `w`.
6. If `jsonOutput` is false, write human-readable colored output to `w`.

### computeChecksSummary

```go
func computeChecksSummary(checks []CheckRun) ChecksSummary
```

Classifies each check run and produces an aggregate summary.

Behavior:
1. For each check, determine its display name: use `Name` if non-empty, otherwise use `Context`.
2. Classify the check:
   - If `Status` is `IN_PROGRESS`, `QUEUED`, or `PENDING`, or if `Conclusion` is empty: count as Pending, add to `PendingChecks`.
   - If `Conclusion` is `SUCCESS`: count as Passed.
   - If `Conclusion` is `FAILURE`: count as Failed, add to `FailedChecks`.
   - If `Conclusion` is `SKIPPED` or `NEUTRAL`: count as Skipped.
3. Set `Total` to the length of the input slice.

### computeReviewsSummary

```go
func computeReviewsSummary(reviews []Review) ReviewsSummary
```

Deduplicates reviews by author (last review wins) and counts by state.

Behavior:
1. Iterate over reviews in order, building a map of author login to review state. Later entries overwrite earlier ones (last review wins).
2. Count the deduplicated reviews by state: `APPROVED` increments `Approved`, `CHANGES_REQUESTED` increments `ChangesRequested`, `COMMENTED` increments `Commented`, all other states increment `Pending`.

## Human Output Format

```
PR Status

Branch: feature/my-branch
PR #42: Add new feature
  State: open
  Checks: 5/6 passing (1 failed)
    ✗ build-and-test
  Reviews: 1 approved
  URL: https://github.com/org/repo/pull/42
```

State colors: `merged` = green, `closed` = red, `draft` = yellow, `open` = cyan. Auto-merge is appended to the state line when enabled on an open, non-merged PR (e.g., `open (auto-merge enabled)`).

Checks summary line: all passing = green, any failures = red, any pending (no failures) = yellow. Failed checks are listed with a red `✗` prefix, pending checks with a yellow `○` prefix, up to 5 each. If more than 5, a `...and N more` overflow line is shown.

Reviews summary line: `approved` count in green, `changes requested` count in red, `pending` count in yellow. Parts are comma-separated.

The `Checks` line is omitted when there are no checks. The `Reviews` line is omitted when there are no reviews.

When no PR is found:
```
⚠ No PR found for branch feature/my-branch
```

## JSON Output Format

```json
{
  "branch": "feature/my-branch",
  "pr": {
    "number": 42,
    "title": "Add new feature",
    "url": "https://github.com/org/repo/pull/42",
    "state": "open",
    "draft": false,
    "auto_merge": false
  },
  "checks": {
    "total": 6,
    "passed": 5,
    "failed": 1,
    "pending": 0,
    "skipped": 0,
    "failed_checks": ["build-and-test"],
    "pending_checks": []
  },
  "reviews": {
    "approved": 1,
    "changes_requested": 0,
    "commented": 0,
    "pending": 0
  }
}
```

When no PR is found, only the branch is included:
```json
{
  "branch": "feature/my-branch"
}
```

The `pr`, `checks`, and `reviews` fields are omitted (not null) when there is no PR.

## Error Handling

| Condition | Behavior |
|-----------|----------|
| Detached HEAD | Return error: `"cannot check PR status: not on a branch"` |
| `gh` not installed | Return error: `"gh CLI is required for PR status. Install it: https://cli.github.com"` |
| `gh` auth failure | Propagated with context |
| No PR found | Print warning message, return nil |
| Malformed JSON from `gh` | Return wrapped parse error |

## Testability

Tests replace `getGHPRView` and `getCurrentBranch` via a `stubPRStatusDeps(t)` helper that saves and restores the original values using `t.Cleanup`.

## Testing

- Test `Run` with an open PR (human output); verify output contains branch, PR number, title, state, checks summary, reviews summary, and URL.
- Test `Run` with an open PR (JSON output); verify well-formed JSON with all fields populated.
- Test `Run` with a merged PR; verify state is displayed as "merged" (human) and `"state": "merged"` (JSON).
- Test `Run` with a closed PR; verify state is displayed as "closed".
- Test `Run` with a draft PR; verify state is displayed as "draft".
- Test `Run` with auto-merge enabled; verify auto-merge indication in human output and `"auto_merge": true` in JSON output.
- Test `Run` when no PR is found (human output); verify warning message and nil return.
- Test `Run` when no PR is found (JSON output); verify JSON contains only `branch` field.
- Test `Run` with detached HEAD; verify error message `"cannot check PR status: not on a branch"`.
- Test `Run` when `gh` is not installed; verify error message with install URL.
- Test `Run` with all checks passing; verify green checks summary.
- Test `Run` with failed checks; verify failed checks are listed.
- Test `Run` with pending checks; verify pending checks are listed.
- Test `Run` with no checks; verify `Checks` line is omitted.
- Test `Run` with reviews showing approved; verify reviews summary.
- Test `Run` with reviews showing changes requested; verify reviews summary.
- Test `Run` with no reviews; verify `Reviews` line is omitted.
- Test `Run` with multiple reviews from the same author; verify deduplication (last review wins).
- Test `computeChecksSummary` with a mix of passed, failed, pending, and skipped checks; verify all counts and name lists.
- Test `computeChecksSummary` with an empty slice; verify all counts are zero.
- Test `computeChecksSummary` uses `Name` when non-empty and `Context` as fallback.
- Test `computeReviewsSummary` with multiple authors and states; verify counts.
- Test `computeReviewsSummary` with duplicate authors; verify last review wins.
- Test `computeReviewsSummary` with an empty slice; verify all counts are zero.

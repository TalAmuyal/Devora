# Health Package Spec

Package: `internal/health`

## Purpose

Check and report on Devora IDE dependency availability. Each dependency is looked up on `PATH` and, if found, its version is retrieved. Results are printed in a human-readable report grouped by required and optional dependencies.

## Dependencies

- `devora/internal/process` -- `PassthroughError` for exit-code signalling
- `charm.land/lipgloss/v2` -- colored output

## Types

### Dependency

```go
type Dependency struct {
    Name           string
    Required       bool
    VersionCommand []string
}
```

Describes a single external tool that Devora depends on.

### CheckResult

```go
type CheckResult struct {
    Dependency
    Found   bool
    Path    string
    Version string
}
```

The result of checking a single dependency. Embeds the original `Dependency`. `Found` indicates whether the binary exists on `PATH`. `Path` is the resolved binary path. `Version` is the cleaned version string extracted from the first line of version command output (see `cleanVersion`).

## Dependency List

Six required and three optional dependencies:

| Name | Required | Version Command |
|------|----------|-----------------|
| `kitty` | yes | `kitty --version` |
| `claude` | yes | `claude --version` |
| `git` | yes | `git --version` |
| `uv` | yes | `uv --version` |
| `glow` | yes | `glow --version` |
| `zsh` | yes | `zsh --version` |
| `nvim` | no | `nvim --version` |
| `mise` | no | `mise --version` |
| `jq` | no | `jq --version` |

## Functions

### Check

```go
func Check(dep Dependency) CheckResult
```

Checks a single dependency.

Behavior:
1. Look up the binary via `lookPath(dep.Name)`.
2. If not found, return a `CheckResult` with `Found: false`.
3. If found, set `Found: true` and `Path` to the resolved path.
4. Run the version command via `getVersion(dep.VersionCommand)`.
5. If the version command fails, return the result without a `Version` (the dependency is still considered found).
6. Otherwise, take the first line of output, trim it, and pass it through `cleanVersion` to extract the version number.

### Run

```go
func Run(w io.Writer, strict bool, verbose bool) error
```

Runs the full health check and prints a report.

Behavior:
1. Iterate over all dependencies, calling `Check` for each. Separate results into required and optional lists.
2. Calculate column widths for aligned output.
3. Print the header: `Devora Health Check`.
4. Print the `Required:` section. Each dependency is shown with a colored status prefix (green checkmark or red cross + name) followed by version in default color. In verbose mode, the shortened path is also shown.
5. Print the `Optional:` section in the same format.
6. Print a two-line summary: `Required met: <pct>% (<found>/<total>)` and `Optional met: <pct>% (<found>/<total>)`. The required percentage is green or red; the optional percentage is green or yellow.
7. Return `&process.PassthroughError{Code: 1}` if any required dependency is missing.
8. In strict mode, also return `&process.PassthroughError{Code: 1}` if any optional dependency is missing.
9. Otherwise, return nil.

## Output Format

```
Devora Health Check

Required:
  ✓ kitty   0.44.0      /opt/homebrew/bin/kitty
  ✗ claude               not found

Optional:
  ✓ nvim    v0.12.0     /opt/homebrew/bin/nvim
  ✗ jq                  not found

Required met: 100% (6/6)
Optional met:  66% (2/3)
```

Only the status indicator and dependency name are colored (green for found, red for missing). Version and path columns use the default terminal color. Paths under `$HOME` are shortened with `~`. Version strings are cleaned to extract just the version number (e.g., `"git version 2.50.1 (Apple Git-155)"` becomes `"2.50.1"`). Column widths are dynamically calculated to align all entries.

The summary percentages are colored: required is green (100%) or red; optional is green (100%) or yellow.

## Version Cleaning

`cleanVersion` uses a regex (`v?\d+(?:\.\d+)+`) to extract the version number from raw version command output, then strips any leading `v` prefix. If no match is found, the raw string is returned as-is. Examples:

| Raw Output | Cleaned |
|---|---|
| `kitty 0.44.0 created by Kovid Goyal` | `0.44.0` |
| `git version 2.50.1 (Apple Git-155)` | `2.50.1` |
| `NVIM v0.12.0` | `0.12.0` |
| `jq-1.7.1-apple` | `1.7.1` |

## Path Shortening

`shortenPath` replaces the `$HOME` prefix in paths with `~` for readability (e.g., `/Users/alice/.local/bin/kitty` becomes `~/.local/bin/kitty`).

## Exit Code Behavior

| Condition | Exit Code |
|-----------|-----------|
| All required found, strict off | 0 |
| All required found, strict on, all optional found | 0 |
| Any required missing | 1 |
| All required found, strict on, any optional missing | 1 |

## Testability

The package exposes two package-level variables for test injection:

```go
var lookPath = exec.LookPath
var getVersion = defaultGetVersion
```

Tests can replace `lookPath` and `getVersion` to simulate dependency presence/absence and version output without relying on the actual system `PATH`. `homeDir` can also be overridden for `shortenPath` tests.

## Testing

- Test `cleanVersion` with various real-world version strings (kitty, claude, git, uv, glow, zsh, nvim, mise, jq) and edge cases (empty, no match).
- Test `shortenPath` with paths under `$HOME`, paths outside `$HOME`, and empty string.
- Test `Check` with a dependency that is found (mock `lookPath` to return a path and `getVersion` to return version output); verify version is cleaned.
- Test `Check` with a dependency that is not found (mock `lookPath` to return an error).
- Test `Check` when the binary is found but the version command fails (mock `getVersion` to return an error); verify `Found` is true and `Version` is empty.
- Test `Check` with multi-line version output; verify only the first line is used and version is cleaned.
- Test `Run` with all dependencies found; verify output contains section headers, summary lines with counts, and return value is nil.
- Test `Run` with a missing required dependency; verify it returns `*process.PassthroughError` with code 1.
- Test `Run` with all required found but a missing optional dependency (strict off); verify it returns nil.
- Test `Run` with all required found but a missing optional dependency (strict on); verify it returns `*process.PassthroughError` with code 1.

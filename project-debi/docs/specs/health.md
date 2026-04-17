# Health Package Spec

Package: `internal/health`

## Purpose

Check and report on Devora IDE dependency availability and credential status. Each dependency is looked up on `PATH` and, if found, its version is retrieved. Credentials are verified by attempting to access the corresponding service API. Results are printed in a human-readable report grouped by required dependencies, optional dependencies, and credentials.

## Dependencies

- `devora/internal/process` -- `PassthroughError` for exit-code signalling
- `devora/internal/version` -- application version string
- `devora/internal/config` -- config file path
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

### CredentialStatus

```go
type CredentialStatus int

const (
    CredentialOK CredentialStatus = iota
    CredentialFailed
    CredentialUnchecked
)
```

Enum for the status of a credential check. `CredentialOK` means the credential is valid, `CredentialFailed` means authentication failed, and `CredentialUnchecked` means the prerequisite tool was not found so the check was skipped.

### CredentialResult

```go
type CredentialResult struct {
    Name    string
    Status  CredentialStatus
    Message string
}
```

The result of checking a single credential. `Name` is the service name (e.g., "GitHub"). `Status` indicates whether the check succeeded, failed, or was skipped. `Message` provides human-readable detail (e.g., "Logged in as Jane Doe", "auth token expired", or "gh not detected").

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
| `gh` | no | `gh --version` |

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
3. Print a version banner: `Devora Health Check (version: <version>)` using `getAppVersion()`.
4. Print the config file path via `getConfigPath()`. If `statFile` reports the file exists, show a green checkmark after the path. If the file does not exist, show a yellow `(not found)` marker. This is informational only and never affects the exit code.
5. Print the `Required:` section. Each dependency is shown with a colored status prefix (green checkmark or red cross + name) followed by version in default color. In verbose mode, the shortened path is also shown.
6. Print the `Optional:` section in the same format.
7. Check credentials. If `gh` was found in the optional results, first run `checkGitHubToken()` to check for a locally stored token via `gh auth token`. If no token is found, report `CredentialFailed` with the message `"no token stored (run: gh auth login)"` and skip the network call. If a token is found, run `checkGitHub()` to verify GitHub authentication via `gh api user --jq ".name // .login"`. If `gh` was not found, mark the credential as unchecked.
8. Print the `Credentials:` section. Each credential is shown with a colored status prefix: green checkmark for OK, red cross for failed, yellow question mark for unchecked.
9. Print a three-line summary: `Required met:`, `Optional met:`, and `Credentials met:`, each with `<pct>% (<found>/<total>)`. Labels are right-padded to align. The required percentage is green or red; the optional percentage is green or yellow; the credentials percentage is green, red (if any failed), or yellow (if only unchecked).
10. In strict mode, credential failures are treated like missing optional dependencies (exit code 1).
11. Return `&process.PassthroughError{Code: 1}` if any required dependency is missing.
12. In strict mode, also return `&process.PassthroughError{Code: 1}` if any optional dependency is missing or any credential check is not OK.
13. Otherwise, return nil.

## Output Format

```
Devora Health Check (version: 1.2.0)

Config: ~/.config/devora/config.json ✓

Required:
  ✓ kitty   0.44.0      /opt/homebrew/bin/kitty
  ✗ claude               not found

Optional:
  ✓ nvim    v0.12.0     /opt/homebrew/bin/nvim
  ✓ mise    2025.4.6    ~/.local/bin/mise
  ✓ gh      2.74.0      /opt/homebrew/bin/gh

Credentials:
  ✓ GitHub  Logged in as Jane Doe

Required met:    100% (6/6)
Optional met:    100% (3/3)
Credentials met: 100% (1/1)
```

Only the status indicator and dependency name are colored (green for found, red for missing). Version and path columns use the default terminal color. Paths under `$HOME` are shortened with `~`. Version strings are cleaned to extract just the version number (e.g., `"git version 2.50.1 (Apple Git-155)"` becomes `"2.50.1"`). Column widths are dynamically calculated to align all entries.

The summary percentages are colored: required is green (100%) or red; optional is green (100%) or yellow; credentials is green (100%), red (if any failed), or yellow (if only unchecked). Summary labels are right-padded to align to the longest label ("Credentials met:").

## Version Cleaning

`cleanVersion` uses a regex (`v?\d+(?:\.\d+)+`) to extract the version number from raw version command output, then strips any leading `v` prefix. If no match is found, the raw string is returned as-is. Examples:

| Raw Output | Cleaned |
|---|---|
| `kitty 0.44.0 created by Kovid Goyal` | `0.44.0` |
| `git version 2.50.1 (Apple Git-155)` | `2.50.1` |
| `NVIM v0.12.0` | `0.12.0` |

## Path Shortening

`shortenPath` replaces the `$HOME` prefix in paths with `~` for readability (e.g., `/Users/alice/.local/bin/kitty` becomes `~/.local/bin/kitty`).

## Exit Code Behavior

Credential failures are treated like missing optional dependencies for exit code purposes.

| Condition | Exit Code |
|-----------|-----------|
| All required found, strict off | 0 |
| All required found, strict on, all optional found, all credentials OK | 0 |
| Any required missing | 1 |
| All required found, strict on, any optional missing | 1 |
| All required found, strict on, any credential check not OK | 1 |

## Testability

The package exposes seven package-level variables for test injection:

```go
var getAppVersion = version.Get
var getConfigPath = config.ConfigPath
var statFile = os.Stat
var checkGitHubToken = defaultCheckGitHubToken
var lookPath = exec.LookPath
var getVersion = defaultGetVersion
var checkGitHub = defaultCheckGitHub
```

Tests can replace `lookPath` and `getVersion` to simulate dependency presence/absence and version output without relying on the actual system `PATH`. `checkGitHub` can be replaced to simulate GitHub authentication results without calling the real `gh` CLI. `getAppVersion` can be replaced to control the version string in the banner. `getConfigPath` and `statFile` can be replaced to simulate config file presence/absence. `checkGitHubToken` can be replaced to simulate the local token check independently of the network API call. `homeDir` can also be overridden for `shortenPath` tests.

## Testing

- Test `cleanVersion` with various real-world version strings (kitty, claude, git, uv, glow, zsh, nvim, mise) and edge cases (empty, no match).
- Test `shortenPath` with paths under `$HOME`, paths outside `$HOME`, and empty string.
- Test `Check` with a dependency that is found (mock `lookPath` to return a path and `getVersion` to return version output); verify version is cleaned.
- Test `Check` with a dependency that is not found (mock `lookPath` to return an error).
- Test `Check` when the binary is found but the version command fails (mock `getVersion` to return an error); verify `Found` is true and `Version` is empty.
- Test `Check` with multi-line version output; verify only the first line is used and version is cleaned.

**Version banner:**
- Test `Run` output starts with `Devora Health Check (version: <version>)` where `<version>` comes from `getAppVersion`.

**Config existence check:**
- Test `Run` when config file exists (mock `statFile` to return nil error); verify output contains the config path with a green checkmark.
- Test `Run` when config file does not exist (mock `statFile` to return an error); verify output contains the config path with `(not found)`.
- Test that config file absence does not affect exit code (all required found, config missing, return nil).

**Two-stage credential check:**
- Test `checkCredentials` with `gh` found and `checkGitHubToken` returning no token; verify `CredentialFailed` status with message `"no token stored (run: gh auth login)"` and `checkGitHub` is not called.
- Test `checkCredentials` with `gh` found, `checkGitHubToken` returning a token, and `checkGitHub` failing; verify `CredentialFailed` status with the API error message.
- Test `checkCredentials` with `gh` found, `checkGitHubToken` returning a token, and `checkGitHub` succeeding; verify `CredentialOK` status and "Logged in as ..." message.
- Test `checkCredentials` with `gh` not found; verify `CredentialUnchecked` status, "gh not detected" message, and `checkGitHubToken` is not called.

**Run integration:**
- Test `Run` with all dependencies found; verify output contains section headers (including Credentials), summary lines with counts, and return value is nil.
- Test `Run` with a missing required dependency; verify it returns `*process.PassthroughError` with code 1.
- Test `Run` with all required found but a missing optional dependency (strict off); verify it returns nil.
- Test `Run` with all required found but a missing optional dependency (strict on); verify it returns `*process.PassthroughError` with code 1.
- Test `Run` with `gh` not found; verify credentials section shows "?" marker and "gh not detected", and `checkGitHub` is not called.
- Test `Run` with `gh` found and auth succeeding; verify credentials section shows login name and summary "(1/1)".
- Test `Run` with `gh` found and auth failing; verify credentials section shows error message and summary "(0/1)".
- Test that credential failure does not affect exit code in non-strict mode (return nil when all required/optional are found).
- Test that credential failure causes exit code 1 in strict mode (return `*process.PassthroughError` with code 1 when all deps are found but credential check fails).

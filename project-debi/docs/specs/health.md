# Health Package Spec

Package: `internal/health`

## Purpose

Check and report on Devora IDE dependency availability and credential status. Each dependency is looked up on `PATH` and, if found, its version is retrieved. Credentials are verified by attempting to access the corresponding service API. `Gather()` collects everything into a structured `Report`, which is rendered either as a human-readable report (`Run`, grouped by required dependencies, optional dependencies, and credentials) or as JSON (`RunJSON`, for `debi health --json` — consumed by Devora's in-app Health Hub).

## Dependencies

- `devora/internal/process` -- `PassthroughError` for exit-code signalling
- `devora/internal/version` -- application version string
- `devora/internal/config` -- config file path, task-tracker provider lookup
- `devora/internal/credentials` -- task-tracker token lookup and `SetupHint`
- `devora/internal/style` -- shared Catppuccin Mocha palette + pre-built lipgloss styles for colored output
- `charm.land/lipgloss/v2` -- `lipgloss.Style` type for the `renderSummaryLine` percentage-style parameter

## Profile Resolution

`debi health` resolves an active profile before reading config so that profile-scoped keys (most notably `task-tracker.provider`) are visible. Resolution happens in the CLI layer via `cli.ResolveActiveProfile` (see [cli.md](./cli.md)); when it succeeds, `config.SetActiveProfile` is called and the profile-aware config reader picks up profile-level overrides.

Resolution order, stopping at the first hit (see `cli.ResolveActiveProfile` / `cli.ResolveActiveProfileByPath`):

1. Explicit `--profile-path <path>` flag — matched against `config.GetProfiles()[].RootPath` (compared after `filepath.Clean`). Selecting by path is unambiguous (profile names are not required to be unique). An unknown path returns a `*cli.UsageError` before `health.Run`/`health.RunJSON` is called. Devora's in-app Health Hub uses this flag.
2. Explicit `-p, --profile <name>` flag — looked up by name. An unknown name returns a `*cli.UsageError`. When both are given, `--profile-path` wins.
3. `DEBI_PROFILE_PATH` env-var — matched by path, as a *default* (Devora sets it on session PTYs so `debi health` typed in a terminal resolves the session's profile). A path that matches no profile falls through rather than erroring, so a stale env var never breaks the command.
4. CWD-based — `workspace.ResolveWorkspaceFromCWD(cwd)`. When the CWD falls inside a profile's workspaces root, that profile becomes active. A failure from `os.Getwd` is treated as "no CWD match" and falls through to global.
5. No active profile — all reads fall back to global config.

The `DEBI_PROFILE_PATH` env default applies to every profile-aware command (the shared `cli.ResolveActiveProfile`), so `pr submit`/`pr close` also honor it. The `--profile-path`/`--profile` flags are exposed on `health`.

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
    CredentialInfo
)
```

Enum for the status of a credential check. `CredentialOK` means the credential is valid, `CredentialFailed` means authentication failed, `CredentialUnchecked` means the prerequisite tool was not found so the check was skipped, and `CredentialInfo` marks an informational row that does not count toward the "Credentials met: X/Y" denominator (e.g., an optional feature that is not configured).

### CredentialResult

```go
type CredentialResult struct {
    Name    string
    Status  CredentialStatus
    Message string
    FixHint string
}
```

The result of checking a single credential. `Name` is the service name (e.g., "GitHub"). `Status` indicates whether the check succeeded, failed, or was skipped. `Message` provides human-readable detail (e.g., "Logged in as Jane Doe", "auth token expired", or "gh not detected"). `FixHint` is a bare, copy-pasteable command that resolves the issue when a single one exists (e.g. `gh auth login`); it is empty otherwise. The text renderer ignores `FixHint` (the command is already embedded in `Message`); it exists for JSON consumers (the Health Hub renders a copy button).

### Structured Report (JSON)

`Gather()` returns a `Report`, the JSON shape consumed by Devora's Health Hub (`debi health --json`). Paths are absolute; the text renderer shortens them for display.

```go
type DependencyStatus struct {
    Name    string `json:"name"`
    Found   bool   `json:"found"`
    Version string `json:"version"`
    Path    string `json:"path"`
}

type CredentialReport struct {
    Name    string `json:"name"`
    Status  string `json:"status"`  // "ok" | "failed" | "unchecked" | "info"
    Message string `json:"message"`
    FixHint string `json:"fixHint,omitempty"`
}

type FileCheck struct {
    Path    string `json:"path"`
    Found   bool   `json:"found"`
    FixHint string `json:"fixHint,omitempty"`
}

type Summary struct {
    RequiredMet      int `json:"requiredMet"`
    RequiredTotal    int `json:"requiredTotal"`
    OptionalMet      int `json:"optionalMet"`
    OptionalTotal    int `json:"optionalTotal"`
    CredentialsMet   int `json:"credentialsMet"`
    CredentialsTotal int `json:"credentialsTotal"`
}

type Report struct {
    Version     string             `json:"version"`
    Config      FileCheck          `json:"config"`
    Completion  FileCheck          `json:"completion"`
    Required    []DependencyStatus `json:"required"`
    Optional    []DependencyStatus `json:"optional"`
    Credentials []CredentialReport `json:"credentials"`
    Summary     Summary            `json:"summary"`
}
```

The `Completion` `FileCheck` carries a `FixHint` of `debi completion zsh > ~/.zsh/completions/_debi` when the completion file is missing. `Config` never carries a `FixHint`. Credential `Status` strings map from `CredentialStatus`: `CredentialOK`→`ok`, `CredentialFailed`→`failed`, `CredentialUnchecked`→`unchecked`, `CredentialInfo`→`info`.

## Dependency List

Required and optional dependencies:

| Name | Required | Version Command |
|------|----------|-----------------|
| `claude` | yes | `claude --version` |
| `git` | yes | `git --version` |
| `uv` | yes | `uv --version` |
| `zsh` | yes | `zsh --version` |
| `nvim` | no | `nvim --version` |
| `mise` | no | `mise --version` |
| `gh` | no | `gh --version` |

Kitty is **not** a dependency: the published Devora (Ember) build does not use Kitty. (The legacy OG TUI used Kitty at runtime but is source-only and unpublished.)

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

### Gather

```go
func Gather() Report
```

Runs every check (dependencies, credentials, config presence, completion presence, version) and returns the structured `Report`. It is the single source of truth for both renderers — `Run` (text) and `RunJSON` (JSON) call it.

### RunJSON

```go
func RunJSON(w io.Writer) error
```

Writes `Gather()` as indented JSON (via `json.MarshalIndent`) followed by a newline. Output is pure JSON with no styling, so `debi health --json | jq` works. Dispatched from the CLI when `--json` is passed; it has no exit-code semantics (the consumer interprets the report).

### Run

```go
func Run(w io.Writer, strict bool, verbose bool) error
```

Runs the full health check and prints a human-readable report. Calls `Gather()` once and renders its fields (the text output is unchanged from previous releases).

Behavior:
1. Call `Gather()` to obtain the structured report.
2. Calculate column widths for aligned output from the report's dependency rows.
3. Print a version banner: `Devora Health Check (version: <version>)` using `getAppVersion()`.
4. Print the config file path via `getConfigPath()`. If `statFile` reports the file exists, show a green checkmark after the path. If the file does not exist, show a yellow `(not found)` marker. This is informational only and never affects the exit code.
5. Print the `Required:` section. Each dependency is shown with a colored status prefix (green checkmark or red cross + name) followed by version in default color. In verbose mode, the shortened path is also shown.
6. Print the `Optional:` section in the same format.
7. Check credentials. If `gh` was found in the optional results, first run `checkGitHubToken()` to check for a locally stored token via `gh auth token`. If no token is found, report `CredentialFailed` with the message `"no token stored (run: gh auth login)"` and skip the network call. If a token is found, run `checkGitHub()` to verify GitHub authentication via `gh api user --jq ".name // .login"`. If `gh` was not found, mark the credential as unchecked. Then call `checkTrackerCredential()`; it always returns a row (info row when no tracker is configured), which is appended to the credential list.
8. Print the `Credentials:` section. Each credential is shown with a colored status prefix: green checkmark for OK, red cross for failed, yellow question mark for unchecked, muted circle (`○`) for info.
9. Print a three-line summary: `Required met:`, `Optional met:`, and `Credentials met:`, each with `<pct>% (<found>/<total>)`. Labels are right-padded to align. The required percentage is green or red; the optional percentage is green or yellow; the credentials percentage is green, red (if any failed), or yellow (if only unchecked). Info rows do NOT count toward the credentials denominator (see `countedCredentials`).
10. In strict mode, credential failures are treated like missing optional dependencies (exit code 1). Info rows are ignored by strict mode because they are not "met or not met".
11. Return `&process.PassthroughError{Code: 1}` if any required dependency is missing.
12. In strict mode, also return `&process.PassthroughError{Code: 1}` if any optional dependency is missing or any credential check is not OK.
13. Otherwise, return nil.

## Output Format

```
Devora Health Check (version: 1.2.0)

Config: ~/.config/devora/config.json ✓

Required:
  ✓ claude  2.1.181     ~/.local/bin/claude
  ✗ uv                   not found

Optional:
  ✓ nvim    v0.12.0     /opt/homebrew/bin/nvim
  ✓ mise    2025.4.6    ~/.local/bin/mise
  ✓ gh      2.74.0      /opt/homebrew/bin/gh

Credentials:
  ✓ GitHub        Logged in as Jane Doe
  ○ task-tracker  not configured (optional)

Required met:    100% (4/4)
Optional met:    100% (3/3)
Credentials met: 100% (1/1)
```

Only the status indicator and dependency name are colored (green for found, red for missing). Version and path columns use the default terminal color. Paths under `$HOME` are shortened with `~`. Version strings are cleaned to extract just the version number (e.g., `"git version 2.50.1 (Apple Git-155)"` becomes `"2.50.1"`). Column widths are dynamically calculated to align all entries.

Credential rows use four markers: green `✓` for OK, red `✗` for failed, yellow `?` for unchecked, muted `○` for info. All four come from the shared `internal/style` package, which centralizes the Catppuccin Mocha palette.

The summary percentages are colored: required is green (100%) or red; optional is green (100%) or yellow; credentials is green (100%), red (if any failed), or yellow (if only unchecked). Summary labels are right-padded to align to the longest label ("Credentials met:").

## Version Cleaning

`cleanVersion` uses a regex (`v?\d+(?:\.\d+)+`) to extract the version number from raw version command output, then strips any leading `v` prefix. If no match is found, the raw string is returned as-is. Examples:

| Raw Output | Cleaned |
|---|---|
| `gh version 2.62.0 (2024-11-14)` | `2.62.0` |
| `git version 2.50.1 (Apple Git-155)` | `2.50.1` |
| `NVIM v0.12.0` | `0.12.0` |

## Path Shortening

`shortenPath` replaces the `$HOME` prefix in paths with `~` for readability (e.g., `/Users/alice/.local/bin/claude` becomes `~/.local/bin/claude`). Applied only by the text renderer; the JSON `Report` keeps absolute paths.

## Exit Code Behavior

Credential failures are treated like missing optional dependencies for exit code purposes. Info rows are ignored.

| Condition | Exit Code |
|-----------|-----------|
| All required found, strict off | 0 |
| All required found, strict on, all optional found, all counted credentials OK | 0 |
| Any required missing | 1 |
| All required found, strict on, any optional missing | 1 |
| All required found, strict on, any counted credential check not OK | 1 |

"Counted credentials" excludes info rows (`CredentialInfo`).

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

- Test `cleanVersion` with various real-world version strings (gh, claude, git, uv, zsh, nvim, mise) and edge cases (empty, no match).
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

**Tracker credential row:**

Always appended to the credential list. The row is named `task-tracker` when unconfigured, or `<provider> token` when a provider is configured.

- When `task-tracker.provider` is `""`: `CredentialInfo`, message `"not configured (optional)"`. The row uses the name `task-tracker`.
- When `credentials.GetToken(provider)` succeeds with a non-empty token: `CredentialOK`, message `"stored in keychain"`.
- When `GetToken` returns a `*credentials.NotFoundError` (detected via `errors.As`): `CredentialFailed`, message is the multi-line output of `credentials.SetupHint(provider)`.
- When `GetToken` returns any other error: `CredentialFailed`, message is `err.Error()`.
- When `GetToken` returns an empty token with no error (defensive): `CredentialFailed`, message is `credentials.SetupHint(provider)`.

Implemented via two stubbable vars:

```go
var (
    getTrackerProvider = config.GetTaskTrackerProvider
    getTrackerToken    = credentials.GetToken
)
```

- Test `checkTrackerCredential` with no provider configured; verify `CredentialInfo` row with the "not configured" message.
- Test with provider configured and token returned; verify `CredentialOK` and `"stored in keychain"`.
- Test with `*credentials.NotFoundError`; verify `CredentialFailed` and message equal to `SetupHint(provider)`.
- Test with generic error; verify `CredentialFailed` and message equal to the error text.
- Test with `GetToken` returning `"", nil` (defensive); verify `CredentialFailed` and message equal to `SetupHint(provider)`.
- Test `Run` includes the tracker row when a provider is set.
- Test `Run` renders the info row and keeps the credentials summary at `(1/1)` (GitHub only) when no tracker is configured.

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

**Structured report / JSON:**
- Test that `dependencies` does not include `kitty`.
- Test `Gather` returns the expected required/optional rows, summary counts, config/completion presence, and credential rows for a stubbed environment (e.g. `gh` missing → optional 2/3, GitHub `unchecked`, tracker `info`).
- Test `Gather` sets `Completion.FixHint` when the completion file is missing, and the GitHub credential `FixHint` (`gh auth login`) when no token is stored.
- Test `RunJSON` emits JSON that unmarshals back into a `Report` with the expected version and counts.

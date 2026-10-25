# Config Package Specification

Package: `internal/config`

## Overview

The config package manages Devora's two-level JSON configuration system (global config + per-profile config) and profile loading. It provides a resolution chain where profile-level settings override global settings for supported keys.

## Data Types

### Profile

```go
type Profile struct {
    Name     string
    RootPath string         // Absolute path to profile root directory
    Config   map[string]any // Raw parsed JSON from the profile's config.json
}
```

## File Locations

| File | Description |
|------|-------------|
| `~/.config/devora/config.json` | Global config file (override with `DEVORA_CONFIG_PATH`) |
| `<profile-root>/config.json` | Profile-specific config |
| `<profile-root>/repos/` | Auto-discovered repos directory |
| `<profile-root>/workspaces/` | Profile-scoped workspaces directory |

The global config path is resolved by `defaultConfigPath()` at package init: if the `DEVORA_CONFIG_PATH` env-var is set it is used verbatim, otherwise it defaults to `~/.config/devora/config.json`. This lets a host process (Devora's Tauri backend, or the acceptance-test harness) point `debi` at the same config file it uses, matching the Ember backend's `workspace.rs:global_config_path` override.

## Global Config

### Structure

```json
{
  "profiles": ["/path/to/profile-1", "/path/to/profile-2"],
  "terminal": {
    "default-app": "shell"
  },
  "prepare-command": "prep",
  "task-tracker": {
    "provider": "asana",
    "asana": {
      "workspace-id": "YOUR-WORKSPACE-GID",
      "project-id": "YOUR-PROJECT-GID",
      "cli-tag": "YOUR-CLI-TAG-GID",
      "section-id": "YOUR-SECTION-GID"
    }
  },
  "feature": {
    "branch-prefix": "feature"
  },
  "pr": {
    "auto-merge": true
  }
}
```

`terminal.default-app`, `prepare-command`, and `repos` are part of the shared config format but are read by the Devora app, not by `debi`.

`task-tracker.*`, `feature.branch-prefix`, and `pr.auto-merge` are profile-overridable. `task-tracker.<provider>.<key>` merges per leaf, so global and profile can each contribute different leaves under the same provider (e.g., global sets `workspace-id`, profile sets `project-id`).

### Loading and Caching

The global config is loaded lazily from `~/.config/devora/config.json` on first access and cached in a package-level variable.

- On first call, reads and parses the file. If the file does not exist, returns an empty map.
- On subsequent calls, returns the cached value.
- If the file exists but contains invalid JSON, returns an empty map (log a warning).
- The cache is reset after operations that mutate the global config file (e.g., `SetPrAutoMergeGlobal`), so the next access re-reads from disk.

### Concurrency Note

Devora is single-threaded from a business logic perspective. The global config cache does not require mutex protection. A plain package-level variable with a `loaded` flag is sufficient. If concurrency requirements change in the future, protect with a `sync.Mutex`.

## Config Resolution Chain

### Dot-Path Resolution

Config values are accessed via dot-separated paths that navigate into nested JSON objects. The path is split on `"."` and each segment walks into nested `map[string]any` values. Returns the value if the full path resolves, or not-found if any segment is missing or a non-map intermediate is encountered.

Example: resolving `"terminal.default-app"` navigates `config["terminal"].(map[string]any)["default-app"]`.

### Profile-Overridable Resolution

Resolution order:
1. If an active profile is set, resolve the path against the active profile's `Config`. If found, return it.
2. Resolve the path against the global config. If found, return it.
3. Return not-found.

### Global-Only Resolution

Resolves the path against the global config only. Skips profile lookup entirely.

### Scope Classification

| Config Key | Scope |
|------------|-------|
| `profiles` | global-only |
| `terminal.default-app` | profile-overridable |
| `prepare-command` | profile-overridable |
| `task-tracker.provider` | profile-overridable |
| `task-tracker.<provider>.<key>` | profile-overridable (per-leaf merge: profile and global can each contribute different leaves) |
| `feature.branch-prefix` | profile-overridable |
| `pr.auto-merge` | per-repo + profile-overridable (per-repo > profile > global) |
| `name` | profile-only (read directly from `Profile.Config`) |
| `repos` | profile-only (read directly from `Profile.Config`) |

## Active Profile State

```go
var activeProfile *Profile // nil when not set

func SetActiveProfile(p *Profile)
func GetActiveProfile() *Profile
```

- `SetActiveProfile(nil)` clears the active profile.
- `GetActiveProfile()` returns `nil` when no profile is active.
- Functions that require an active profile (`SetPrAutoMergeProfile`) return an error if `GetActiveProfile()` is `nil`. The error message: `"no active profile set"`.

## Profile Operations

### GetProfiles

```go
func GetProfiles() []Profile
```

Reads the `profiles` key from global config (a `[]string` of absolute paths to profile root directories). For each path:

1. Check if `<path>/config.json` exists. If not, log a warning and skip.
2. Parse the file as JSON. If parsing fails (invalid JSON or OS error), log a warning and skip.
3. Check that the parsed JSON contains a `"name"` key. If not, log a warning and skip.
4. Construct a `Profile` with `Name` from the JSON `"name"`, `RootPath` from the path, and `Config` from the parsed JSON.

Returns the list of successfully loaded profiles. Returns an empty slice if `profiles` is absent from global config.

Warnings are logged via Go's `log` package (not returned as errors), since partial results are expected and useful.

### SetActiveProfile / GetActiveProfile

See [Active Profile State](#active-profile-state) above.

### Global Config Helpers

```go
func loadFreshGlobalConfig() map[string]any
```

Reads the global config file from disk (bypassing the cache). Returns an empty map if the file does not exist or contains invalid JSON.

```go
func writeFreshGlobalConfig(cfg map[string]any) error
```

Writes the given map to the global config file path with 4-space JSON indentation, creating the parent directory if needed. Resets the global config cache after writing.

Both helpers are unexported and used by the global-config setters (e.g., `SetPrAutoMergeGlobal`).

## Workspace Root

### WorkspacesRootForProfile

```go
func WorkspacesRootForProfile(p *Profile) string
```

Returns `<p.RootPath>/workspaces`. Does not check whether the directory exists.

## Config Mutation

### SetPrAutoMergeGlobal

```go
func SetPrAutoMergeGlobal(value *bool) error
```

Writes the `"pr.auto-merge"` key to the global config.

- If `value` is non-nil, sets `"pr.auto-merge"` to `*value`, creating the `"pr"` object if it does not already exist.
- If `value` is nil, removes the `"auto-merge"` key. If the `"pr"` object becomes empty as a result, it is pruned too.

Operates on the global config only (does not touch profile config). Uses `loadFreshGlobalConfig` / `writeFreshGlobalConfig` to bypass the cache.

### SetPrAutoMergeProfile

```go
func SetPrAutoMergeProfile(value *bool) error
```

Writes the `"pr.auto-merge"` key to the active profile's `config.json`.

- If `value` is non-nil, sets `"pr.auto-merge"` to `*value`, creating the `"pr"` object if it does not already exist.
- If `value` is nil, removes the `"auto-merge"` key. If the `"pr"` object becomes empty as a result, it is pruned too.

Uses the read-modify-write pattern (see [Profile Config Mutation](#profile-config-mutation)). Returns an error if no active profile is set.

### Profile Config Mutation

Profile config changes use a read-modify-write pattern:

1. Read `<p.RootPath>/config.json` from disk.
2. Parse as JSON into `map[string]any`.
3. Apply the mutation.
4. Write the updated map back to `<p.RootPath>/config.json` with 4-space JSON indentation.
5. If the profile being updated is the active profile (same `RootPath`), update the in-memory active profile with the new config data.

Returns an error if any filesystem or JSON operation fails.

## Generic Getter

### Get

```go
func Get(path string) (any, bool)
```

Resolves a config key using the standard profile-overridable resolution chain: active profile first, then global config. Returns `(value, true)` if the key is found at either level, or `(nil, false)` if not found. This is the exported counterpart of the internal `get(path)` function, intended for callers that need to read arbitrary config keys without a typed convenience wrapper.

## Convenience Getters

### ConfigPath

```go
func ConfigPath() string
```

Returns the global config file path (`~/.config/devora/config.json`). The path is computed once at package init from `os.UserHomeDir()` and cached in a package-level variable.

### GetTaskTrackerProvider

```go
func GetTaskTrackerProvider() string
```

Returns the resolved value of `"task-tracker.provider"` (profile-overridable). Returns `""` when unset or when the stored value is not a string. An empty string signals "no task tracker configured".

### GetTaskTrackerString

```go
func GetTaskTrackerString(provider, key string) string
```

Resolves `"task-tracker.<provider>.<key>"` (profile-overridable) via the standard per-leaf resolver: profile first, global fallback. Returns `""` when the leaf is unset at both levels or the value is not a string.

Each leaf is queried independently, so profile and global can contribute different leaves under the same provider (e.g., global sets `workspace-id`, profile sets `project-id`).

### GetBranchPrefix

```go
func GetBranchPrefix(fallback string) string
```

Returns the resolved value of `"feature.branch-prefix"` (profile-overridable). Returns `fallback` when the key is unset or the value is not a string.

### GetAutoMergeDefault

```go
func GetAutoMergeDefault(fallback bool) bool
```

Returns the resolved value of `"pr.auto-merge"` (profile-overridable). Returns `fallback` when the key is unset or the value is not a bool.

### GetPrAutoMergeGlobalRaw

```go
func GetPrAutoMergeGlobalRaw() *bool
```

Returns a pointer to the value of `"pr.auto-merge"` stored in the global config, or `nil` if the key is unset or not a bool. Reads only from the global config -- does not fall back to any other scope. Intended for UI and diagnostic code that needs to distinguish "unset at this scope" from "set to `false`".

### GetPrAutoMergeProfileRaw

```go
func GetPrAutoMergeProfileRaw() *bool
```

Returns a pointer to the value of `"pr.auto-merge"` stored in the active profile's config, or `nil` if there is no active profile, the key is unset at the profile scope, or the value is not a bool. Reads only from the active profile -- does not fall back to global.

### GetAutoMergeDefaultForRepo

```go
func GetAutoMergeDefaultForRepo(fallback bool, getRepo func() (*bool, error)) bool
```

Resolves `"pr.auto-merge"` with per-repo > profile > global > `fallback` precedence. The `getRepo` callback is expected to return `(*bool, error)`; a non-nil pointer wins outright. The callback indirection keeps this package free of dependencies on `internal/git`. When `getRepo` returns an error, a warning is written to stderr via the standard `log` package and resolution falls through to `GetAutoMergeDefault(fallback)`. A `nil` callback (or a callback that returns `(nil, nil)`) is treated as "no per-repo value".

## Per-repo overrides

Per-repo state for `pr.auto-merge` lives in git's local config (`devora.pr.auto-merge`), which git stores in `$GIT_COMMON_DIR/config` and shares natively across all linked worktrees of a clone (including bare clones). This package does not read the git config directly; callers (notably `internal/submit`) plumb the per-repo value into `GetAutoMergeDefaultForRepo` via the `getRepo` callback. The read and write primitives live in `internal/git` (`GetRepoConfigBool`, `SetRepoConfigBool`, `UnsetRepoConfig`).

A wrong-type value stored under `devora.pr.auto-merge` (e.g., `maybe`) is treated as "unset" by `GetRepoConfigBool` -- resolution falls through to the profile/global layers rather than propagating the parse failure.

## JSON Formatting

All JSON written by this package uses 4-space indentation (`json.MarshalIndent(data, "", "    ")`).

## Testing Notes

### Test Setup

Tests should use temporary directories for both the global config location and profile directories. The package provides exported helpers for cross-package test use:

```go
func SetConfigPathForTesting(path string)
```

Redirects the global config path to a temporary directory and resets the config cache.

```go
func ResetForTesting()
```

Clears the active profile and resets the global config cache. Must be called between test cases.

### Test Cases

**Config resolution chain:**
- Value in profile config only: returns profile value.
- Value in global config only: returns global value.
- Value in both: returns profile value (profile wins).
- Value in neither: returns `(nil, false)`.
- Global-only key ignores profile config even if present.

**Dot-path resolution:**
- Single-segment path (e.g., `"name"`).
- Multi-segment path (e.g., `"terminal.default-app"`).
- Missing intermediate key returns not-found.
- Non-map intermediate (e.g., path `"a.b"` where `a` is a string) returns not-found.

**Profile loading (`GetProfiles`):**
- Valid profiles are loaded.
- Profile with missing `config.json` is skipped with warning.
- Profile with invalid JSON is skipped with warning.
- Profile with missing `"name"` key is skipped with warning.
- Empty `profiles` list returns empty slice.

**Convenience getters:**
- Return correct type (string, int) from resolved config value.
- Return fallback when key is absent.
- Handle type mismatch gracefully (e.g., key exists but is wrong type: return fallback).

**Task-tracker getters:**
- `GetTaskTrackerProvider` returns `""` when unset.
- `GetTaskTrackerProvider` returns the profile value when both profile and global are set (profile wins).
- `GetTaskTrackerString` returns `""` when the leaf is unset at both levels.
- `GetTaskTrackerString` resolves per leaf: a leaf set only globally is returned, and a leaf set only in the profile is returned, even when other sibling leaves come from the other level.
- `GetTaskTrackerString` returns `""` when the stored value is not a string.
- `GetBranchPrefix` returns the fallback when unset or the stored value is not a string.
- `GetBranchPrefix` returns the profile value when both profile and global are set.
- `GetAutoMergeDefault` returns the fallback when unset or the stored value is not a bool.
- `GetAutoMergeDefault` returns the profile value when both profile and global are set.


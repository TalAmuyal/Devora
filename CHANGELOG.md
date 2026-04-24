# Changelog

All notable changes to Devora will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/).

Types of changes:
- `Added` for new features
- `Changed` for changes in existing functionality
- `Deprecated` for soon-to-be removed features
- `Removed` for now removed features
- `Fixed` for any bug fixes
- `Security` in case of vulnerabilities

## Unreleased

### Added

- `debi pr submit` auto-merge is now configurable via `pr.auto-merge` (bool, profile-overridable) in `config.json`; defaults to on when unset
- New `--auto-merge` flag on `debi pr submit` to force-enable auto-merge for a single invocation, overriding the config
- Per-repo override for `pr.auto-merge`, stored in git's local config (`devora.pr.auto-merge`) and shared across all linked worktrees of a clone. Precedence: per-repo > profile > global > built-in default (on)
- New `debi pr auto-merge <enable|disable|reset|show> [--scope=repo|profile|global] [--json]` command to manage the default at any of the three scopes. `reset` is idempotent; `show` prints the resolved value plus each layer's contribution
- A mise command to build and open dev Devora in one go

### Changed

- `-b, --blocked` on `debi pr submit` is now also a per-invocation override of the new `pr.auto-merge` config (behavior unchanged when the config is unset)
- Scheduled nightly builds are skipped when `master` has not advanced since the previous nightly
- `mise mac-install` now automatically runs `download-deps` first, so third-party bundler dependencies are fetched on demand

### Removed

- Top-level `debi submit`, `debi close`, and `debi check` aliases; use `debi pr submit`, `debi pr close`, and `debi pr check` instead

## 2026-04-21.0

### Added

- `debi pr` commands for a detached-HEAD PR workflow:
	- `debi pr submit` commits changes, creates a tracker task (if configured), pushes a feature branch, and opens a GitHub PR
	- `debi pr close` marks the tracker task complete (if configured), deletes the branch, and returns to detached HEAD on the default branch
	- `debi pr check` reports PR status, CI checks, and code reviews for the current branch
- `detached-flow` Claude Code plugin, bundled with Devora, providing `submit-pr`, `close-pr`, and `check-pr` shell aliases (wrapping the corresponding `debi pr` subcommands) and a `submit-pr` skill (both model and user-invocable)
- `team-work` Claude Code plugin skill for leading and coordinating a team of agents on a task (user-invocable only)
- Bundled Devora bootstrap now prepends each cc-plugin `bin/` directory to `PATH`, enabling plugins to ship first-class CLI commands alongside skills and hooks
- `ccc` now supports `update`, `-u`, and `--update` to update Claude Code via `claude --update`
- Settings page fields for configuring the workspace terminal app (`terminal.default-app`) at global and per-profile scope
- `debi health` improvements:
	- Version display
	- Config file status
	- `zsh` completion installation check (optional; fails under `--strict` if missing)
	- Task-tracker credential status (when a provider is configured)
	- Performs a two-stage credential check, distinguishing between missing tokens and authentication failures
	- `--profile <name>` flag for explicit profile selection (defaults to CWD-based resolution)
- User guide: sections for `ccc` command, Judge plugin, `debi` CLI, shell completions, and troubleshooting

### Changed

- Default workspace app is now `shell` (bare login/interactive shell) instead of `nvim`; existing configs with `"terminal.default-app": "nvim"` continue to work unchanged
	- Now configurable from the settings page
- `ccc` now sets Claude Code's effort level to "max" and uses `claude-opus-4-7` for Opus and Sonnet tasks

### Removed

- `debi`: removed hidden CLI aliases `w`, `a`, `r`
	- Use full command names (`workspace-ui`, `add`, `rename`)
	- Not needed anymore thanks to the shell completion feature

### Fixed

- Kitty launch command for shell sessions no longer wraps in `-c shell` (no shell-in-shell)
	- Triggered either by the `"shell"` sentinel or by supplying a value equal to `$SHELL`
- Main page (`debi workspace-ui`):
	- Repo rows no longer wrap or push the clean/dirty status off-screen when branch/worktree names are long
	- Long values are tail-truncated with `…`
	- Columns are separated by a double-cell gap for cleaner visual separation
	- Dirty status visibility by setting it to bold
- `debi` crash reports now echo the log content to stderr in addition to writing the log file
- User guide cheatsheet: includes all keyboard shortcuts and fixes mislabeled ctrl+1/2/3 description

## 2026-04-10.1

### Fixed

- Fix DMG file path in CD workflow smoke test and publish steps

## 2026-04-10.0

### Added

- CI/CD pipelines with automated nightly releases (`nightly-YYYY-MM-DD`), stable releases on `v*` tags, smoke tests, and changelog-extracted release notes
- 23 git shortcut commands for common workflows (`gaa`, `gb`, `gd`, `gl`, `gst`, and more)
- Version tracking displayed in the settings page
- `debi health` command for verifying Devora dependencies, with optional GitHub credential checking via `gh`
- Diagnostic error page when bundled tools are missing from PATH
- Shell completion scripts via `debi completion <bash|zsh|fish>`
- `-h`/`--help` flag support in the debi CLI
- Project documentation: MIT LICENSE, SECURITY.md, CONTRIBUTING.md, THIRD_PARTY_LICENSES.md, bundled third-party license files, and updated README with getting started guide and prerequisites

### Changed

- `debi health --strict` now treats credential failures as missing optional dependencies (exit code 1)
- DMG filename now includes the version (e.g., `Devora_2026-03-28.0.dmg`)

### Removed

- Legacy `cc-simple-statusline.sh` (superseded by Go binary in project-status-line)
- `jq` as an optional dependency from health check

### Fixed

- Launch failure caused by PATH manipulation in shell startup files overriding `debi` location
- Footer now stays attached to the bottom of the screen when content is short

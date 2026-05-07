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

### Changed

- User Guide tab (F1) and View Changelog now use `glimpse-tty` instead of `glow` for rendering
- Always show the Kitty tab bar, even with a single tab
- Judge now recognizes any env-var prefix (`NAME=VALUE`) generically instead of requiring hardcoded entries

### Fixed

- Judge no longer crashes on commands where all tokens are env-var assignments, which previously bypassed permission checks

### Removed

- Removed `glow` as a bundled dependency (replaced by `glimpse-tty`)

## 2026-05-06.0

### Highlights

#### Crit integration

[Crit](https://crit.md/) is an app that provides a GitHub-like code review experience for Claude's plan, as well as local changes.
Most of the changes in this release are related to the Crit integration.

- Crit and glimpse-tty are now bundled as vendored dependencies
- Added `review.open-mode` config key to control how Crit review UI opens (tab, overlay, or browser)
	- Defaults to `tab`

#### `cmd+shift`-based Vim-style key-bindings

- Kitty split panes: `cmd+shift+\` (pipe - vertical split) and `cmd+shift+-` (dash horizontal)
	- New panes inherit the active pane's working directory
- `cmd+shift+h/j/k/l` moves focus
- `cmd+shift+w` closes a pane

### Added

- One-line `install.sh` installer: downloads the latest release, replaces any existing app, clears macOS quarantine, and installs shell completion
	- Pass `--nightly` for nightly builds
- `debi pr auto-merge` command and `pr.auto-merge` config key for controlling PR auto-merge at repo, profile, or global scope
	- `--auto-merge` and `-b`/`--blocked` flags on `debi pr submit` override for a single invocation
- `debi util yaml-validate` and `debi util toml-validate` validation commands
- `debi get-conf` command for reading config values from scripts
- Judge audit log: every decision is logged to `~/.claude/cc-judge-audit.jsonl` with decision trail and timing data
	- Queryable via `mise stats` in project-judge (not currently bundled with Devora, but available in the repo)

### Changed

- Workspace-level `debi gst` and `debi gcl`: when run at the workspace root, both commands now operate on all repos in parallel; `debi gst` prints per-repo summaries (branch, file counts, commits behind origin, PR status)
- Judge expanded coverage:
	- Auto-approves more safe commands (`command -v`, `bash -n`, `source .venv/bin/activate`, crit file reads)
	- Strips `timeout`/`time`/`watch` prefixes
	- Declines `node`/`bun`/`shellcheck` (directing to mise tasks)
- Claude Code now starts in plan mode by default (part of the Crit integration)
- Titlebar color is now `#181926` (Catppuccin Macchiato crust) for a consistent look across macOS themes
- Tab titles for Devora pages are now bracketed (`[Devora]`, `[Shell]`, `[Crit]`)
- Bootstrap and Kitty tab launchers now honor `$SHELL` instead of hardcoding `zsh`
- User Guide tab (F1) launches `glow` directly for faster startup
- Updated 3rd-party dependencies: UV v0.11.8→v0.11.10

### Fixed

- Race condition in Judge's JSON file writes — concurrent writes could corrupt data
- Missing comma in Judge's `git tag -l` approval rule

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

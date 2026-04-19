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

- `debi pr submit` command for committing changes, creating a tracker task, pushing a feature branch, and opening a GitHub PR from detached HEAD
- `debi pr close` command for marking the tracker task complete, deleting the branch, and returning to detached HEAD on the default branch
- `process.WithSilent()` exec option that routes `RunPassthrough` stdout/stderr to `io.Discard`; used by submit/close in Normal and Quiet modes.
- `debi health` now reports task-tracker credential status when a provider is configured
- `debi health --profile <name>` explicitly selects a profile to check (defaults to CWD-based resolution)
- `debi pr check` command (alias: `debi check`) for checking GitHub PR status, CI checks, and code reviews for the current branch
- `debi util json-validate` subcommand for validating JSON files with line:column error positions
- User guide: sections for `ccc` command, Judge plugin, `debi` CLI, shell completions, and troubleshooting
- Version display and config file status in `debi health` output
- debi health: reports whether zsh completion is installed (optional; fails under `--strict` if missing).
- GitHub Actions workflow to enforce CHANGELOG.md updates on pull requests
- Optional pre-commit hook for local CHANGELOG.md reminders, installed via `mise run install-hooks`
- `ccc` now supports `update`, `-u`, and `--update` to update Claude Code via `claude --update`

### Fixed

- User guide: cheatsheet now includes all keyboard shortcuts and fixes mislabeled ctrl+1/2/3 description
- debi: PR commands (submit/close/check) now print a friendly error instead of crashing when run outside a git repository.

### Changed

- debi: Crash reports now echo the log content to stderr in addition to writing the log file.
- Debi workspace-ui UI improvement
	- Repo rows in workspace cards no longer wrap or push the clean/dirty status off-screen when a branch or worktree name is long
	- Long values are tail-truncated with `…`
	- Columns are separated by a single-cell gap
	- Dirty status is now bold
- `debi health` now performs a two-stage credential check, distinguishing between missing tokens and authentication failures
- `debi health` always shows a task-tracker row; when unconfigured it is a neutral informational row (`○ task-tracker  not configured (optional)`), excluded from the "Credentials met: X/Y" denominator and from `--strict` accounting
- Automated release-cutting process: `cut-release.sh` now handles pre-flight checks, optional AI-powered changelog cleanup, interactive review pause, and PR creation in a single script
- Release tagging is now automated via GitHub Actions when release PRs are merged
- `ccc` now sets Claude Code's effort level to "max"
- `ccc` now uses `claude-opus-4-7` for Opus and Sonnet tasks
- mac-install: automatically installs zsh completion to `~/.zsh/completions/_debi`. Prints a notice if `.zshrc` is not configured to source that directory.
- USER_GUIDE: promoted Shell Completions from Optional Additions to a Recommended section.

### Removed

- debi: removed hidden CLI aliases `w`, `a`, `r`. Use full command names (`workspace-ui`, `add`, `rename`) or install shell completion with `debi completion zsh > ~/.zsh/completions/_debi`.

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

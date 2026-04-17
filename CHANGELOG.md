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

- `debi pr status` command (alias: `debi prs`) for checking GitHub PR status, CI checks, and code reviews for the current branch
- `debi util json-validate` subcommand for validating JSON files with line:column error positions
- User guide: sections for `ccc` command, Judge plugin, `debi` CLI, shell completions, and troubleshooting
- Version display and config file status in `debi health` output
- debi health: reports whether zsh completion is installed (optional; fails under `--strict` if missing).

### Fixed

- User guide: cheatsheet now includes all keyboard shortcuts and fixes mislabeled ctrl+1/2/3 description

### Changed

- Debi workspace-ui UI improvement
	- Repo rows in workspace cards no longer wrap or push the clean/dirty status off-screen when a branch or worktree name is long
	- Long values are tail-truncated with `…`
	- Columns are separated by a single-cell gap
	- Dirty status is now bold
- `debi health` now performs a two-stage credential check, distinguishing between missing tokens and authentication failures
- Automated release-cutting process: `cut-release.sh` now handles pre-flight checks, optional AI-powered changelog cleanup, interactive review pause, and PR creation in a single script
- Release tagging is now automated via GitHub Actions when release PRs are merged
- `ccc` now sets Claude Code's effort level to "max"
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

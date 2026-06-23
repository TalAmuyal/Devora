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

- Updated README.md with clearer installation instructions
- `debi preview <TAB>` now completes file paths; the `--stack`/`-h`/`--help` flags surface once the word begins with `-`

## 2026-06-23.0

### Highlights

#### Markdown/HTML preview

Devora now has a built-in Markdown and HTML previewer, accessible via a new `debi preview` command.
Markdown previews render Mermaid diagrams.

Try it out with `debi preview path/to/file.md` or `debi preview path/to/file.html`.

#### New Palette commands

- `Clone repo into Profile`: clone a git repo into a profile's `repos/` directory from a pasted URL (GitHub HTTPS/SSH or repo-page URL)
- `Duplicate current session`: duplicate the current workspace session (worktrees (with their commits) and CLAUDE.md)
- `Close Current Session`: close the current workspace session/tab

#### Customizable Claude Code configuration

Both model and effort level are now configurable globally and per-profile, with a new "Claude Models & Effort" card in the Profile Manager.
This decouples Devora release from Claude model releases, allowing you to use newly released models without updating Devora.

### Added

- Added the ability to clone a git repo into a profile's `repos/` directory from a pasted URL (GitHub HTTPS/SSH or repo-page URL) — reachable from the Command Palette, the Workspace Hub's New Task form, and the Profile Manager
- Added a filter to the repo list (in the New Task form and the Add Repo dialog): type to narrow by name, with `↑/↓` to move and `Enter` to select/toggle
- Added the ability to duplicate a workspace using "Duplicate Current Session" (Command Palette) or the Workspace Hub's "Duplicate" button (next to "Open")
- Added a "Close Current Session" command to the Command Palette
- Added configurable Claude model tiers (Opus/Sonnet/Haiku) and effort level, editable per user and per profile in the Profile Manager (a "User Defaults" entry plus a per-profile "Claude Models & Effort" card). Each setting can be a value, None (let Claude Code use its own default), or unset (fall through profile → user → Devora default); models are free text, so a newly released model can be used without updating Devora
- Added an in-app **Health Hub** that checks Devora's dependencies, credentials, configs, etc. — reachable from the Command Palette and the Workspace Hub
- Added `debi preview [--stack] <file>` to render a Markdown or HTML file in a preview pane to the right of the terminal
	- Markdown previews render Mermaid code blocks as diagrams
	- Re-previewing the same file refreshes its pane
	- `--stack` opens an additional pane

### Changed

- The Command Palette now focuses its search field on open so you can type a filter immediately; `Esc` closes it and `↑/↓`/`Enter` navigate and run
- Moved the Workspace Hub's "Health" and "Save loading latencies" actions into a new burger (☰) menu to the right of the profile dropdown
- `debi health` now supports profile-by-path selection (a `--profile-path` flag and a `DEBI_PROFILE_PATH` env-var), and now honors `DEVORA_CONFIG_PATH`
- `debi health` gained a `--json` output mode
- Judge now allows `open` and `sed`, and abstains on `Write` calls
- `ccc` now treats its `CLAUDE_CODE_*` env-vars as defaults, respecting any pre-existing definitions in the environment instead of overriding them

### Removed

- Removed the `ccc --fable` flag; set the Opus/Sonnet model to `claude-fable-5` in the Claude config instead
- Removed the `debi add` command (and its OG add-repo TUI) as part of the Devora OG → Ember migration; use the "Add Repo to Workspace" Command Palette command instead
- Removed Kitty from `debi health` checks
- Removed the unused `review.open-mode` config key and the legacy OG Kitty/glimpse-tty path from the Crit wrapper; the published Ember build always opens Crit in its built-in overlay

## 2026-06-16.0

### Highlights

#### The Ember Project is GA

Ember is the codename for the UI rewrite of Devora using web technologies.

Ember brings:

- Native desktop app built with Tauri and xterm.js
- Refreshed and modern web-based UI
- Command Palette (Shift+Shift): a searchable list of actions with a filter box and `j`/`k` navigation
- HTML-based User Guide (F1)

### Added

- `--fable` flag for `ccc` that runs Claude Code on the Fable 5 model (`claude-fable-5`) for Opus and Sonnet (Haiku still uses Sonnet)

### Fixed

- `debi pr submit` now shows pre-commit hook output when a commit fails in non-verbose mode

### Changed

- `ccc` now defaults to the Opus 4.8 model (`claude-opus-4-8`) for Opus and Sonnet
- Reduce Claude effort level from "max" to "xhigh" for better response times
- Improved team-work skill: structured team roles, added planning interview phase, and delegation guidance
- Judge
	- Now recognizes any env-var prefix (`NAME=VALUE`) generically instead of requiring hardcoded entries
	- Declines `cargo` and `tsx` commands, directing to `mise` tasks
	- Evaluates `find -exec` commands by inspecting the executed command rather than blanket-deferring all `-exec` usage
- Updated 3rd-party dependencies:
	- UV v0.11.10 → v0.11.17
	- Crit v0.10.5 → v0.15.4

### Deprecated

- The `--ember` flag on the one-line installer (`install.sh`) is now a no-op (Devora is the Ember build). It is still accepted for backward compatibility.

### Removed

- Removed `glow` as a bundled dependency (replaced by `glimpse-tty`, which is now deprecated in favor of the built-in markdown renderer in the Ember UI)

### Fixed

- Judge no longer crashes on commands where all tokens are env-var assignments

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

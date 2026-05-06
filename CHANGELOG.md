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

- Judge audit log: every invocation (allow, deny, defer, error) is logged to `~/.claude/cc-judge-audit.jsonl` with full decision trail, structured reason keys, and timing data. Concurrency-safe via `O_APPEND`. Uncaught exceptions are captured with full tracebacks
- `audit-stats.py` script (`mise stats` in project-judge) for querying audit log statistics with time, decision, and tool filters
- `update-deps` script and mise task for automated third-party dependency updates (checks latest GitHub releases, updates JSON/checksums, refreshes artifacts)
- `debi get-conf` command for reading config values from scripts
- `review.open-mode` config key to control how crit review UI opens (tab, overlay, or browser)
- Crit is now bundled as a vendored dependency (binary + Claude Code plugin)
- Catppuccin attribution in THIRD_PARTY_LICENSES.md
- `debi util yaml-validate <file|->` validates YAML files (multi-document streams supported, empty input rejected)
- `debi util toml-validate <file|->` validates TOML files (empty input rejected; comment-only files are valid)
- `debi pr submit` auto-merge is now configurable via `pr.auto-merge` (bool, profile-overridable) in `config.json`; defaults to on when unset
- New `--auto-merge` flag on `debi pr submit` to force-enable auto-merge for a single invocation, overriding the config
- Per-repo override for `pr.auto-merge`, stored in git's local config (`devora.pr.auto-merge`) and shared across all linked worktrees of a clone. Precedence: per-repo > profile > global > built-in default (on)
- `mise clean` task to remove build artifacts (`bin/` and `bundler/macos/3rd-party-apps/`)
- New `debi pr auto-merge <enable|disable|reset|show> [--scope=repo|profile|global] [--json]` command to manage the default at any of the three scopes. `reset` is idempotent; `show` prints the resolved value plus each layer's contribution
- A mise command to build and open dev Devora in one go
- Dev builds set `macos_titlebar_color` to `#F5A97F` (Catppuccin Macchiato peach) so the dev `.app` titlebar is visually distinct from the release build
- Vim-style Kitty split panes: `cmd+shift+\` (vertical split, new shell to the right) and `cmd+shift+-` (horizontal split, new shell below); `cmd+shift+h/j/k/l` moves focus between panes, `cmd+shift+w` closes the focused pane; new panes inherit the active pane's working directory
- One-line `install.sh` installer: `curl -fsSL .../install.sh | bash` downloads the latest release DMG, replaces any existing `/Applications/Devora.app`, clears macOS quarantine, and installs the `debi` zsh completion. Pass `--nightly` to install the latest nightly build instead of the latest stable release
- Judge records permission requests for non-Bash tool types to `~/.claude/cc-judge-unhandled-requests.json` (separate from the existing Bash unsupported-cases file), so we can inspect real payloads and add support
- Judge now handles `WebFetch` permission requests: blocks `file://` URLs (directing to use the `Read` tool instead), defers `http(s)://` URLs to the user
- Judge now strips `timeout <duration>` prefix from commands before matching
- Judge now auto-approves `command -v`, `bash -n` (syntax checking), and `source .venv/bin/activate` commands
- glimpse-tty is now bundled as a vendored dependency (terminal web viewer binary)
- Judge now auto-allows reading crit plan and review files (`~/.crit/plans/`, `~/.crit/reviews/`)
- Bundler test suite and `test-bundler` mise task

### Changed

- Judge now abstains on `AskUserQuestion` and `Edit` permission requests (exits without a decision, deferring to Claude Code's normal permission flow)
- Updated crit to v0.10.4
- DMG filename simplified from `Devora_<version>.dmg` to `Devora.dmg` (the version is still embedded inside the app bundle's `Contents/Resources/VERSION` file)
- Consolidated Catppuccin Mocha palette into a single internal/style package; eliminates duplicated hex literals across the TUI fallback theme, pr check, pr submit, pr close, gst, gcl, and health
- New workspace-level functionality:
	- When run at the root of a workspace (instead of in a repo), `debi gst` and `debi gcl` now operate on all repos in the workspace in parallel instead of failing
	- `debi gst` now prints a per-repo summary (branch, file counts, commits behind origin/<default>, PR status)
	- `debi gcl` now applies `debi gcl` on every repo in the workspace (after verifying they are clean and detached)
- `-b, --blocked` on `debi pr submit` is now also a per-invocation override of the new `pr.auto-merge` config (behavior unchanged when the config is unset)
- Scheduled nightly builds are skipped when `master` has not advanced since the previous nightly
- `mise mac-install` now automatically runs `download-deps` first, so third-party bundler dependencies are fetched on demand
- Bootstrap and Kitty tab launchers now honor `$SHELL` (falling back to `/bin/zsh`) instead of hardcoding `zsh`
- User Guide tab (F1) launches `glow` directly instead of wrapping it in a login/interactive shell (faster startup)
- Main page tab and shell tab titles are now bracketed (`[Devora]`, `[Shell]`)
- Titlebar color is now `#181926` (Catppuccin Macchiato crust) instead of the system default, for a consistent look across macOS themes
- Judge now treats `time` and `watch` as irrelevant prefixes (stripped before matching), and recognizes `NODE_PATH=.` as a harmless env-var prefix
- Judge's Claude Code hook now matches all permission requests instead of only `Bash`, so non-Bash requests can be inspected and supported incrementally
- Judge now declines `node`, `bun`, and `shellcheck` commands (directing to use mise tasks)
- Judge now escalates non-syntax-checking `bash` invocations to the user instead of silently blocking them
- Claude Code now starts in plan mode by default (via `--permission-mode plan` in `ccc.sh`)
- Judge no longer intercepts `ExitPlanMode` permission requests (required for crit plan-exit integration)
- Updated 3rd-party dependencies:
	- UV v0.11.8 -> v0.11.10
	- Crit v0.10.4 -> v0.10.5
	- Glimpse-tty v2.0.9 -> v2.1.0

### Fixed

- Crit plan-hook integration broken with crit v0.10.4: port detection failed because v0.10.4 changed its stderr format from `on port N` to `http://localhost:N`
- Crit hook errors and window dismissals no longer silently auto-approve plan exits — the user is now prompted to confirm
- `download-deps.sh` now tracks dependency versions via `.dep-version` marker files, preventing stale artifacts when versions are bumped in `3rd-party-deps.json`
- Race condition in Judge's `save_unsupported_case` and `save_unhandled_request` — concurrent writes could corrupt the JSON files. Now uses `fcntl.flock` for exclusive locking
- Fixed a missing comma in Judge's `git tag -l` approval rule

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

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

- **Devora Ember** (experimental): a new variant of Devora that replaces Kitty and Glimpse-TTY with Tauri and xterm.js
  - Native desktop app built with Tauri (Rust + WebView) and xterm.js terminal emulator
  - Web-based workspace management panel with hybrid cards (compact, expand on hover), keyboard navigation (j/k, Enter, f to filter, 1/2/3 for active/inactive/all), and workspace creation
  - Overlay system: tab-covering overlays (workspace panel, User Guide) and panel overlays (Crit review UI tied to session tabs)
  - Session tabs with tab bar at the bottom, PTY-backed terminals, auto-close on process exit
  - Dynamic theme loading from `kitty-configs/current-theme.conf` via CSS custom properties
  - UI scaling via Ctrl+Shift++/-, Ctrl+=, Ctrl+1/2/3 — scales both terminal and UI
  - Crit integration via HTTP IPC: crit review opens as a panel overlay with submit/dismiss/crash detection
  - User Guide rendered as themed markdown overlay (F1)
  - Keyboard shortcuts: Ctrl+S / Shift+Shift (workspace panel), Ctrl+Shift+S (new tab), Ctrl+Left/Right (switch tabs), q/Esc (close overlays)
  - xterm.js addons: WebGL rendering, clickable URLs, OSC 52 clipboard, Unicode 11, search
  - Error logging to `/tmp/devora-ember-*.log` with panic hook for crash diagnostics
  - Build script (`bundler/macos-ember/bundle-ember.sh`) and mise tasks (`ember-dev`, `ember-build`, `ember-test`, `ember-bundle`, `ember-install`)
  - CI/CD: Ember tests and builds run alongside the Kitty variant in GitHub Actions
- Crit wrapper detects Ember mode (`DEVORA_EMBER=1`) and routes through HTTP IPC instead of Kitty/Glimpse-TTY
- **Devora Ember**: Acceptance testing framework with Gherkin scenarios and eval bridge
  - 19 acceptance scenarios covering themes, sessions, crit overlays, workspace panel, and Claude Code hook integration
  - Eval bridge: Rust HTTP server inside the Tauri app for driving WKWebView from Cucumber.js tests
  - Fake Claude API server with cassette record/replay for deterministic `@real-claude` tests
  - Interactive `ccc` mode: tests launch Claude Code the same way users do (no `--print` shortcuts)
  - Structured per-scenario API logging at `/tmp/devora-ember-test/` for debugging replay issues
  - UI interaction layer: `UIDriver`, `ws-panel-helper`, `fixture-helper` for DOM-level testing
  - Environment isolation: tests use an allowlisted env (HOME, SHELL, PATH, TMPDIR, USER, LANG) with PATH sanitized to strip inherited Devora paths
  - `pty.rs` detects `bundled-apps/` at runtime and adds it to PATH, making Ember self-sufficient
  - CI runs all scenarios including `@real-claude` on macOS 15
- **Devora Ember**: End-to-end Crit acceptance test (`@real-crit`) that exercises the full flow: workspace creation via UI, crit invocation, overlay verification, and review approval
- **Devora Ember**: postMessage eval bridge for cross-origin iframe content access in acceptance tests (injected via `initialization_script_for_all_frames`)

### Fixed

- **Devora Ember**: Fix Workspace Hub profile switching not loading workspace status (detail panel stayed on "Loading..." forever, status dots never populated)
- **Devora Ember**: Fixed bundling gaps in Devora Ember (compared to Devora OG)
- **Devora Ember**: `original-crit` was not bundled with Ember because `ember-bundle`/`ember-install` tasks didn't depend on `download-deps` and used soft `bundle` instead of `bundle_required`
- **Devora Ember**: `@real-claude` After hook called non-existent `isPanelOverlayActive()`, silently preventing overlay cleanup between scenarios

### Changed

- Standardized terminology across codebase and documentation: canonical glossary in CLAUDE.md now defines Workspace Hub, Profile, Worktree, Judge, Debi, CCC, CC Status Line, and Acceptance Test; aligned code and UI strings to match (IDLE badge → INACTIVE, WorkspacePanel → WorkspaceHub, "Add Repo" → "Register existing repo", tracker task disambiguation)
- Extended glossary with Tracker Task, Repo, Prepare Command, and Ember-specific terms (Terminal Pane, Tab Bar, Overlay); fixed remaining terminology leftovers from the initial standardization pass (MakeAndPrepareWorkTree casing, stale "workspace list" and "Add Repo" references in docs and comments)
- Merged "Task" concept into "Workspace" as a state: glossary now uses "Active Workspace" instead of "Task"; field labels changed from "Task Name" to "Title"; UI action labels ("New Task", "Start New Task") kept as user-friendly aliases
- **Devora Ember**: `mise ember-install` now installs the App with a versioned name for non-release commit builds
- **Devora Ember**: The app's window title bar now shows the current version of the app
- **Devora Ember**: Redesigned workspace panel from expandable cards to a master-detail split layout
  - Left panel: slim scrollable list of workspace names with status dots
  - Right panel: full detail for the focused workspace (title, repo table, open button)
  - Visual states for active, inactive, and invalid (error) workspaces
  - Responsive layout from phone to ultrawide (5 breakpoints)
  - Eliminates jarring card expand/collapse animations
- User Guide tab (F1) and View Changelog now use `glimpse-tty` instead of `glow` for rendering
- Always show the Kitty tab bar, even with a single tab
- Judge now recognizes any env-var prefix (`NAME=VALUE`) generically instead of requiring hardcoded entries
- Updated 3rd-party dependencies:
    - UV v0.11.10 → v0.11.11
    - Crit v0.10.5 → v0.11.0

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

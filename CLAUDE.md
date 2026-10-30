@README.md

You can read `./USER_GUIDE.md` for high-level understanding of the project.

## Language

**Devora**: The name of the application we develop. The published product is the Ember build (see below)
**Devora-Ember** / **Ember**: The Tauri + xterm.js build, which is published as "Devora"
	- It keeps the "Ember" name internally (the `project-ember/` directory, the `devora-ember` binary, some `ember-*` mise tasks)
**Devora Sapphire** / **Sapphire**: The native macOS build in progress (SwiftUI + libghostty over the shared Rust core, `project-sapphire/`); not published yet

Below are terms that have a specific meaning in the context of Devora:
- **Workspace**: A directory that houses one or more git worktrees; it is the root directory from which work is being done in Devora (could be active or inactive) and is where Claude Code is invoked. Workspaces that failed initialization appear as Invalid in the Workspace Hub
- **Active Workspace**: A workspace that has git repos checked out for active work (appears as "Active" in the UI). The UI uses "New Task" as a shorthand for workspace activation. Not to be confused with Tracker Tasks
- **Tracker Task**: A task in an external project management system (e.g., Asana) created by `debi pr submit`; distinct from an Active Workspace
- **Session**: An active workspace that is currently open in Devora. Each session appears as a tab in the UI
- **Repo**: A git repository registered under a profile, or auto-discovered under `<profile-root>/repos/`
- **Profile**: A named, isolated configuration scope with its own root directory, registered repos, and workspaces
- **Worktree**: A git worktree checkout inside a workspace; each registered repo gets its own worktree directory
- **Prepare Command**: A user-configured shell command that runs after worktree creation (config key: `prepare-command`)
- **Workspace Hub**: The UI for listing, filtering, creating, and managing workspaces; a tab-covering overlay
- **Settings Hub**: The Devora-Ember tab-covering overlay for managing profiles and editing settings — User Defaults, per-profile Claude models/effort, and registered repos. Reached from the Workspace Hub via `P`, the profile dropdown's "Manage Profiles…", or the burger menu's "Settings"
- **Terminal Pane** (Ember): An xterm.js terminal instance within a session
- **Tab Bar** (Ember): The strip at the bottom of the window showing open sessions
- **Judgess**: A Claude Code plugin that auto-manages permission requests to reduce permission fatigue (`./project-judgess/`)
- **Code-Cleanup**: A Claude Code plugin that defines and directs the post-task cleanup process (`./project-code-cleanup/`)
- **Debi**: Devora's CLI for workspace management and utilities. Being ported from Go (`./project-debi/`, bundled as `debi-go`) to Rust (`./project-core/crates/debi`, bundled as `debi`), which hands every not-yet-ported command to `debi-go`
- **CCC**: Customized Claude Code — the launcher that wraps Claude Code with Devora-specific configuration (`debi ccc`, also installed as a `ccc` command; built on `devora-agent` in `./project-core/`). Written as CCC when used as a name, `ccc` when referring to the shell command
- **CC Status Line**: A status line script for Claude Code that shows context-window usage and session cost (`./project-status-line/`)
- **Acceptance Test**: A test that verifies the functionality of Devora as a whole

## Development and Testing

Dependencies like Python and Go are managed using Mise.
To run Go/Python commands, use `mise exec -- go ...` or `mise exec -- python ...` to ensure the correct environment is used.
Repeating tasks should be added to `mise.toml` for easier execution.

Inspect the root `mise.toml` (or the project-specific one) for available commands and tasks.

## Architecture Decision Records

ADRs document significant architectural decisions.
They are stored in `docs/adrs/` and listed below.

When making a new architectural decision, create a new ADR file following the format of existing ones and add it to this list.

An ADR records the *decision and its history*.
Knowledge that describes *current* behavior belongs where it is implemented — on the type or module that owns it, or, when it spans several files of one subsystem, in a `CLAUDE.md` at that subsystem's directory root (e.g. `project-ember/src/ui/layers/CLAUDE.md`).
Reference those homes rather than restating them.

- [ADR-001: Ember Acceptance Testing Framework](docs/adrs/ADR-001-ember-bdd-testing-framework.md) — Eval bridge + Gherkin acceptance tests + Fake Claude API for testing the Tauri app
- [ADR-002: Ember Centralized Error Reporting](docs/adrs/ADR-002-ember-centralized-error-reporting.md) — Single showError/report_error path, invoke wrapper with surfaced-by-default errors, ACL completeness test
- [ADR-003: Ember Layer System (Stacked Surfaces)](docs/adrs/ADR-003-ember-layer-system.md) — Surfaces routed and focused by stack position, with extent from which stack (one per tab, plus one for the window); shortcuts routed before any surface; paint order from DOM order. Current behavior is documented in [`project-ember/src/ui/layers/CLAUDE.md`](project-ember/src/ui/layers/CLAUDE.md)
- [ADR-004: Ember Vim-Inspired Navigation](docs/adrs/ADR-004-ember-vim-inspired-navigation.md) — Vim/Neovim as the explicit keyboard-navigation model; reusable primitives (`createVimScroll` for viewport scroll: j/k, Ctrl+D/U, gg/G; `createListCursor` for list selection)
- [ADR-005: Judgess](docs/adrs/ADR-005-judgess.md) — Rust rewrite of the permission hook: a real shell parser, a four-way verdict collapsed to three on the wire, and a raw → labelled → redacted-cases pipeline
- [ADR-006: Two Builds Side by Side](docs/adrs/ADR-006-two-builds-side-by-side.md) — Ember stays the general Mac/Linux build (shipped as `Devora Ember.app`); Sapphire is a native macOS-only build (`Devora Sapphire.app`); features land in the core first and Ember opts in
- [ADR-007: Shared Rust Core and Debi in Rust](docs/adrs/ADR-007-shared-rust-core.md) — `project-core/` owns every non-UI behavior behind one `DevoraCore` facade (commands in, events out); Debi is ported to Rust by command group, starting with `ccc` as `debi ccc`
- [ADR-008: Core-Owned App State and Durability](docs/adrs/ADR-008-core-owned-state-and-durability.md) — the core owns tabs, splits, panes, drafts and agents; every mutation is a named command written through to SQLite; conversation history lives in a separate `conversations.db`
- [ADR-009: Agent Runtime](docs/adrs/ADR-009-agent-runtime.md) — one persistent `claude -p` stream-json process per agent session, driven over the control protocol, with Judgess linked in-process as a `PreToolUse` hook callback
- [ADR-010: Sapphire UI Stack and Build](docs/adrs/ADR-010-sapphire-ui-stack.md) — SwiftUI on macOS 27 over UniFFI, libghostty terminal, natively drawn `nvim --embed` prompt editor, `WKWebView` panes, SwiftPM modules plus a generated app target

## Vision

When planning new features or changes, refer to the relevant vision documents to ensure alignment with the overall goals of the project.
Vision documents are stored at @docs/vision/

## Pull Request Guidelines

When submitting a PR, update the `## Unreleased` section in `CHANGELOG.md` with a brief description of the change.
Add plain bullet points under the right type/category heading of Unreleased.
Add a type/category sub-headings like Added/Changed/Fixed if it doesn't already exist.

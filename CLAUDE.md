@README.md

When reading/updating docs, make sure to also check `./USER_GUIDE.md` as well.

## Language

**Devora**: The name of the application we develop
**Devora OG**: The current version of Devora that is based on Kitty and Glimpse-TTY
**Devora Ember**: The next-gen version of Devora that is based on Tauri and xterm.js

Below are terms that have a specific meaning in the context of Devora:
- **Workspace**: A directory that houses one or more git worktrees; it is the root directory from which work is being done in Devora (could be active or inactive) and is where Claude Code is invoked. Workspaces that failed initialization appear as Invalid in the Workspace Hub
- **Active Workspace**: A workspace that has git repos checked out for active work (appears as "Active" in the UI). The UI uses "New Task" as a shorthand for workspace activation. Not to be confused with Tracker Tasks
- **Tracker Task**: A task in an external project management system (e.g., Asana) created by `debi pr submit`; distinct from an Active Workspace
- **Session**: An active workspace that is currently open in Devora. Each session appears as a tab in the UI
- **Repo**: A git repository registered under a profile, or auto-discovered under `<profile-root>/repos/`
- **Profile**: A named, isolated configuration scope with its own root directory, registered repos, and workspaces
- **Worktree**: A git worktree checkout inside a workspace; each registered repo gets its own worktree directory
- **Prepare Command**: A user-configured shell command that runs after worktree creation (config key: `prepare-command`)
- **Workspace Hub**: The UI for listing, filtering, creating, and managing workspaces. In Devora OG this is a full-screen TUI; in Devora Ember this is a tab-covering overlay
- **Terminal Pane** (Ember): An xterm.js terminal instance within a session
- **Tab Bar** (Ember): The strip at the bottom of the window showing open sessions
- **Overlay** (Ember): A UI layer rendered on top of main content. Two types: Panel Overlay (covers main panel area, tied to a session) and Tab-Covering Overlay (covers entire window including tab bar)
- **Judge**: A Claude Code plugin that auto-manages permission requests to reduce permission fatigue (`./project-judge/`)
- **Debi**: Devora's CLI for workspace management and utilities (`./project-debi/`)
- **CCC**: Customized Claude Code — a launcher script that wraps Claude Code with Devora-specific configuration (`./ccc.sh`). Written as CCC when used as a name, `ccc` when referring to the shell command
- **CC Status Line**: A status line script for Claude Code that shows context-window usage and session cost (`./project-status-line/`)
- **Acceptance Test**: A test that verifies the functionality of Devora as a whole

## Development and Testing

Dependencies like Python and Go are managed using Mise.
To run Go/Python commands, use `mise exec -- go ...` or `mise exec -- python ...` to ensure the correct environment is used.
Repeating tasks should be added to `mise.toml` for easier execution.

Inspect the root `mise.toml` (or the project-specific one) for available commands and tasks.

## Architecture Decision Records

ADRs document significant architectural decisions. They are stored in `docs/adrs/` and listed below.

When making a new architectural decision, create a new ADR file following the format of existing ones and add it to this list.

- [ADR-001: Ember Acceptance Testing Framework](docs/adrs/ADR-001-ember-bdd-testing-framework.md) — Eval bridge + Gherkin acceptance tests + Fake Claude API for testing the Tauri app

## Pull Request Guidelines

When submitting a PR, update the `## Unreleased` section in `CHANGELOG.md` with a brief description of the change.
Add plain bullet points under the right type/category heading of Unreleased.
Add a type/category sub-headings like Added/Changed/Fixed if it doesn't already exist.

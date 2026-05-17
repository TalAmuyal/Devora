@README.md

When reading/updating docs, make sure to also check `./USER_GUIDE.md` as well.

## Language

**Devora**: The name of of the application we develop
**Devora OG**: The current version of Devora that is based on Kitty and Glimpse-TTY
**Devora Ember**: The next-gen version of Devora that is based on Tauri and xterm.js

Below are terms that has a sepcific meaning in the context of Devora:
- **Workspace**: A directory that houses one or more git worktrees; It is the root directory from which work is being done in Devora (could be active or inactive) and is where Claude Code is invoked
- **Task**: An active workspace in the sense that it has git repos checked out for work on a specific task (appears as "Active" in the UI)
- **Session**: An active Task in the sense that the workspace exists, is ready for work, and is actively open in Devora as a tab to work with (appears as a tab in the UI)
- **Acceptance Test**: A test that verifies the functionality of the app (Devora) as a whole and ensures that it meets the requirements and expectations of the users.

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

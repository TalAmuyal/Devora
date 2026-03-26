# Documentation Index

## Reading Order

Start with [architecture.md](architecture.md) for the high-level picture, then read individual specs as needed.

## Architecture

| Document | Description |
|----------|-------------|
| [architecture.md](architecture.md) | Glossary, package dependency map, end-to-end flows |

## Specs (normative)

Specifications for the domain packages. These define the intended behavior of each package.

| Document | Package | Description |
|----------|---------|-------------|
| [specs/config.md](specs/config.md) | `internal/config` | Configuration loading, profiles, paths |
| [specs/workspace.md](specs/workspace.md) | `internal/workspace` | Workspace creation, worktrees, locking |
| [specs/terminal.md](specs/terminal.md) | `internal/terminal` | Kitty terminal session management |
| [specs/cli.md](specs/cli.md) | `internal/cli` | CLI entry point and subcommands |
| [specs/process.md](specs/process.md) | `internal/process` | Shell command execution |
| [specs/task.md](specs/task.md) | `internal/task` | Task JSON read/write |
| [specs/crash.md](specs/crash.md) | `internal/crash` | Crash logging |
| [specs/tui.md](specs/tui.md) | `internal/tui` | TUI pages, state machine, components |
| [specs/tui-navigation.md](specs/tui-navigation.md) | `internal/tui` | Navigation key bindings: back, quit, exit behavior across all pages |
| [specs/tui-operations.md](specs/tui-operations.md) | `internal/tui` | Business logic: workspace gathering, creation, deletion rules, add-repo |
| [specs/tui-theme.md](specs/tui-theme.md) | `internal/tui` | Theme loading, palette, color mapping, styles |
| [specs/tui-components.md](specs/tui-components.md) | `internal/tui/components` | Reusable UI widgets: list, checkbox, text input, vim navigation, footer |

## Spec Conventions

- Specs document the **exported API** and behavioral contracts for each package.
- Testing sections are included only for **critical test cases** (complex concurrency, tricky edge cases). Routine test expectations are not listed — tests speak for themselves in the code.
- Specs are **normative**: they define intended behavior. Known gaps between spec and implementation are annotated with a link to [BACKLOG.md](BACKLOG.md).

## Backlog

| Document | Description |
|----------|-------------|
| [BACKLOG.md](BACKLOG.md) | UX improvements to be addressed |

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
| [specs/health.md](specs/health.md) | `internal/health` | Dependency health checking (includes tracker credential row) |
| [specs/jsonvalidate.md](specs/jsonvalidate.md) | `internal/jsonvalidate` | JSON validation with line:column error positions |
| [specs/yamlvalidate.md](specs/yamlvalidate.md) | `internal/yamlvalidate` | YAML validation (multi-document) with line:column error positions |
| [specs/tomlvalidate.md](specs/tomlvalidate.md) | `internal/tomlvalidate` | TOML validation with line:column error positions |
| [specs/git.md](specs/git.md) | `internal/git` | Git shortcut commands and submit/close helpers |
| [specs/prstatus.md](specs/prstatus.md) | `internal/prstatus` | PR status checking via gh CLI |
| [specs/submit.md](specs/submit.md) | `internal/submit` | `debi pr submit`: commit, tracker task, PR creation |
| [specs/close.md](specs/close.md) | `internal/close` (pkg `closecmd`) | `debi pr close`: task completion, branch cleanup |
| [specs/tasktracker.md](specs/tasktracker.md) | `internal/tasktracker` | Pluggable issue tracker interface and registry |
| [specs/credentials.md](specs/credentials.md) | `internal/credentials` | OS keychain token lookup |
| [specs/gh.md](specs/gh.md) | `internal/gh` | GitHub CLI (`gh`) wrapper for submit/close |

## Spec Conventions

- Specs document the **exported API** and behavioral contracts for each package.
- Testing sections are included only for **critical test cases** (complex concurrency, tricky edge cases). Routine test expectations are not listed — tests speak for themselves in the code.
- Specs are **normative**: they define intended behavior.

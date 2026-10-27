# Documentation Index

## Reading Order

Start with [architecture.md](architecture.md) for the high-level picture, then read individual specs as needed.

## Architecture

| Document | Description |
|----------|-------------|
| [architecture.md](architecture.md) | Glossary, package dependency map, end-to-end flows |

## Specs (legacy, being migrated)

These specs are a legacy reference.
We no longer write new specs or update them when a package's API changes; instead, the "why" lives in the code itself.
Existing specs are being gradually migrated into the code, and each spec is trimmed as the code absorbs its content.
Treat the code as the source of truth, and consult a spec only for context the code has not yet absorbed.

| Document | Package | Description |
|----------|---------|-------------|
| [specs/workspace.md](specs/workspace.md) | `internal/workspace` | Workspace creation, worktrees, locking |
| [specs/jsonvalidate.md](specs/jsonvalidate.md) | `internal/jsonvalidate` | JSON validation with line:column error positions |
| [specs/yamlvalidate.md](specs/yamlvalidate.md) | `internal/yamlvalidate` | YAML validation (multi-document) with line:column error positions |
| [specs/tomlvalidate.md](specs/tomlvalidate.md) | `internal/tomlvalidate` | TOML validation with line:column error positions |
| [specs/prstatus.md](specs/prstatus.md) | `internal/prstatus` | PR status checking via gh CLI |
| [specs/submit.md](specs/submit.md) | `internal/submit` | `debi pr submit`: commit, tracker task, PR creation |
| [specs/close.md](specs/close.md) | `internal/close` (pkg `closecmd`) | `debi pr close`: task completion, branch cleanup |
| [specs/tasktracker.md](specs/tasktracker.md) | `internal/tasktracker` | Pluggable issue tracker interface and registry |
| [specs/gh.md](specs/gh.md) | `internal/gh` | GitHub CLI (`gh`) wrapper for submit/close |

## Spec Conventions

These conventions describe the legacy specs as they were written; they are not a guide for new work (see "Specs (legacy, being migrated)" above).

- Specs document the **exported API** and behavioral contracts for each package.
- Testing sections are included only for **critical test cases** (complex concurrency, tricky edge cases). Routine test expectations are not listed — tests speak for themselves in the code.

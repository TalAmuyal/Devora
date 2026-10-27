# Architecture

## Glossary

| Term | Definition |
|------|-----------|
| **Profile** | A named configuration scope pointing to a root directory that contains workspaces. Each profile has its own set of registered repos and settings. Profiles are stored in `~/.config/devora/config.json`. |
| **Workspace** | An isolated working environment stored as a `ws-N` directory under the profile's workspaces root. Contains git worktrees, a `task.json` file, and an `initialized` marker. |
| **Worktree** | A git worktree checkout inside a workspace. Each registered repo gets its own worktree directory, created in detached HEAD state from the repo's default branch. |
| **Active Workspace** | A workspace activated for work. Represented as `task.json` with a UUID, title, and start date. A workspace without a `task.json` is considered inactive. |

## Package Dependency Map

```
main.go
  ├── cli        (command dispatch)
  └── crash      (panic/error logging)

cli
  ├── cmdinfo    (shared command metadata types)
  ├── close      (pkg closecmd: debi pr close command)
  ├── completion (shell completion script generation)
  ├── config     (profiles, repos, settings)
  ├── credentials (OS keychain lookup; for submit/close error translation)
  ├── gitcmd     (debi git dispatch: customs, passthrough, fan-out)
  ├── git        (git compositions and plumbing)
  ├── jsonvalidate
  ├── process    (shell command execution)
  ├── prstatus   (PR status checking)
  ├── submit     (debi pr submit command)
  ├── tasktracker/asana (blank import; registers "asana" provider via init)
  ├── tomlvalidate
  ├── workspace  (workspace detection from CWD)
  └── yamlvalidate

workspace
  ├── config     (profiles and workspaces root, for CWD detection)
  └── process    (git status)

git
  └── process    (command execution)

completion
  └── cmdinfo    (command metadata)

prstatus
  └── process    (gh CLI execution)

health
  ├── process    (exit-code signalling)
  ├── config     (config file path, task-tracker provider)
  ├── credentials (task-tracker token lookup)
  └── version    (app version string)

submit
  ├── config       (branch prefix)
  ├── gh           (GitHub CLI wrapper)
  ├── git          (commit, branch, push, config helpers)
  ├── process      (default browser opener)
  ├── tasktracker  (pluggable issue tracker interface)
  ├── lipgloss     (styled progress output)
  └── errgroup     (concurrent pre-fetch)

close (pkg closecmd)
  ├── gh           (GitHub CLI wrapper: gh pr view)
  ├── git          (branch, config, fetch, checkout, delete helpers)
  ├── tasktracker  (pluggable issue tracker interface)
  ├── lipgloss     (styled progress output)
  └── errgroup     (concurrent task completion + remote branch deletion)

tasktracker
  └── config     (reads task-tracker.provider)

tasktracker/asana
  ├── config       (reads task-tracker.asana.*)
  ├── credentials  (token lookup, lazy)
  └── tasktracker  (Register + Tracker interface)

gh
  └── process    (gh CLI execution)

credentials
  └── zalando/go-keyring (sole OS keychain backend)

tomlvalidate
  └── BurntSushi/toml (TOML parser with typed ParseError positions)

yamlvalidate
  └── goccy/go-yaml (YAML parser with typed errors carrying token positions)

config, process, crash, cmdinfo, version
  └── (stdlib only)
```

No circular dependencies. Data flows downward: `cli` orchestrates top-level flow, and domain packages (`workspace`, `config`, `process`) are independent of each other (except `workspace` depending on `config` and `process`).

## CLI Commands

The binary is named `debi`. The command registry lives in `internal/cli/commands.go`; `debi --help` (and `debi <command> --help`) prints the commands and their arguments.

## Workspace States

For the state decision tree and conditions, see [workspace.md](specs/workspace.md#workspace-states).

## PR Submit/Close Flow

`debi pr submit` and `debi pr close` form a two-ended feature workflow around a detached-HEAD model.

```
detached HEAD on origin/<default>
         │
         ▼
   ┌──────────────────┐
   │  debi pr submit  │  1. Guard: HEAD must be detached
   │   -m "..."       │  2. git add . && git commit -m "<message>"
   │                  │  3. (optional) tracker.CreateTask via tasktracker
   │                  │  4. git checkout -b <prefix>-<slug>; set branch.<name>.task-id
   │                  │  5. git push --set-upstream
   │                  │  6. gh pr create --assignee @me [--draft]
   │                  │  7. (per pr.auto-merge per-repo/profile/global config + flags) gh pr merge --auto --squash
   └──────┬───────────┘
          ▼
  feature branch with open PR
          │
          ▼
  (review / merge on GitHub)
          │
          ▼
   ┌──────────────────┐
   │   debi pr close  │  1. Resolve tracker + task-id (from --task-url or branch config)
   │                  │  2. Unless --force, prompt when PR is still OPEN
   │                  │  3. Parallel (best-effort): CompleteTask + delete remote branch
   │                  │  4. git fetch origin; git checkout origin/<default>
   │                  │  5. git branch -D <branch> (non-fatal)
   └──────┬───────────┘
          ▼
detached HEAD on origin/<default>
```

`internal/submit` and `internal/close` (pkg `closecmd`) each expose a single `Run(io.Writer, Options)` entry point and a set of domain-specific sentinel errors. The CLI layer (`internal/cli`) translates those sentinels into `*cli.UsageError` or `*process.PassthroughError` as described in the command specs. Credentials for the task tracker are fetched lazily from the OS keychain via `internal/credentials`; `internal/gh` centralizes all `gh` CLI invocations used by submit and close.

For per-command details see [specs/submit.md](specs/submit.md) and [specs/close.md](specs/close.md). For the pluggable tracker interface see [specs/tasktracker.md](specs/tasktracker.md).

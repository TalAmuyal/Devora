# Architecture

## Glossary

| Term | Definition |
|------|-----------|
| **Profile** | A named configuration scope pointing to a root directory that contains workspaces. Each profile has its own set of registered repos and settings. Profiles are stored in `~/.config/devora/config.json`. |
| **Workspace** | An isolated working environment stored as a `ws-N` directory under the profile's workspaces root. Contains git worktrees, a `task.json` file, and an `initialized` marker. |
| **Worktree** | A git worktree checkout inside a workspace. Each registered repo gets its own worktree directory, created in detached HEAD state from the repo's default branch. |
| **Active Workspace** | A workspace activated for work. Represented as `task.json` with a UUID, title, and start date. A workspace without a `task.json` is considered inactive. |

## CLI Commands

The binary is named `debi`.
The command registry lives in `internal/cli/commands.go`; `debi --help` (and `debi <command> --help`) prints the commands and their arguments.

## Workspace States

See the `internal/workspace` package doc for which workspace states debi distinguishes.

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

`internal/submit` and `internal/close` (pkg `closecmd`) each expose a single `Run(io.Writer, Options)` entry point and a set of domain-specific sentinel errors.
The CLI layer (`internal/cli`) translates those sentinels into `*cli.UsageError` or `*process.PassthroughError` (see `runSubmit` and `runClose` in `internal/cli/cli.go`).
Credentials for the task tracker are fetched lazily from the OS keychain via `internal/credentials`; `internal/gh` centralizes all `gh` CLI invocations used by submit and close.

For per-command details see the `internal/submit` and `internal/close` package docs.
For the pluggable tracker interface see the `internal/tasktracker` package doc.

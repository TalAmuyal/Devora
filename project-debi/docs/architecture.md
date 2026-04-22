# Architecture

## Glossary

| Term | Definition |
|------|-----------|
| **Profile** | A named configuration scope pointing to a root directory that contains workspaces. Each profile has its own set of registered repos and settings. Profiles are stored in `~/.config/devora/config.json`. |
| **Workspace** | An isolated working environment stored as a `ws-N` directory under the profile's workspaces root. Contains git worktrees, a `task.json` file, and an `initialized` marker. |
| **Worktree** | A git worktree checkout inside a workspace. Each registered repo gets its own worktree directory, created in detached HEAD state from the repo's default branch. |
| **Task** | A named unit of work associated with a workspace. Represented as `task.json` with a UUID, title, and start date. A workspace without a task is considered inactive. |
| **Session** | A Kitty terminal tab associated with a workspace's working directory. Sessions are identified by Kitty tab ID and matched to workspaces by comparing the tab's CWD to the workspace path. |
| **Backend** | The terminal integration layer. Currently Kitty-only, using `kitty @` remote control commands. The `Backend` interface exists for testability. |

## Package Dependency Map

```
main.go
  ├── cli        (command dispatch)
  └── crash      (panic/error logging)

cli
  ├── cmdinfo    (shared command metadata types)
  ├── close      (pkg closecmd: debi close command)
  ├── completion (shell completion script generation)
  ├── config     (profiles, repos, settings)
  ├── credentials (OS keychain lookup; for submit/close error translation)
  ├── git        (git shortcut commands)
  ├── jsonvalidate
  ├── process    (shell command execution)
  ├── prstatus   (PR status checking)
  ├── submit     (debi submit command)
  ├── tasktracker/asana (blank import; registers "asana" provider via init)
  ├── task       (task file read/write)
  ├── tui        (UI entry points)
  └── workspace  (workspace detection from CWD)

tui
  ├── config     (active profile, settings, repos)
  ├── task       (task file creation)
  ├── terminal   (session listing, creation, attachment)
  ├── workspace  (workspace CRUD, git operations)
  ├── tui/components  (reusable UI widgets)
  ├── bubbletea  (Elm architecture framework)
  └── lipgloss   (styling)

workspace
  ├── config     (workspaces root path, prepare command)
  ├── git        (default branch name utility)
  ├── process    (git commands)
  └── errgroup   (parallel worktree removal)

terminal
  └── process    (kitty @ commands)

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

config, process, task, crash, cmdinfo, version
  └── (stdlib only)

tui/components
  └── lipgloss   (styling)
```

No circular dependencies. Data flows downward: `cli` orchestrates top-level flow, `tui` handles interactive UI, domain packages (`workspace`, `terminal`, `config`, `task`) are independent of each other (except `workspace` depending on `config` and `process`).

## CLI Commands

The binary is named `debi`. For the command table with arguments, see [cli.md](specs/cli.md#commands).

## Workspace Creation Flow

This is the primary end-to-end flow, spanning `cli` → `tui` → `workspace` → `terminal`.

```
User runs: debi workspace-ui
         │
         ▼
   ┌─────────────┐
   │  cli.Run()  │  Load profiles, set active profile, get repo names
   └──────┬──────┘
          ▼
   ┌──────────────────┐
   │ tui.RunWorkspaceUI│  Bubble Tea program starts
   └──────┬───────────┘
          ▼
   ┌──────────────────┐
   │ PageWorkspaceList │  User sees workspace cards
   │   user presses n  │
   └──────┬───────────┘
          ▼
   ┌──────────────────┐
   │   PageNewTask     │  User selects repos, enters task name, submits
   └──────┬───────────┘
          ▼
   ┌──────────────────┐
   │  PageCreation     │  Shows progress while background work runs:
   │                   │  1. Search for reusable inactive workspace
   │                   │  2. Create ws-N directory (if needed)
   │                   │  3. Write CLAUDE.md (multi-repo only)
   │                   │  4. Create git worktrees (parallel)
   │                   │  5. Write task.json
   └──────┬───────────┘
          ▼
   TUI exits with WorkspaceReadyResult
          │
          ▼
   ┌──────────────────────────────┐
   │ tui.CreateAndAttachSession() │  Post-TUI, called by CLI:
   │                              │  1. Create Kitty backend
   │                              │  2. Try to attach to existing session
   │                              │  3. If none, launch new Kitty tab
   │                              │  4. Poll until session appears
   └──────────────────────────────┘
```

## Add Repo Flow

Runs as a standalone TUI, separate from the main app.

```
User runs: debi add  (from inside a workspace)
         │
         ▼
   ┌─────────────┐
   │  cli.Run()  │  Detect workspace from CWD, load profile
   └──────┬──────┘
          ▼
   ┌──────────────────┐
   │  tui.RunAddRepo   │  Standalone Bubble Tea program
   │                   │  User selects repo, optional postfix
   │                   │  Creates worktree, updates CLAUDE.md
   │                   │  Exits on completion
   └──────────────────┘
```

## Workspace States

For the state decision tree and conditions, see [workspace.md](specs/workspace.md#workspace-states). The TUI's workspace list assigns categories based on these states and matches sessions by comparing workspace paths against Kitty tab CWDs.

## PR Submit/Close Flow

`debi submit` and `debi close` form a two-ended feature workflow around a detached-HEAD model. The CLI registers them both as top-level commands and as `pr submit` / `pr close` subcommands; flag slices are declared once and shared.

```
detached HEAD on origin/<default>
         │
         ▼
   ┌──────────────────┐
   │   debi submit    │  1. Guard: HEAD must be detached
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
   │    debi close    │  1. Guard: on a non-protected branch
   │                  │  2. Resolve tracker + task-id (from --task-url or branch config)
   │                  │  3. Unless --force, prompt when PR is still OPEN
   │                  │  4. Parallel (best-effort): CompleteTask + delete remote branch
   │                  │  5. git fetch origin; git checkout origin/<default>
   │                  │  6. git branch -D <branch> (non-fatal)
   └──────┬───────────┘
          ▼
detached HEAD on origin/<default>
```

`internal/submit` and `internal/close` (pkg `closecmd`) each expose a single `Run(io.Writer, Options)` entry point and a set of domain-specific sentinel errors. The CLI layer (`internal/cli`) translates those sentinels into `*cli.UsageError` or `*process.PassthroughError` as described in the command specs. Credentials for the task tracker are fetched lazily from the OS keychain via `internal/credentials`; `internal/gh` centralizes all `gh` CLI invocations used by submit and close.

For per-command details see [specs/submit.md](specs/submit.md) and [specs/close.md](specs/close.md). For the pluggable tracker interface see [specs/tasktracker.md](specs/tasktracker.md).

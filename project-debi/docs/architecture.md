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
  ├── config     (profiles, repos, settings)
  ├── process    (shell command execution)
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
  ├── process    (git commands)
  └── errgroup   (parallel worktree removal)

terminal
  └── process    (kitty @ commands)

config, process, task, crash
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

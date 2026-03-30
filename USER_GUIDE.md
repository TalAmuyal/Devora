# User Guide

## Initial Setup

1. Install [Claude Code](https://code.claude.com/docs/en/quickstart#step-1-install-claude-code)
2. Go over this user guide to understand the core concepts and workflow
	- Setup anything in [Optional Additions](#optional-additions) that catches your eye
3. Start Devora and create your first profile

### MacOS

For the first usage, you might need to:
1. "Right-click > Open" the app
2. Press "Done" on the warning dialog
3. Open the "security & privacy" settings
4. Allow the app to run (on the lower part of the page)

## Introduction

Devora organizes your work around **profiles**, **repos**, and **workspaces**.

A **profile** is a named, isolated configuration scope.
Each profile has its own root directory, its own set of registered git repositories, and its own workspaces.
Profiles provide complete isolation between different contexts; for example, you might have one profile for work projects and another for personal ones.

A **repo** is a git repository registered under a profile, or located under `<profile-root>/repos/`.
When you create a workspace, you select one or more repos from the active profile's repo list.

A **workspace** is a set of git worktrees (one per selected repo) tied to a task.
Devora creates the worktrees, opens a terminal session, and you are ready to go.

### First Launch

When you start Devora for the first time, it will prompt you to create your first profile.
You choose a root directory (defaults to `~/devora`) and give the profile a name.
Once the profile is created, you land on the workspace list - Devora's home screen.

### Profiles

Each profile is a self-contained directory with the following structure:

```
<profile-root>/
├─ config.json
├─ repos/
│  └─ ...
└─ workspaces/
   └─ ...
```

- `config.json` holds profile-level settings (like the prepare command and additionally registered repos)
- `repos/` is where auto-discovered repositories live
- `workspaces/` contains the worktrees Devora creates for your tasks

You can have multiple profiles.
To cycle between them, press `p` from the workspace list.
To create additional profiles, go to Settings (press `s` from the workspace list) and navigate to "Profiles".

**Note**: Deleting a profile (also via Settings) only removes it from Devora's registry. The profile directory and all its contents remain on disk.

### Managing Repos

Each profile maintains its own repo directory and list of additional registered git repositories.
There are two ways to add repos to a profile:

1. **Auto-discovered repos**: Clone a git repository directly into `<profile-root>/repos/`
	- Devora automatically picks it up - no extra registration needed
2. **Explicitly registered repos**: Register repos at arbitrary paths via Settings (press `s`, then navigate to Repos and select "Add Repo")

Removing a registered repo only removes Devora's reference to it.
The actual directory on disk is not affected.

**Note**: Only explicitly registered repos can be removed from the Settings page. To stop seeing an auto-discovered repo, move or delete it from `<profile-root>/repos/`.

### Workspaces

From the workspace list:

1. Press `n` to start a new task
2. Select one or more repos from the active profile's repo list
3. Enter a short task name (shorter is better, e.g: "login bug", "refactor auth", "<project name initials>")
4. Devora creates a git worktree from each selected repo and opens a terminal session in the workspace directory
	- If a "prepare command" is set in the profile's settings, it runs in each worktree as part of its setup (e.g., to install dependencies)
	- An automated `CLAUDE.md` is generated in the workspace root, where you can supplement Claude with task related context and why each repo was included in the workspace
5. Run the "ccc" command to launch the Customized Claude Code CLI in the workspace context
	- Now, Claude Code is ready with all of the Devora features and has access to all of the repos in the workspace

Once a workspace exists, you can manage it from the workspace list:

- `d` deactivates a workspace (cleans it up and keeps it on disk as cache for future use). Only available for active workspaces
- `D` deletes a workspace permanently (removes its worktrees and root directory). Only available for inactive or invalid workspaces (you must deactivate first)

Both actions are blocked if the workspace has an active terminal session, uncommitted changes, or worktrees that are still on a branch (not yet detached).

### Settings

Press `s` from the workspace list to open the settings page.

Global settings include:

- **Profiles**: Add new profiles or delete existing ones

Profile-specific settings (for the current profile) include:

- Setting a prepare command: a shell command that runs after every worktree creation (e.g., `npm install`, `mise install`, custom Bash script)
- **Repos**: Register or unregister repos at arbitrary paths (in addition to auto-discovered repos under `<profile-root>/repos/`)

Info section:

- **View Changelog**: Opens the full changelog in a new kitty tab using glow

## Optional Additions

### Simple Status Line

A Devora-provided status line is available to show the current context-window usage and session cost.

To enable it, copy `cc-simple-statusline` to your `~/.claude/` directory and add the following to your `~/.claude/settings.json`:

```json
{
  "statusLine": {
    "type": "command",
    "command": "~/.claude/cc-simple-statusline",
    "padding": 0
  }
}
```

### Mise en place

[Mise](https://mise.jdx.dev) is a great tool to manage project-specific setups in a very automated way.
It can usually replace:
- System package managers (`apt`, `dnf`, `brew`, `nix`, etc.)
- Programming-language version managers (`nvm`, `pyenv`, `rbenv`, `asdf`, etc.)
- [Task](https://mise.jdx.dev/tasks) management tools (like common non-build-system `make` usages)

It can also automate:
- Programming-language package-managements (`npm`, `poetry`, `uv`, `gem`, `cargo`, etc.) with its [prepare](https://mise.jdx.dev/dev-tools/prepare.html) feature
- Setting up env-vars

## Versioning

Devora follows a date-based version format: `YYYY-MM-DD.PATCH` (e.g., `2026-03-28.0`).

Development builds between releases show as `YYYY-MM-DD.PATCH-dev.N` where N is the number of commits since the last release.

The current version is displayed in the bottom-right corner of the workspace manager.

To view the full changelog, go to Settings and select "View Changelog".

## Cheatsheet

### Devora-wide

- `ctrl + s`: Open workspace manager in a new tab
- `ctrl + shift + s`: Open shell in a new tab
- `ctrl + 1 / 2 / 3`: Set test size to small / medium / large (respectively)
- `F1`: Open this User Guide

### Workspace List

- `<enter>`: Open selected workspace (closes the current UI and opens a terminal session)
- `n`: New task (reuses or creates a workspace on opens it)
- `d`: Deactivate workspace (active only)
- `D`: Delete workspace (inactive/invalid only)
- `r`: Refresh workspace list
- `1 / 2 / 3`: Filter workspaces by "Active" (default) / "Inactive" / "All"
- `j / k`: Navigate list
- `s`: Open settings (includes "View Changelog" option)
- `p`: Cycle active profile
- `q`: Quit (or back in other pages)
- `esc`: Same as `q`, and also "unfocuses" on text inputs

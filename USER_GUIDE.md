# User Guide

## Initial Setup

1. Install [Claude Code](https://code.claude.com/docs/en/quickstart#step-1-install-claude-code)
2. Go over this user guide to understand the core concepts and workflow
	- Setup anything in [Optional Additions](#optional-additions) that catches your eye
3. Run `debi health` to verify that all required dependencies are installed (use `--strict` to also check optional ones)
4. Start Devora and create your first profile

### MacOS

If you installed via the curl-based installer (see README), the quarantine attribute is cleared automatically and you can skip these steps.

Otherwise, for the first usage, you might need to work around macOS Gatekeeper (see also [Troubleshooting](#troubleshooting)):
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

Workspaces can span multiple repositories simultaneously.
Devora manages a separate git worktree for each selected repo, so you can work across repos within a single workspace without affecting other tasks.

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

#### Multi-repo git shortcuts at the workspace root

Two of the `debi` git shortcuts are workspace-aware when run from the exact root of a workspace directory (`ws-N/`):

- `debi gst` prints an aligned per-repo summary across every repo in the workspace: branch (or `HEAD` if detached), staged/unstaged/untracked file counts, commits behind `origin/<default>`, and PR state. All repos are queried in parallel and a SUMMARY block reports the totals
- `debi gcl` first verifies that every repo is clean and on detached HEAD; if any repo fails, the command aborts and no git operations run. Otherwise it fetches and checks out `origin/<default>` in every repo in parallel and reports per-repo success or failure

Inside any specific repo (including a repo that lives inside a workspace), both commands work exactly as before: `gst` is `git status` passthrough, `gcl` is `gfo` then `gcom`.

### Settings

Press `s` from the workspace list to open the settings page.

Global settings include:

- **Profiles**: Add new profiles or delete existing ones

Profile-specific settings (for the current profile) include:

- Setting a prepare command: a shell command that runs after every worktree creation (e.g., `npm install`, `mise install`, custom Bash script)
- **Repos**: Register or unregister repos at arbitrary paths (in addition to auto-discovered repos under `<profile-root>/repos/`)

Info section:

- **View Changelog**: Opens the full changelog in a new kitty tab using glow

## The `ccc` Command

`ccc` (Customized Claude Code) is a launcher that wraps Claude Code with Devora-specific configuration.
Run it from within a workspace to start a Claude Code session with all Devora integrations active.

What it does:

- **Model upgrades**: Default models are upgraded -- Sonnet is replaced with Opus, and Haiku with Sonnet. This provides stronger reasoning out of the box but does affect billing accordingly
- **Effort level**: Sets the effort level to "max" by default, so that Claude Code always tries to give the best answer it can (instead of trying to be fast or cost-efficient)
- **Privacy**: Disables telemetry and nonessential network traffic
- **Agent teams**: Enables experimental agent teams support
- **Plugins**: Auto-loads Devora plugins (including Judge -- see below)

### Judge

Judge is a permission auto-manager plugin for Claude Code.
It automatically handles Claude Code's permission requests to reduce permission fatigue during development.
Known-safe commands (like `cat`, `ls`, `git diff`, etc.) are approved automatically, disallowed commands are declined with a reason, and anything unrecognized is deferred to the user for manual review.
Judge runs automatically when using `ccc` -- no additional setup is needed.

## The `debi` CLI

`debi` is Devora's command-line tool for workspace management and utilities.
Run `debi` with no arguments or `debi --help` to see all available commands.

Highlights:

- **Git shortcuts**: A collection of short aliases for common git workflows (e.g., `debi gst` for `git status`, `debi gd` for `git diff`, `debi gaa` for staging all changes). Run `debi --help` for the full list
- **PR status**: `debi pr check` (or `check-pr`) shows the CI/review status of the current branch's pull request
- **Submit a PR**: `debi pr submit` (or `submit-pr`) from detached HEAD creates a commit, optionally a tracker task, a feature branch, and a GitHub PR in one step
- **Close a PR**: `debi pr close` (or `close-pr`) completes the tracker task (if any), deletes the remote and local branches, and returns the working tree to detached HEAD on the default branch
- **Health check**: `debi health` verifies that all required dependencies are installed
- **Shell completions**: See [Recommended: Shell Completions](#recommended-shell-completions)

## Recommended: Shell Completions

Shell completion makes `debi` much faster to use by expanding unique command prefixes (for example, typing `debi w` and pressing Tab runs `workspace-ui`).

`debi` supports tab-completion for bash, zsh, and fish.
Run `debi completion <shell>` to generate the completion script, and `debi completion <shell> -h` for shell-specific installation instructions.

For zsh (add to `~/.zshrc` before `compinit`):

```zsh
fpath=(~/.zsh/completions $fpath)
autoload -U compinit; compinit
```

Then generate the completion file:

```
mkdir -p ~/.zsh/completions
debi completion zsh > ~/.zsh/completions/_debi
```

On macOS, `mise mac-install` automatically generates the zsh completion file for you; you still need to add the `fpath` line above to your `~/.zshrc`.

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

Note: the split-pane shortcuts below operate on Kitty panes (each pane is its own shell). Neovim's own `:sp` / `:vsp` still work independently inside any pane.

- `ctrl + s`: Open workspace manager in a new tab
- `ctrl + shift + s`: Open shell in a new tab
- `ctrl + left / right`: Navigate between tabs (workspeces)
- `ctrl + shift + left / right`: Reorder tabs
- `ctrl + 1 / 2 / 3`: Set font size to small (12pt) / medium (15pt) / large (26pt)
- `ctrl + equal / minus`: Increase / decrease font size
- `ctrl + shift + v` or `cmd + v`: Paste from clipboard
- `ctrl + shift + r`: Reload configuration
- `cmd + shift + \`: Split the current Kitty pane vertically (new shell to the right) — vim `:vsp` analog
- `cmd + shift + -`: Split the current Kitty pane horizontally (new shell below) — vim `:sp` analog
- `cmd + shift + h / j / k / l`: Move focus between split panes (left / down / up / right)
- `cmd + shift + w`: Close the current split pane
- `F1`: Open this User Guide

### Workspace List

- `<enter>`: Open selected workspace (closes the current UI and opens a terminal session)
- `n`: New task (reuses or creates a workspace and opens it in a dedicated tab)
- `d`: Deactivate workspace (active only)
- `D`: Delete workspace (inactive/invalid only)
- `r`: Refresh workspace list
- `1 / 2 / 3`: Filter workspaces by "Active" (default) / "Inactive" / "All"
- `j / k`: Navigate list
- `s`: Open settings (includes "View Changelog" option)
- `p`: Cycle active profile
- `q`: Quit (or back in other pages)
- `esc`: Same as `q`, and also "unfocuses" on text inputs

## Troubleshooting

### Crash Logs

If Devora crashes, a log file is written to your system's temp directory with the name `devora_crash_<YYYYMMDD_HHMMSS>.log`.
On macOS, the temp directory is typically under `/var/folders/`.
The crash output also prints the exact file path to stderr.

### Dependency Issues

Run `debi health --verbose` to see the status and location of each dependency.
Use `--strict` to also check optional dependencies.

### macOS Gatekeeper

On first launch, macOS may block Devora because it is not signed by an identified developer.
If you used the curl-based installer (see README), the quarantine attribute is cleared automatically and this step is not needed.
To work around this manually: right-click the app and select "Open", press "Done" on the warning dialog, then open "Security & Privacy" in System Settings and allow the app to run.

# User Guide

Work in Devora is organized around **profiles** and **workspaces**.

When you start Devora for the first time, you'll be prompted to create a **profile**.
A **profile** is a named configuration scope.
Each profile has its own root directory and configurations (that can override global defaults).
Profiles provide isolation between different contexts; for example, you might have one profile for company-internal projects and another for OSS projects.

To do work in Devora, you create a **task**.
A **task** is a **workspace** with a title describing the work.

A **workspace** is a directory that contains a set of git worktrees.
The idea is that each task you do involves one or more repos, so when you start a task, you select the relevant repositories, and a workspace is created with a git worktree for each selected repo.

When you create a task, a workspace is created (or reused from cache if it already exists for the same set of repos) and a **session** is launched.

A **session** is a task that is open in Devora in a dedicated tab.
You can switch between sessions by clicking on the tabs, or cycle through them with `ctrl + left / right`.
Within a session, you have a shell where you can run commands, and a special `ccc` command to launch Customized Claude Code in the workspace context.
It is recommended to run Neovim (or Tmux) within the session to take advantage of pane splits and file navigation/editing features.

## First Launch

When you start Devora for the first time, it will prompt you to create your first profile.
You choose a root directory (defaults to `~/devora`) and give the profile a name.
Once the profile is created, you land on the Workspace Hub - Devora's home screen, where you can create and manage tasks and workspaces.

If the chosen directory already contains an initialized profile, Devora detects it and registers it as-is instead of creating a new one.

## Profiles

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
- `workspaces/` contains the worktrees Devora creates for your workspaces

You can have multiple profiles.
To select a profile (or manage them) go to the **Profile Manager**.
That can by done from the Workspace Hub by pressing `P` (uppercase) or the profile dropdown in the top-right corner.
In addition, you can open it through the Command Palette (rapid shift-shift) by selecting the "Manage Profiles" command.

**Note**: Deleting a profile (also via Settings) only removes it from Devora's registry. The profile directory and all its contents remain on disk.

## Repos

Each profile maintains its own repo directory and list of additional registered git repositories.
There are two ways to add repos to a profile:

1. **Auto-discovered repos**: Clone a git repository directly into `<profile-root>/repos/`
	- Devora automatically picks it up - no extra registration needed
2. **Explicitly registered repos**: Register repos at arbitrary paths - this is currently not implemented in the you and requires a manual editing of the profile's configuration.
	- This can be used to include repos that are shared across profiles, or that you don't want to live inside the profile directory for any reason

## Workspaces

Workspaces are built to span multiple repositories simultaneously.
This is achieved by including a separate git worktree for each selected repo, so you can work across repos within a single workspace without affecting other workspaces - even for work that touches the same repo(s).

From the Workspace Hub:

1. Press `n` to start a new task
2. Select one or more repos from the list
3. Enter a short title (shorter is better, e.g: "login bug", "refactor auth", "<project name initials>")
4. Devora creates a git worktree from each selected repo and opens a terminal session in the workspace directory
	- If a "prepare command" is set in the profile's settings, it runs in each worktree as part of its setup (e.g., to install dependencies)
	- An automated `CLAUDE.md` is generated in the workspace root, where you can supplement Claude with context about the work and why each worktree was included in the workspace
5. Run the `ccc` command to launch Claude Code in the workspace context
	- Now, Claude Code is ready with all of the Devora features and has access to all of the worktrees in the workspace

Once a workspace exists, you can manage it from the Workspace Hub (press `ctrl + s` of from the Command Palette)

## Git shortcuts

Devora provides a collection of git shortcuts for your convenience:

| Shortcut | Description |
|----------|-------------|
| `gst` | `git status` passthrough |
| `gd` | `git diff` passthrough |
| `gri N` | `git rebase -i HEAD~N` |
| `gfo` | Fetches the latest changes from origin and prunes deleted branches |
| `gcom` | Shortcut for "git checkout master*": checks out the default branch in detached mode |
| `gcl` | Shortcut for "git checkout latest": equal to `gfo` then `gcom` |
| `gaa` | Stages all changes (including untracked files) |
| `gaaa` | Stages all changes (including untracked files) and amends to the previous commit |
| `gaac` | Stages all changes (including untracked files) and commits with a message provided in the command arguments |
| `grom` | Shortcut for "git rebase origin/master*": rebases the current branch onto the default branch |
| `grl` | Shortcut for "git rebase latest": equal to `gfo` then `grom` |
| `grlp` | Shortcut for "git rebase latest push": equal to `gfo` then `grl` then `git push --force` |

* The name of the default branch is detected automatically, so it works even if it's not called "master"

### Workspace-level git shortcuts

Two of the git shortcuts are workspace-aware when run from the exact root of a workspace directory (`ws-N/`):

- `gst` prints an aligned per-worktree summary across every worktree in the workspace: branch (or `HEAD` if detached), staged/unstaged/untracked file counts, commits behind `origin/<default>`, and PR state
- `gcl` performs the "checkout latest" workflow across every worktree in the workspace in parallel, with a pre-check to ensure all worktrees are clean and on detached HEAD before doing any operations
	- This makes it easy to sync up an entire workspace to the latest origin state with one command without worrying about accidentally losing uncommitted changes in any of the worktrees

## Debi

Devora includes a CLI tool called `debi` for CLI-based workflows.
Debi implements the git shortcuts described above, as well as other utilities.

Highlights:

- **PR**:
	- `debi pr check` to check the status of the current branch's PR
	- `debi pr submit` to create a PR from detached HEAD
	- `debi pr close` to close the current branch's PR and clean up branches
- **Health check**: `debi health` verifies that all required dependencies are installed
- **Read config**: `debi get-conf [--profile <name>] <key>` reads a resolved config value and prints it to stdout, intended for use in shell scripts

### Shell Completions

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

## Config File Settings

For now, some settings are configured directly in `config.json` (global or per-profile) rather than through the UI or `debi`.

| Key | Values | Default | Scope |
|-----|--------|---------|-------|
| `terminal.default-app` | any shell command (e.g. `fish`, `nvim`, `tmux`) | empty (bare login shell) | profile-overridable |
| `terminal.git-shortcuts` | `true`, `false` | `true` | profile-overridable |

### `terminal.git-shortcuts`

By default, each session has a shell.
Set `terminal.default-app` to `nvim` (or any other command) to launch instead of a plain shell.

Example:

```json
{
	"terminal": {
		"default-app": "nvim"
	}
}
```

### `terminal.git-shortcuts`

Controls whether Devora session shells expose Debi's git shortcuts as bare commands (see [Git shortcuts](#git-shortcuts)).

To turn this off, set `terminal.git-shortcuts` to `false` in `config.json` (globally or per-profile):

```json
{
	"terminal": {
		"git-shortcuts": false
	}
}
```

## The `ccc` Command

`ccc` (Customized Claude Code) is a launcher that wraps Claude Code with Devora-specific configuration.
Run it from within a workspace to start a Claude Code session with all Devora integrations active.

What it does:

- **Model upgrades**: Default models are upgraded -- Sonnet is replaced with Opus, and Haiku with Sonnet
	- This provides stronger reasoning out of the box
	- The cost-per-token is higher, but that should be balanced by lower token usage due to better performance
	- These defaults, and the effort level below, are configurable (globally and per profile)
- **Effort level**: Sets the effort level to `xhigh` (extra-high) by default, so that Claude Code always tries to give a better result
- **Plan mode**: Claude Code starts in plan permission mode by default, which requires explicit approval before making changes and takes advantage of the Crit integration
- **Plugins**: Auto-loads Devora plugins (including Judge -- see below)

## Crit Integration

[Crit](https://github.com/tomasz-tomczyk/crit) is a tool for reviewing and commenting on plans, code diffs, and frontend elements to  send feedback directly to your agent.
Crit is integrated into Devora and works out-of-the-box.
When Claude Code has a plan ready, it opens up Crit's UI in a tab (session) covering overlay, where you can review the plan, comment on specific lines, and approve or request changes.
Once Claude receives your feedback, it adjusts the plan and re-presents for a follow-up review until it's approved.

## Judge

Judge is a permission auto-manager plugin for Claude Code.
It automatically handles Claude Code's permission requests to reduce permission fatigue during development.
Known-safe commands (like `cat`, `ls`, `git diff`, etc.) are approved automatically, disallowed commands are declined with a reason, and anything unrecognized is deferred to the user for manual review.
Judge runs automatically when using `ccc` -- no additional setup is needed.

Jedge persists unsupported cases to `~/.claude/cc-judge-unsupported-cases.json`, which can be submitted to the project to help expand Judge's automatic coverage.

## Optional Additions

### CC Status Line

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
- Programming-language package-managements (`npm`, `poetry`, `uv`, `gem`, `cargo`, etc.) with its [deps](https://mise.jdx.dev/dev-tools/deps.html) feature
- Setting up env-vars

## Versioning

Devora follows a date-based version format: `YYYY-MM-DD.PATCH` (e.g., `2026-03-28.0`).
Development builds between releases show as `YYYY-MM-DD.PATCH-dev.N` where N is the number of commits since the last release.

The current version is displayed in the window title.

## Cheatsheet

### Devora-wide

- Rapid `shift + shift`: Open the Command Palette for quick access to all commands and settings
- `ctrl + s`: Open the Workspace Hub (covers the whole window)
- `ctrl + shift + s`: Open shell in a new tab
- `ctrl + left / right`: Navigate between tabs (sessions)
- `ctrl + shift + left / right`: Reorder tabs
- `ctrl + 1 / 2 / 3`: Set font size to small / medium / large
- `ctrl + shift + minus / plus`: Decrease / increase font size
- `F1`: Open this User Guide (this)

### Workspace Hub

- `<enter>`: Open selected task in a new session tab
- `n`: New task (reuses or creates a workspace and opens it in a new tab)
- `R`: Refresh Workspace Hub
- `1 / 2 / 3`: Filter workspaces by "Active" (has a task, default) / "Inactive" / "All"
- `j / k`: Navigate list
- `P`: Open the Profile Manager (list, switch, create, and delete profiles)
- `q`: Quit (or back in other pages)
- `esc`: Same as `q`, and also "unfocuses" on text inputs

## Troubleshooting

### Crash Logs

If Devora crashes, a log file is written to your system's temp directory with the name `devora_crash_<YYYYMMDD_HHMMSS>.log`.
On macOS, the temp directory is typically under `/var/folders/`.
The crash output also prints the exact file path to stderr.

### Dependency Issues

Open the **Health Hub** (Command Palette or Workspace Hub), or use `debi health` to see the status of each dependency, credential, and config.

From a terminal, `debi health --verbose` shows the status and location of each dependency (`--strict` also checks optional dependencies, `--json` prints the machine-readable report the Health Hub renders).

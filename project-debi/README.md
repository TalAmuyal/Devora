# Devora

Devora is a developer workspace manager. It manages isolated git worktree-based workspaces, each associated with a task, and integrates with the Kitty terminal to create dedicated terminal sessions per workspace. The TUI (`debi`) lets you create, switch between, and manage workspaces, register git repos, and configure profiles.

## Kitty Setup

Devora uses Kitty's remote control protocol to manage terminal sessions. Enable it in your Kitty config (`~/.config/kitty/kitty.conf`):

```
allow_remote_control yes
```

## Tech Stack

- [Mise](https://mise.jdx.dev/) for dependency and toolchain management
- Go as the programming language
- [Bubble Tea](https://github.com/charmbracelet/bubbletea) for building the TUI
- [Lip Gloss](https://github.com/charmbracelet/lipgloss) for styling the TUI

## Prerequisites

This project uses [mise](https://mise.jdx.dev/) to manage dependencies and toolchains.

1. Install `mise`.
2. Run `mise trust` to trust the project and its dependencies.
3. That's it! `mise` will handle the rest, including installing the correct Go version and keeping your dependencies in sync.

## Running the App

Because `mise` is auto-running `go mod tidy` for you under the hood, you can run the app directly via:
```bash
mise start
```

## Building

```bash
mise build
```

This produces the `debi` binary.

## Testing

```bash
mise test
```

## PR workflow: `debi submit` and `debi close`

`debi submit` commits local changes, creates a tracker task (when a task tracker is configured), pushes a feature branch, and opens a GitHub PR in one step. `debi close` is the other end: after the PR is merged it marks the tracker task complete, deletes the remote branch, and returns the working tree to detached HEAD on the default branch. Both commands are also available as `debi pr submit` and `debi pr close`.

## Documentation

See [docs/README.md](docs/README.md) for a guide to the project documentation.

This repo contains the "Devora" project.
Devora is a heavily opinionated IDE for the agentic world.
Its goal is to leverage and compose existing tools to create a powerful and efficient agentic IDE.

## Supported platform

Only macOS (Apple Silicon) is currently supported, but there are plans to add Linux support in the future.

## Prerequisites

### Required

- [zsh](https://www.zsh.org/)

### Expected

- [Claude Code](https://docs.anthropic.com/en/docs/claude-code) (`claude`) -- agentic coding tool (requires a subscription)
- [git](https://git-scm.com/)

### Optional (recommended)

- [Neovim](https://neovim.io/) (`nvim`) -- recommended editor/terminal multiplexer; configurable via `terminal.default-app` (defaults to a bare login shell)
- [mise](https://mise.jdx.dev/) -- dev tool and task manager
- [GitHub CLI](https://cli.github.com/) (`gh`) -- GitHub integration and credential checking

Run `debi health` to verify that all required dependencies are installed.
Use `--strict` to also check optional ones.

## Getting started

1. Install (see options below)
2. Read the [User Guide](USER_GUIDE.md) for day-to-day usage (available from the app via F1)

### Install from a release

#### One-line install (recommended)

Stable:

```
curl -fsSL https://raw.githubusercontent.com/TalAmuyal/Devora/master/install.sh | bash
```

Nightly:

```
curl -fsSL https://raw.githubusercontent.com/TalAmuyal/Devora/master/install.sh | bash -s -- --nightly
```

Both commands download the latest DMG, replace any existing `/Applications/Devora Ember.app`, clear the macOS quarantine attribute (so Gatekeeper won't block first launch), and install the `debi` zsh completion.

Choose Sapphire on the nightly channel:

```
curl -fsSL https://raw.githubusercontent.com/TalAmuyal/Devora/master/install.sh | bash -s -- --app sapphire --nightly
```

`--app ember` is the default; Sapphire is nightly-only until its daily-use gate passes.
Installing Ember removes the legacy `/Applications/Devora.app` after the new app is in place, preserving suffixed source installs.
During the transition, the installer also accepts the previous stable `Devora.dmg` asset and installs its legacy app under the new directory name.
Sapphire installs alongside Ember as `/Applications/Devora Sapphire.app`.

#### Manual install

Download the latest `.dmg` from the [GitHub Releases](https://github.com/TalAmuyal/Devora/releases) page and drag `Devora Ember.app` to `/Applications`.

If Gatekeeper blocks the app on first launch, clear the quarantine attribute manually:

```
xattr -dr com.apple.quarantine "/Applications/Devora Ember.app"
```

### Build from source

With mise installed:

```
mise mac-install
```

This builds `Devora Ember.app` and installs it to `/Applications`.

To build and install the native Sapphire app, run `mise run sapphire-install`.
Use `--path <directory>` to choose the destination, or `--suffix <name>` for a side-by-side install with separate app state.
Untagged builds include their `-dev.` version in the app name.
This uses an ad hoc signature for local use.
Nightly Sapphire downloads use the installer above.

## Components

Devora builds upon the following tools:
- Claude Code, as the agentic coding tool
- Ember (Tauri + xterm.js), as the UI framework
- Sapphire (`./project-sapphire/`), the native macOS build in progress (SwiftUI + libghostty over the shared Rust core)
- Core (`./project-core/`), the shared Rust core that holds Devora's non-UI logic
- Debi (`./project-debi/`), Devora's CLI for workspace management and utilities
- Judgess (`./project-judgess/`), a Claude Code plugin for auto-approving/rejecting permission requests aiming to reduce permission fatigue and speed up the development process
- Code-Cleanup (`./project-code-cleanup/`), a Claude Code plugin that defines and directs a post-task cleanup process (code decluttering and comment cleanup)
- CCC (`debi ccc`, also available as `ccc`), a launcher for Claude Code that customizes its behavior and integrates it with the rest of the IDE
- CC Status Line (`./project-status-line/`), a simple status line script for Claude Code that shows the current context-window usage and session cost
- Bundler (`./bundler/`), a tool for bundling the IDE into a self-contained app bundle for distribution

## Project layout

Some of the tools are sub-projects in the repo, while others are off-the-shelf tools.
Sub-projects are located in directories at the root of the repo with a "project-" prefix (e.g., `project-debi/`), while off-the-shelf tools are either vendored with the IDE or are mentioned in a requirements/lock file.

## License

This project is licensed under the MIT License. See [LICENSE](LICENSE) for details.

For third-party licenses, see [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md).

## Anthropic trademark notice

Claude and Claude Code are trademarks of Anthropic, PBC.
Devora is an independent project not affiliated with or endorsed by Anthropic.
Using Devora expects a Claude Code subscription/plan and might need acceptance of Anthropic's Terms of Service.

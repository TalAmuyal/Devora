This repo contains the "Devora" project.
Devora is a heavily opinionated IDE for the agentic world.
Its goal is to leverage and compose existing tools to create a powerful and efficient agentic IDE.

This is not a traditional IDE, like Eclipse, VSCode, or JetBrains IDEs.
It is a terminal-based app that integrates existing tools as much as possible to provide a complete (and dedicated) experience.

## Supported platform

macOS (Apple Silicon) only.

## Prerequisites

### Required

- [zsh](https://www.zsh.org/)

### Expected

- [Claude Code](https://docs.anthropic.com/en/docs/claude-code) (`claude`) -- agentic coding tool (requires a subscription)
- [git](https://git-scm.com/)

### Bundled

- [Kitty](https://sw.kovidgoyal.net/kitty/) -- terminal emulator used as the main UI container
- [uv](https://docs.astral.sh/uv/) -- Python package manager
- [glow](https://github.com/charmbracelet/glow) -- terminal-based Markdown renderer

### Optional

- [Neovim](https://neovim.io/) (`nvim`) -- recommended editor/terminal multiplexer; configurable via `terminal.default-app` (defaults to a bare login shell)
- [mise](https://mise.jdx.dev/) -- dev tool and task manager
- [GitHub CLI](https://cli.github.com/) (`gh`) -- GitHub integration and credential checking

Run `debi health` to verify that all required dependencies are installed.
Use `--strict` to also check optional ones.

## Getting started

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

Both commands download the latest DMG, replace any existing `/Applications/Devora.app`, clear the macOS quarantine attribute (so Gatekeeper won't block first launch), and install the `debi` zsh completion.

#### Manual install

Download the latest `.dmg` from the [GitHub Releases](https://github.com/TalAmuyal/Devora/releases) page and drag Devora.app to `/Applications`.

If Gatekeeper blocks the app on first launch, clear the quarantine attribute manually:

```
xattr -dr com.apple.quarantine /Applications/Devora.app
```

### Build from source

With mise installed:

```
mise mac-install
```

This builds Devora.app and installs it to `/Applications`.

See the [User Guide](USER_GUIDE.md) for full documentation on profiles, workspaces, and day-to-day usage.

## Components

Devora builds upon the following tools:
- Claude Code, as the agentic coding tool
- Kitty, as:
	- The main UI container (which means this IDE is terminal-based)
	- A multi-session manager (each Kitty tab holds an active session of the IDE)
- Neovim, as the recommended multi-purpose tool for:
	- Code exploration and editing
	- Terminal multiplexer: It can multiplex files, scratch buffers, and terminals in the same session over different Neovim tabs and Neovim windows (splits)
	- Configurable via `terminal.default-app`: by default each workspace session opens a bare login/interactive shell; set `terminal.default-app` to `nvim` (or any other command) to launch a wrapped app instead
- Debi (`./project-debi/`), a UI tool for workspace and session management
- Judge (`./project-judge/`), a Claude Code plugin for auto-approving/rejecting permission requests aiming to reduce permission fatigue and speed up the development process
- CCC (`./ccc.sh`), a launcher script for Claude Code that customizes its behavior and integrates it with the rest of the IDE
- CC Status Line (`./project-status-line/`), a simple status line script for Claude Code that shows the current context-window usage and session cost
- Bundler (`./bundler/`), a tool for bundling the IDE into a self-contained app bundle for distribution

## Project layout

Some of the tools are sub-projects in the repo, while others are off-the-shelf tools.
Sub-projects are located in directories at the root of the repo with a "project-" prefix (e.g., `project-debi/`), while off-the-shelf tools are either vendored with the IDE or are mentioned in the requirements section of the user guide.

## License

This project is licensed under the MIT License. See [LICENSE](LICENSE) for details.

For third-party licenses, see [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md).

## Anthropic trademark notice

Claude and Claude Code are trademarks of Anthropic, PBC.
Devora is an independent project not affiliated with or endorsed by Anthropic.
Using Devora expects a Claude Code subscription and might need acceptance of Anthropic's Terms of Service.

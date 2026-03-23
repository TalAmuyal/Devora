This repo contains the "Devora" project.
Devora is a heavily opinionated IDE for the agentic world.
Its goal is to leverage and compose existing tools to create a powerful and efficient agentic IDE.

This is not a tradition IDE, like Eclipse, VSCode, or JetBrains IDEs.
It is a terminal-based app that integrates existing tools as much as possible to provide a complete (and dedicated) experience.

Devora builds upon the following tools:
- Claude Code, as the agentic coding tool
- Kitty, as:
	- The main UI container (which means this IDE is terminal-based)
	- A multi-session manager (each Kitty tab holds an active session of the IDE)
- Neovim, as a multi-purpose tool for:
	- Code exploration and editing
	- Terminal multiplexer: It can multiplex files, scratch buffers, and terminals in the same session over different Neovim tabs and Neovim windows (splits)
- Debi (`./project-debi/`), a UI tool for [Devora workspace]() and [Devora session]() management
- Judge (`./project-judge/`), a Claude Code plugin for auto-approving/rejecting permission requests aiming to reduce permission fatigue and speed up the development process
- CCC (`./ccc.sh`), a launcher script for Claude Code that customizes its behavior and integrates it with the rest of the IDE
- CC Status Line (`./project-status-line/`), a simple status line script for Claude Code that shows the current context-window usage and session cost
- Bundler (`./bundler/`), a tool for bundling the IDE into a self-contained app bundle for distribution

## Project layout

Some of the tools are sub-projects in the repo, while others are off-the-shelf tools.
Sub-projects are located in directories at the root of the repo with a "project-" prefix (e.g., `project-debi/`), while off-the-shelf tools are either vendored with the IDE or are mentioned in the requirements section of the user guide.

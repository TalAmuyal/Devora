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
- Debi (./project-debi/), a UI tool for [Devora workspace]() and [Devora session]() management
- Bundler (./bundler/), a tool for bundling the IDE into a self-contained app bundle for distribution

## Project layout

```
repo root/
├─ bundler/
│  └─ ...
├─ [PLACE HOLDER]
└─ project-debi/
   └─ ...
```

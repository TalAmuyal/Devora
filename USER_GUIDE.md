# User Guide

## Requirements

- Install [Claude Code](https://code.claude.com/docs/en/quickstart#step-1-install-claude-code)

### MacOS

For the first usage, you might need to "Right-click > Open" the app to bypass MacOS Gatekeeper.

## Optional Additions

### Simple Status Line

A Devora-provided status line is available to show the current context-window usage and session cost.

To enable it, copy `cc-simple-statusline` to your `~/.claude/` directory and add the following to your `~/.claude/settings.json`:

```json
	"statusLine": {
		"type": "command",
		"command": "~/.claude/cc-simple-statusline",
		"padding": 0
	},
```

## Introduction

TBD

## Cheetsheet

- `ctrl + s`: Open workspace manager in a new OS window tab
- `ctrl + shift + s`: Open shell in a new OS window tab

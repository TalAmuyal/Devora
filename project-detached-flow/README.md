# Detached-Flow

A Claude Code plugin that supports Devora's detached-HEAD based PR workflow.

It ships with Devora and is loaded automatically when Claude Code is launched from a Devora session.

## Contents

- `cc-plugin/bin/` — short aliases for the `debi pr` subcommands
	- `submit-pr` → `debi pr submit`
	- `close-pr`  → `debi pr close`
	- `check-pr`  → `debi pr check`
- `cc-plugin/skills/` — Claude Code skills
	- `submit-pr/` — guidance for submitting a PR, includes instructions for writing a message and a description

## Design notes

### Why ship aliases alongside a skill?

The aliases are thin wrappers around the corresponding `debi pr` subcommand and serve two audiences at once:

- **Users** get short, memorable commands in their shell.
- **Claude** gets the same commands, plus usage context from the `submit-pr` skill.

Devora's bootstrap automatically prepends each cc-plugin `bin/` directory to `PATH`, so shipping the aliases as part of the plugin is enough to make them available everywhere in a Devora session.

### Why only a `submit-pr` skill and no `close-pr` / `check-pr` skills?

`submit-pr` is the only command that benefits from a skill today: writing a good PR message and description is a judgment call that warrants guidance.
`close-pr` and `check-pr` take no meaningful arguments and the wrapped `debi` command already does the right thing, so a dedicated skill would add noise without value.

Dedicated skills may be added later as the flow grows richer.
For example, if Claude is ever notified of review comments or merge events.

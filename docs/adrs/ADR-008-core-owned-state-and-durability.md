# ADR-008: Core-Owned App State and Durability

## Status

Accepted

## Date

2026-10-28

## Context

Sapphire must be durable: after a crash, a kill or a quit, reopening it restores every tab, pane, conversation and unsent prompt as if nothing happened.
It also adds layout that Ember doesn't have: two levels of tabs, splits, overlays, and a `:` command line with Vim-style keymaps (ADR-004).

If each UI owned its layout state, durability, commands and keymaps would be built twice, and a crash would lose whatever only lived in a view.

## Decision

- **The core owns the full app-state model**
	- The model: a window, holding workspace tabs (one per open workspace), holding inner tabs, each a split tree of panes (agent, terminal, editor, web, file/diff), plus an overlay stack following ADR-003.
	- UIs are projections of that state.
		Anything that must survive a restart lives in the core, not in a view.
- **Every mutation is a named command**
	- The same registry backs the `:` command line and keybindings.
	- The flow is command → reducer → one transaction → event to the UIs.
		This gives one place for undo, logging, and future scripting or agent control of the UI.
- **SQLite in WAL mode, written through per command**, so a `kill -9` loses at most the last debounce window of typing.
	Unsent prompts stream from the embedded editor to the core, debounced to about 250 ms.
- **Each app keeps its own databases.**
	Ember and Sapphire never share a state database; workspaces and config stay shared through the filesystem, as today.
- **Conversation history lives in a separate `conversations.db`**
	- Fixed tables:
		- `agents`
		- `agent_sessions`: agent, Claude session id, an optional workspace, who spawned it, start and end
		- `events`: session, sequence number, kind, payload
	- Whole messages and UI-only facts are stored (Judgess verdicts, permission answers, interruptions). Token deltas stay in memory.
	- Large tool outputs are compressed or stored content-addressed.
	- History is deleted with its session or workspace.
- **Agents and agent sessions are first-class state, not panes**
	- An agent is a long-lived identity that may reference a workspace but is not owned by one. An agent session is one Claude conversation.
	- A pane views a session, so a session can run unseen, be shown twice, or belong to no workspace.
		This keeps multi-agent work open (per-workspace agents and a cross-Devora agent) without designing it now.

## Alternatives considered

- **Each UI owns its own layout state.** Rejected: durability, commands and keymaps would be written twice, and view-only state is lost on a crash.
- **JSON/TOML files.** Rejected: not crash-safe at keystroke frequency without reinventing transactions.
- **One shared database per profile for both apps.** Rejected: it needs cross-app locking, and the apps' layouts differ anyway.
- **Claude Code's own session JSONL as the conversation record.** Rejected: its format is undocumented and reshaped by compaction, the CLI prunes it, and it lacks UI-only facts. It remains the source for `--resume` (ADR-009).
- **A table per agent session.** Rejected: schema churn, per-table migrations, and no queries across sessions (search, cost per workspace, tracing messages between agents).

## Consequences

- Restoring is verified by an acceptance test that kills the app at random points and compares the restored state tree.
- Ember can adopt the model and command registry incrementally (ADR-006).
- Terminal processes cannot survive a crash: a terminal pane restores its cwd, command and a scrollback snapshot, and relaunches its shell.

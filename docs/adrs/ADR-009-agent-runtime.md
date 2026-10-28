# ADR-009: Agent Runtime (stream-json Claude with Judgess In-Process)

## Status

Accepted

## Date

2026-10-28

## Context

Ember runs Claude Code as an interactive TUI (`ccc`) in a terminal pane.
Sapphire instead embeds agents and gives them a UI built for Devora's users.
Auto mode, Judgess (ADR-005), compaction and prompt caching must keep working.

The options were the raw CLI in headless mode, an Agent SDK sidecar process, or the Messages API directly.

Spike TalAmuyal/Devora#205 tested the raw CLI against Claude Code 2.1.312 ([report](https://github.com/TalAmuyal/Devora/issues/205#issuecomment-6478770773)).
It recorded protocol fixtures that seed the contract tests.

## Decision

- **One persistent `claude -p` process per agent session, driven over stdio**
	- Flags: `--input-format stream-json --output-format stream-json --verbose --include-partial-messages --permission-prompt-tool stdio`, plus the `LaunchSpec`'s plugins, model, effort and permission mode (ADR-007).
	- **Two channels on stdio:**
		- The conversation: user turns in; assistant, tool and result messages and token deltas out.
		- The control protocol: `control_request`/`control_response` in both directions, which the official SDKs use underneath.
			- Host to CLI: `initialize`, `interrupt`, `set_permission_mode`, `set_model`.
			- CLI to host: `can_use_tool`, `hook_callback`, `mcp_message` and `control_cancel_request`.
	- Compaction, caching, hooks and plugins stay inside the CLI.
		`/compact` is a user turn and is marked by a `compact_boundary` message. Mode changes, interrupts and `--resume` after a kill were all verified.
	- Claude is spawned with a scrubbed environment (auth and proxy only).
		Otherwise a launcher running inside a Claude Code session leaks that session's identity, and the child reuses its session id.
- **Judgess is linked in-process as a `PreToolUse` hook callback**
	- Sapphire registers it in `initialize`; the plugin is left out of Sapphire's sessions, so nothing is judged twice.
	- Approve maps to `allow`, Deny to `deny` with the redirect message, and Defer to `ask` (which forces a prompt even in auto mode). An abstention returns nothing, so the CLI's own flow decides.
	- It runs before both the native prompt and auto mode's classifier, with no race.
	- The user's own settings `deny`/`ask` rules still win over an `allow`.
	- A decision costs 40–96 µs, and an approval skips auto mode's ~0.5 s classification.
- **Native prompts come only from `can_use_tool`**
	- It carries the reason, a suggested "always allow" rule and the `tool_use_id`.
	- A prompt is dropped when the CLI sends `control_cancel_request`.
- **Agent processes live and die with the UI.**
	Crash recovery respawns with `--resume <session-id>`, and the transcript renders from the core's own history (ADR-008) while it does.
- **The protocol is isolated**
	- All parsing sits in one `protocol` module, with contract tests over recorded fixtures.
	- Unknown messages pass through as `Raw` instead of failing.
	- A minimum CLI version (2.1.312 for now) is checked by `debi health`.
	- Acceptance tests run against a Fake Claude backend (ADR-001).

## Alternatives considered

- **Agent SDK sidecar (TypeScript or Python).** Rejected: it bundles a runtime for a typed wrapper over the same CLI protocol.
- **The Messages API directly.** Rejected: it would reimplement tools, hooks, plugins, compaction, caching and auto mode.
- **Judgess as the plugin's `PermissionRequest` hook, or as an in-process `PermissionRequest` callback.** Rejected: that hook fires only when a prompt is raised, so it races the prompt in manual mode and never fires in auto mode. TalAmuyal/Devora#213 tracks moving the plugin itself to `PreToolUse`.
- **Judgess answering `can_use_tool`.** Rejected: `can_use_tool` is never sent in auto mode.
- **Long-lived agents in a daemon, detachable from the UI.** Rejected for now: it is simpler to tie agents to the UI. A detach project can host the same in-process crates later.

## Consequences

- The control protocol is a contract between the CLI and its SDKs rather than a documented public API, so CLI upgrades re-record fixtures and rerun the contract tests.
- A Judgess rules fix reaches Sapphire through a Sapphire build.
	Release builds must keep `panic = "unwind"`, so a Judgess panic stays an abstention.
- The `initialize` response (slash commands, agents, models, effort levels) feeds `/` completion and the model and effort pickers.
- `mcp_message` lets the core serve Devora tools to agents in-process, which multi-agent work can build on.

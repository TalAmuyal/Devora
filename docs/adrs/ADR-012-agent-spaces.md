# ADR-012: Agent Spaces

## Status

Proposed.
It builds on the agent runtime (ADR-009) and the state model (ADR-008), and was shaped with the maintainer on TalAmuyal/Devora#254.

## Date

2026-11-05

## Context

Today a profile has `repos/` and `workspaces/`, and a workspace is a space for one task: the user runs Claude Code in it through `ccc`.
Sapphire's agentic part instead centres on persistent agents: an agent keeps its identity and conversation as it moves from task to task, and it keeps notes that survive the conversation's context limit.
ADR-009 decided how Devora drives Claude (one persistent `claude -p` stream-json process, with Judgess in-process).
This ADR decides what an agent is in Devora, where it lives, and how the user works with it.

## Decision

- **A new entity, the Agent Space: a directory owned by one persistent agent**
	- It lives at `<profile>/agent-spaces/<name>/`, beside `repos/` and `workspaces/`.
		It is not a Workspace: it has no task, no Active or Inactive status, and it doesn't appear in the Workspace Hub.
	- Its agent is created with it and has a name, a role and one Claude session (a conversation).
		The agent moves from task to task in the same space and the same conversation.
	- It starts with worktrees of some of the profile's repos, chosen at creation; more are added as needed, by the user (Add Repo) or by the agent, following its instructions.
	- `memory/`, in its root, is the agent's own persistent memory, which it organizes itself.
	- Its own SQLite database, `.agent-space.db` in its root, holds the agent's identity, the creation form's values, the Claude session id and the agent's conversation history.
		The history uses ADR-008's events model, but lives with the space instead of in an app-wide `conversations.db`, so it moves, backs up and is deleted with the space.
	- Agent Spaces are a Sapphire feature; Ember may adopt them later.
- **The agent's instructions are rendered from a template at every launch**
	- The template (the maintainer's draft) is filled from the launch spec, the profile config and the creation form's values, which can override the other two.
		It covers the agent's name and role, the user, the space and its repos, memory, an optional knowledge-base repo, version control and the working relationship, plus custom sections from the profile config.
	- Rendering at each launch, rather than once at creation, lets profile edits reach existing agents.
	- How the rendered text reaches Claude (an output style, `--append-system-prompt`, or a generated `CLAUDE.md` in the space) is chosen when it is implemented, by testing each against the pinned CLI.
- **An Agent Space's agent lives as a tab**
	- Opening its tab starts the agent, resuming its session (`--resume`) if it has one.
	- Closing the tab *pauses* the agent: a running turn is interrupted, the process stops, and everything is kept.
		Reopening the tab resumes the agent and restores its UI as far as possible: its panels, the transcript and its scroll position, and the unsent prompt.
	- Quitting the app pauses every agent, and a relaunch restores the open tabs (ADR-008) and resumes their agents.
	- Other kinds of agents, such as ones that run without a tab, may come later with their own lifecycle.
- **The UI**
	- The **Agent Hub** lists, creates and deletes Agent Spaces; creating one asks for the name, the role, the initial repos and any overrides of the profile's defaults.
	- An Agent Space's tab has a **chat panel** as its main panel.
		Terminal, preview and web panels can be opened beside it for work beyond the chat.
	- The transcript renders through the shared web renderer that previews and the User Guide use, with a native renderer as the fallback if that proves better.
	- The prompt editor starts as a plain multi-line text box.
		An embedded Neovim editor (ADR-010) comes later as an alternative, and the plain editor remains a setting.
- **Permissions: Judgess decides first, and the user answers what remains**
	- Judgess runs in-process as ADR-009 describes, and its verdicts show as non-actionable badges on tool calls.
	- Any question the CLI still asks (`can_use_tool`, including a Judgess Defer, or a user's own `ask` rule) shows as an inline card in the chat, answered from the keyboard (allow, deny, always allow with the CLI's suggested rule).
		A question the CLI withdraws (`control_cancel_request`) disappears.
- **Devora drives memory around compaction**
	- Devora watches the context usage the CLI reports.
		Near a threshold, it asks the agent to save what matters to `memory/`, then compacts (`/compact`), so memory doesn't depend on the agent remembering to save.
	- A hand-off to a fresh session, as a better alternative to compaction, is left for later.
- **Delegation uses Claude's own sub-agents**
	- An agent delegates through Claude Code's built-in sub-agents in its own session.
		Agents delegating to other Agent Spaces belongs to the multi-agent design (Phase 5).
- **Claude only, with neutral naming**
	- Agent Spaces run Claude Code.
		Names in the code and the UI don't assume Claude where it costs nothing, so that Codex support can be a later milestone; it isn't guaranteed and may integrate less well.

## Alternatives considered

- **Agents in ordinary Workspaces.** Rejected: a workspace is per task and is created and removed with the task, while an agent persists across tasks.
- **A JSON file for the agent's identity.** Rejected: one SQLite database holds the identity, the settings and the history with transactions, and is crash-safe as ADR-008 requires.
- **History in the app-wide `conversations.db` (ADR-008).** Rejected for Agent Spaces: the history belongs to the agent, and keeping it in the space keeps the space self-contained.
- **Stopping the agent for good when its tab closes.** Rejected: closing a tab pauses, so the agent resumes where it left off.
- **Agents that keep running without a tab.** Deferred to later agent kinds; this one is a tab.
- **No permission UI, with every remaining question denied automatically.** Rejected: the user answers what Judgess and auto mode leave open.
- **Leaving memory to the agent alone.** Rejected: the CLI compacts on its own schedule, so an agent that hadn't saved would lose that context.

## Consequences

- The core gains the Agent Space: creating and deleting it, its database, rendering its instructions, and pausing and resuming its agent.
	Debi may later get commands for Agent Spaces, as it has for workspaces.
- Sapphire gains the Agent Hub, the chat panel, inline permission cards and the plain prompt editor; the embedded Neovim editor follows as its own milestone.
- The profile config gains the template's inputs: the role, details about the user, custom sections, and later the knowledge-base repo.
- Pausing on tab close means a long-running turn stops when its tab closes; the transcript records the interruption.

# Agent Hub

The Agent Spaces of the selected profile (ADR-012, `Sources/AppShell/AgentHub/`), on a page layer over the whole window, with the Workspace Hub's keyboard model.

- `Ctrl+S` (`Surfaces.hubShortcut`) opens it when no other window surface is open; inside either hub it switches to the other, as the Agents | Workspaces toggle in both headers does (`HubSwitcher`), except while the Workspace Hub shows the first-run welcome.
	The Command Palette's "Agent Hub" opens it too.
- `AgentHubModel` loads the profiles, then lists the spaces (`Store.listAgentSpaces`); invalid ones (an interrupted creation, an unreadable database) show their error and can only be deleted.
	Its keys: `j`/`k` and the arrows move, Enter opens, `n` creates, `d` deletes (with a confirmation; an open agent is paused and its tab closed first, since the core deletes only a stopped one), `R` lists again.
- The New Agent dialog (`NewAgentDialogModel`) is a modal over the hub: the name, with the space's path under it, the template's fields (`agent_creation_fields`: the ones only the form fills, then the pre-filled ones with their source, then the derived ones, read-only, listed again as the name and repos change), the initial repos, the model and the effort.
	Fields show user-facing labels (`AgentFieldLabel`) with their `{{placeholder}}` and source beneath; `agent.name` and `space.path` aren't listed, as the Name field and the path under it show them.
	Editing a pre-filled field changes only this agent; its Save button writes the value to its source (`save_agent_field`: the global config for a global value, otherwise the profile).
	The model is free text, required, starting at `opus` (the latest Opus), with the models Claude Code offers as its suggestions menu once they arrive (`ModelSuggestions`, below); the effort is a stepped slider from "Claude Code default" (no `--effort`) to max, starting at the effort the user set for the profile or in User Defaults (`agent_model_defaults`).
	Both are stored with the agent and used for every launch.
	"Include coding instructions" (on by default) appends the instructions to Claude Code's system prompt, keeping its coding instructions; off, they replace it.
	The Preview beside the form shows the instructions as creation would render them (`agent_creation_preview`), or why it would fail, updated as the form changes.
	Create shows the creation's progress (`Store.createAgentSpace`) in the dialog's place, cancellable; once created, the agent opens in its tab and the hub closes.
- Opening an agent (`Store.openAgentSpace`) asks the core for the launch (`resolve_agent_launch`, the user's login shell), then `open_agent_space`, which opens its tab or activates the one it has.
	A launch resumes the agents of restored tabs in the background (`restore_agent_spaces`), and quitting stops every agent first (`AppTermination.pauseAgents`, from `applicationShouldTerminate`), after waiting for those still working (Global Pause, in `../CLAUDE.md`).
- An agent's tab shows its status emoji (`AgentStatusEmoji`: ⚙️ working, turning slowly unless Reduce Motion is on or the window inactive, ❓ waiting for the user, 💤 idle, ⏸️ pausing, ⚠️ failed), folded off the main actor from the core's chat events into `Store.agentStatuses`.
	Its tab's stack holds `AgentChatPanel`, with the native transcript (`Packages/TranscriptKit/CLAUDE.md`) above an AppKit plain multi-line editor.

## Model suggestions

`Store.modelSuggestions` asks the core for the models Claude Code offers a profile (`agent_model_options`, cached by the core, so only a cold call waits for the CLI) and wraps them in `ModelSuggestions`, the one object every model field takes (the New Agent dialog, the Claude card's tiers, later an agent's model widget): it orders, labels and narrows them, and the views only draw it (`modelSuggestions(_:showsModelIDs:)`, SwiftUI's native `textInputSuggestions` menu).
One list, no headings, in one order: by generation, newest first, then by tier (Fable, Opus, Sonnet, Haiku), both read from the model ID (`claude-<tier>-<major>[-<minor>]`, a trailing date ignored), then every other ID alphabetically.
An agent's model is offered Claude Code's aliases (picking one stores the alias, which follows Claude Code to newer models) and other models' full IDs, but not Claude Code's `default`: an agent is created with a model chosen. A tier, which is what an alias resolves to, is offered full IDs only.
The menu is as wide as its field: in the New Agent dialog's narrow column a row shows only its name (its ID when it has none); a tier's row has its name, with the stored alias beside it, on the left and its full ID on the right, in a monospaced column as wide as the longest ID (`showsModelIDs`).
The New Agent field starts as `opus`, the latest Opus by definition, whatever the profile's default model; a model is required (Create is disabled, and the note says so, while the field is empty).
The field works at once and takes any name: no suggestions (none yet, or Claude Code couldn't be asked, a warning in the log, never a banner; a test run that names no fake CLI, `DEVORA_TEST_CLAUDE_PROGRAM`, doesn't ask at all, so a test never runs the user's own `claude`) leave it a plain text field, and the note under the New Agent field never calls a value wrong (`opusplan`, which the list doesn't name, says what it does).

## Agent chat

`Store.AgentChat` folds the typed chat subscription for one tab and loads earlier pages with `agent_history_before`.
The initial projection is the core's 200-item tail.
Reading older history preserves a sequence/offset anchor, while following the bottom trims automatic live growth to 200 items.
Permission answers carry the observed generation and request ID, and the facade validates that generation and the question's continued existence.
Command+Y, Command+N and Command+Shift+Y answer the first active question through the tab layer, from the editor or transcript.

Enter and Shift+Enter insert newlines by default.
Command+Enter and Control+Enter send, and `agent.enter-sends` in the Settings Hub additionally enables unmodified Enter.
The setting resolves against the agent's owning profile and refreshes when its editor regains focus.
Drafts write through named core commands after 250 ms, with explicit flushes before send, tab close, deletion and Quit.
A failed send keeps the draft, and a successful send clears only the submitted text if it is still unchanged.
A failed flush cancels close or Quit and surfaces the storage error.
Drafts and sequence/offset anchors use the canonical profile and space name, so closing and reopening the tab keeps them.
Stop interrupts with no keyboard binding.
Pause reserves a stop after the running turn, rejects new sends while pending, and leaves permission answers available.
The facade queues `SpaceHandle::pause_after_turn` before returning, and the space actor makes the pause-versus-compaction decision.
`AgentWidgetBar`, below the editor, holds the content-sized Context and Pause widgets on the left and Send on the right, whose tooltip and accessibility hint carry the Enter hint.
A running turn replaces Send with Stop in that position.
`AgentPauseWidget` maps the typed phase and pause flags through `AgentPausePresentation` into Pause, disabled Pausing…, Resume, or a system-colored Paused capsule, with the global resume hint in its tooltip and accessibility description.
Keep paused is a native reversible switch that selects whether this globally held agent remains paused when Devora resumes.
The widget uses native macOS controls and system colors, and exposes each state and its available actions to accessibility.
Agents paused before the global hold show the switch on; once stopped, turning it off makes them resume with the others when Devora resumes (one stored pause flag; see `project-core/CLAUDE.md`).
Its `AgentContextWidget` shows the actor's typed context usage as a percentage of the window and a ring, starting at 9 o'clock, that fills as the usage nears the compaction threshold, with the full numbers in its tooltip and the user's locale for both; an unknown usage or window grays it out and its tooltip says which is missing.
Clicking it opens a native popover (`AgentContextPopover.swift`): the numbers, this agent's compaction threshold as a slider from 30 to 100% that applies on release (Reset follows the profile again), and Compact Now, which runs the threshold's steps now or, during a turn, as Compact After This Turn.
`AgentContextPresentation` decides what it shows and allows: both controls need a running agent and are disabled with the reason otherwise, a configured threshold under 30% shows clamped with a note, and a threshold already passed on an idle agent says "Compacts after the next turn", since nothing starts until a turn ends.
An agent with its own threshold stored (the context's `ownThresholdPercent`, even when it equals the profile's) shows a glyph beside the ring, and its tooltip names both thresholds.
The widget keeps one width whatever it shows (the text reserves its widest forms, the glyph its place): the popover is anchored to it, and a moved anchor makes SwiftUI present the popover anew, as a new window.
Retry and Resume reopen the existing tab against its owning profile.

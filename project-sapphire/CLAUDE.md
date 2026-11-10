# Devora Sapphire

The native macOS build (ADR-006, ADR-010): SwiftUI and AppKit over the shared Rust core (`../project-core/`), bound through UniFFI by the `devora-ffi` crate.
The app is one window: the active tab's terminal (libghostty) over a tab bar, under the window's surfaces: the Agent Hub (`Ctrl+S`), the Workspace Hub (`Ctrl+S` again) and the others.
It reopens the tabs it had when it last quit, crashed or was killed, without the Workspace Hub; with no tabs to reopen, it opens on the Workspace Hub, as Ember always does.
`+` opens a shell tab, and the Workspace Hub opens a workspace in a new tab, or creates a task in one.
Keys go through ADR-003's layer routing (`Sources/Layers/CLAUDE.md`): `Ctrl+S` opens the Agent Hub, and inside either hub switches to the other, `Ctrl+←/→` switch tabs, `Ctrl+Shift+←/→` move the active tab, `Ctrl+Shift+S` opens a shell, and `Ctrl+1/2/3`, `Ctrl+=` and `Ctrl+Shift+=`/`-` set the terminals' font size; every other key reaches the terminal.
`F1` opens the User Guide above any window surface; it renders Markdown in the shared, local-only web renderer and closes with `q` or Escape.
Double-tapping Shift opens the searchable Command Palette when no window surface is open; it offers workspace, session, profile and settings actions, including repurposing an idle workspace.
Closing the window quits the app.

## State and terminals

- The tabs are core-owned state (`devora-state`, ADR-008): the store dispatches a `StateCommand` to the core and changes `Store.app` only by folding the core's events.
	The one early fold is a profile selection's own `ActiveProfileChanged`, applied as soon as `SetActiveProfile` succeeds so that the hub and the New Task dialog see it once the workspaces are listed; the core's copy of that event folds later and sets the same value.
- The core commits every command to `state.db` (SQLite) before its events go out, in `~/Library/Application Support/Devora Sapphire/`, or `Devora Sapphire <suffix>/` for a bundle with an install suffix; `DEVORA_STATE_DIR` replaces the directory (tests).
	The core resolves that path (`sapphireStatePath`, from the bundle's `Resources`); `Store(configPath:state:log:)` takes `.app` for it, `.file` or `.inMemory` (tests).
	A failed command, including one that couldn't be saved, takes no effect and shows as an error banner.
- The app state has one writer: the core holds `state.db` exclusively while it runs, so a second instance on it gets `StartupProblem.stateInUse` and can only quit.
	A state that can't be opened (corrupt, from a newer build) is `StartupProblem.stateUnreadable`: Quit leaves it untouched, Start Fresh (`Command.startFresh`) renames it, with its WAL, to `state-unreadable-<UTC time>.db` beside it and opens a fresh one.
	Until the problem is answered, the store has no core and `StartupProblemView` fills the window.
- The launch (`RootView`) runs once the state is open: the profiles and the selected profile's workspaces, then the Workspace Hub if the core has no tabs (`Store.hasNoTabs`, read from the core since the restored tabs fold on a later frame); restored tabs get fresh terminals from their specs (a terminal's contents aren't restored), and a workspace creation the app stopped in the middle of comes back as failed ("interrupted").
- The store pulls the core's events through `Batches`, the `AsyncSequence` over a core subscription.
	Long-lived state/error consumers resolve their weak store inside the main-actor hop, so queued delivery cannot keep a released store and its database alive. Cancelling the consumer closes its subscription through `Batches`.
	Events wait in an off-main inbox until `FrameDriver`'s display link folds them into `Store.app`, once per frame and only while events are pending.
	Without a display link (tests), they fold on the main actor's next turn.
- `Terminals` (in `AppShell`) holds the live terminals, keyed by the terminal id of each tab's pane: it makes one when a tab appears and drops it when the tab leaves the state, which frees the libghostty surface and ends its process.
	The terminal is at the bottom of its tab's stack at once, while its launch (the core's `terminalSpec`) loads; it runs once it has its launch and its view is in the window, when libghostty makes the surface and spawns the shell.
	What is typed into it before then (keys, the bridge's lines, a paste) waits in its `TerminalSession` and goes to the surface in order once it exists; the PTY holds it until the shell reads it, as in any terminal.
	Every tab's terminal stays in the view tree, so switching tabs keeps them running; a terminal whose process exits closes its tab, as in Ember.
	A tab creating its workspace has no terminal until the workspace exists (its session is `CreatingWorkspace`, which has no terminal spec).
- `Surfaces` (in `AppShell`) holds the key router and one layer stack per tab, with the tab's terminal as its bottom layer, under the progress page of a tab creating its workspace; the active tab's top layer has the keyboard focus unless a window surface is open.

## Preview panes

`debi preview <file>` sends a request to the core's loopback IPC server, whose port is supplied to each terminal alongside its stable pane ID. The core resolves the owning tab, commits an OpenPreview command and delivers PanesChanged through the normal subscription and per-frame fold. Unknown terminal IDs are warnings and no-ops. Preview panes are part of the tab's row, beside its terminal layer; opening one does not activate a background tab or take its terminal's focus. Closing one returns focus to the active tab's terminal unless a window surface owns it.

Paths, pane IDs, display order, preferred widths and refresh revisions are SQLite-backed core state. Restoring a tab re-reads each file; missing or unreadable files show an in-pane error. A new path replaces all existing previews unless `--stack` appends it; any already-open path refreshes in place, preserving its ID, width, position and siblings with or without `--stack`. Markdown and themed Mermaid use WebContentView's revision-driven update path, shared with the guide. Rendered contents, web views and error text are transient UI models. HTML uses a separate HTMLPreviewView with a fresh nonpersistent WKWebView configuration, content JavaScript disabled and no renderer schemes, user scripts or message handlers. It installs a block-all content rule list before reading/loading inline HTML; setup failures never load the document and appear in the pane. An initial CSP permits inline styling but blocks scripts, resources, frames, connections, base URLs and forms. A nil base URL grants no directory access, the navigation delegate only admits the app's initial about:blank load, and popups are refused. HTML is static: linked images/styles/fonts and navigation are unavailable. Refresh and restore reuse the durable pane/file-error flow. `PreviewDivider` dispatches one ResizePreview command when a mouse drag ends, while the row shows the dragged width until the committed one folds; StackPreview and ResizePreview use the same PanesChanged subscription/fold as open and close. The core validates widths (200–16,384 logical points, default 320) and exports those bounds plus the terminal's 240-point minimum through previewLayoutRules. The terminal absorbs spare width. PreviewRowLayout temporarily constrains preferred widths in a smaller window without overwriting them; when even the minimum widths cannot fit, the row scrolls horizontally. The terminal stays in the same native view hierarchy through opening, resizing and closing panes; real UI tests check its shell PID as well as keyboard focus.

The acceptance tests build a real Debi CLI with `test-tools` and run it inside the session's terminal, exercising its environment and the IPC server. Shared Markdown scenarios live in `acceptance/features/preview.feature`; the shared HTML scenario describes sandboxed content rather than an iframe; all preview scenarios now run on both builds. Sapphire additionally probes blocked scripts, local/remote resources, navigation/popups/forms, and HTML refresh/restore/format changes. Preview restore, missing-file and background-tab scenarios are Sapphire-specific. The test bridge exposes pane identity/error state and rendered DOM/SVG observations; the UI tests drag dividers, relaunch, check minimum widths, close stacked previews and verify the same shell process receives subsequent typing.

## Workspace Hub

Ember's home page (`Sources/AppShell/WorkspaceHub/`): the selected profile's workspaces, beside the focused one's details, on a page layer over the whole window.

- `Surfaces.openWorkspaceHub` pushes it with a fresh `WorkspaceHubModel`, which holds what one opening has (the category, the filter, the cursor, the toast), so closing the hub forgets them, as Ember's `unload` does.
	The profiles, the workspaces and their statuses are the store's; the hub loads them on opening, reloads them silently when a page over it closes (its `onReveal`), and reloads them with a toast on `R` ("Refreshing…" for at least 1 s, then "Refreshed successfully" for 3 s unless a call failed; the latest press wins).
- The statuses (`Command.loadWorkspaceStatuses`) are one `workspace_status` core call per workspace, off the main actor and four workspaces at a time (each runs its repos' `git status` in parallel, at most five at once), the active workspaces first; each is kept as it arrives, so the list and the detail panel fill in progressively, and a workspace without one shows as pending.
	A workspace whose status can't be read is invalid (an error dot, badge and message), with no banner; a failed call also has its banner.
	Dropping the statuses (a fresh load, another profile) makes the loads still running drop theirs.
- Its keys are in `WorkspaceHubModel.handleKey`; while the filter field has the focus, keys type into it and Escape leaves it, as Ember's editable-element guard does.
	The cheatsheet (`?`) is a page of its own over the hub, scrolled by `VimScroll`, ADR-004's motions over a SwiftUI scroll view; `ListCursor` is the list's cursor.
- The detail panel's buttons are Ember's: Open; Duplicate and Remove Task for an active workspace; Delete for an inactive or invalid one.
	An active workspace also has Add Repo, a Sapphire addition alongside the Command Palette action (see Repos).
	Remove Task (`Store.removeTask`, `remove_task`) goes ahead at once when every repo is known to be clean and on a detached HEAD (`RemoveTaskDecision`), and otherwise asks with a confirmation dialog listing what will be lost; Delete (`Store.deleteWorkspace`, `delete_workspace`) never asks.
	Either one lists the workspaces again, keeping the filters and the cursor; a failure shows its banner.
- The confirmation dialog (`ConfirmationDialog.swift`, `Surfaces.confirm`) is a modal layer over the window, with Ember's keys: Escape and `q` cancel, Enter confirms, Tab and Shift+Tab move between its two buttons without ever leaving it, and Space presses the focused one; it draws its own focus, so the Tab trap doesn't depend on the system's keyboard navigation setting.
- The header has Ember's two dropdowns (`DropdownMenu.swift`): the profiles (each with its repo count, the selected one checked), and the burger menu, whose "Save loading latencies" writes the latest load's `LoadingLatencies` to `<profile>/diagnostics/hub-profile-<timestamp>.json` and shows the path in a toast.
	An open dropdown is a popup layer (`Surfaces.toggleHubMenu`) whose content is empty: the hub draws the panel under its trigger, as Ember's caller-mounted popup, with a click anywhere else closing it.
	Escape and `q` close only the dropdown; any other key closes it and reaches the hub.
- `P`, the profile dropdown's "Manage Profiles…" and the burger menu's "Settings" open the Settings Hub, and "New Profile…" opens it on its New Profile row (see Settings Hub).
- `H` and the burger menu open the Health Hub as a window-stack page. It runs the shared core health check off the main actor, cancels it on close or re-run (`r`), supports Vim scrolling, and copies fix hints with a toast.

## Agent Hub

The Agent Spaces of the selected profile (ADR-012, `Sources/AppShell/AgentHub/`), on a page layer over the whole window, with the Workspace Hub's keyboard model.

- `Ctrl+S` (`Surfaces.hubShortcut`) opens it when no other window surface is open; inside either hub it switches to the other, as the Agents | Workspaces toggle in both headers does (`HubSwitcher`), except while the Workspace Hub shows the first-run welcome.
	The Command Palette's "Agent Hub" opens it too.
- `AgentHubModel` loads the profiles, then lists the spaces (`Store.listAgentSpaces`); invalid ones (an interrupted creation, an unreadable database) show their error and can only be deleted.
	Its keys: `j`/`k` and the arrows move, Enter opens, `n` creates, `d` deletes (with a confirmation; an open agent is paused and its tab closed first, since the core deletes only a stopped one), `R` lists again.
- The New Agent dialog (`NewAgentDialogModel`) is a modal over the hub: the name, with the space's path under it, the template's fields (`agent_creation_fields`: the ones only the form fills, then the pre-filled ones with their source, then the derived ones, read-only, listed again as the name and repos change), the initial repos, the model and the effort.
	Fields show user-facing labels (`AgentFieldLabel`) with their `{{placeholder}}` and source beneath; `agent.name` and `space.path` aren't listed, as the Name field and the path under it show them.
	Editing a pre-filled field changes only this agent; its Save button writes the value to its source (`save_agent_field`: the global config for a global value, otherwise the profile).
	The model is free text, starting at `opus`; the effort is a stepped slider from "Claude Code default" (no `--effort`) to max, starting at the effort the user set for the profile or in User Defaults (`agent_model_defaults`).
	Both are stored with the agent and used for every launch.
	"Include coding instructions" (on by default) appends the instructions to Claude Code's system prompt, keeping its coding instructions; off, they replace it.
	The Preview beside the form shows the instructions as creation would render them (`agent_creation_preview`), or why it would fail, updated as the form changes.
	Create shows the creation's progress (`Store.createAgentSpace`) in the dialog's place, cancellable; once created, the agent opens in its tab and the hub closes.
- Opening an agent (`Store.openAgentSpace`) asks the core for the launch (`resolve_agent_launch`, the user's login shell), then `open_agent_space`, which opens its tab or activates the one it has.
	A launch resumes the agents of restored tabs in the background (`restore_agent_spaces`), and quitting pauses every agent first (`AppTermination.pauseAgents`, from `applicationShouldTerminate`).
- An agent's tab shows its status emoji (`AgentStatusEmoji`: ⚙️ working, turning slowly unless Reduce Motion is on or the window inactive, ❓ waiting for the user, 💤 idle, ⏸️ pausing, ⚠️ failed), folded off the main actor from the core's chat events into `Store.agentStatuses`.
	Its tab's stack holds `AgentChatPanel`, with the transcript (the shared local web renderer, or the native one behind its switch, see Agent chat) above an AppKit plain multi-line editor.

## Agent chat

`Store.AgentChat` folds the typed chat subscription for one tab and loads earlier pages with `agent_history_before`.
The initial projection is the core's 200-item tail.
Reading older history preserves a sequence/offset anchor, while following the bottom trims automatic live growth to 200 items.
The local web renderer keeps sequence-keyed rows, morphs sanitized Markdown during streaming, and retains expanded thinking/tool details.
Kind dots distinguish user, assistant, tool, thinking and system items, and each row's hover title shows its sequence.
Permission buttons are host-created DOM controls and send the observed generation and request ID through the restricted WKWebView bridge.
The facade validates that generation and the question's continued existence.
Command+Y, Command+N and Command+Shift+Y answer the first active question through the tab layer, from the editor or transcript.

The native transcript (ADR-014, `Sources/AppShell/AgentHub/Transcript/`) replaces the web one when the hidden `DevoraNativeTranscript` default is on (`defaults write <bundle id> DevoraNativeTranscript -bool YES`, or `-DevoraNativeTranscript YES` as launch arguments, which the XCUITests use; `mise run test-acceptance-native` runs `agent-chat.feature` again with it, except the `@permission-card` scenarios); the web transcript stays the default until it is the only one.
`TranscriptScrollView` is an `NSTableView` of `TranscriptCell`s hosting SwiftUI `TranscriptRow`s, fed `AgentChat.items`; `TranscriptChange` turns each update into a prepend, changes in place with rows added or dropped at either end, or a reload.
It follows the bottom while the reader is within 24 pt of it, keeps the top visible row in place otherwise, asks for an older page within 80 pt of the top (saving the anchor first, so the chat stops trimming to its tail), and reports the anchor 250 ms after the reader's scrolls, with the web transcript's sequence/offset meaning; its own adjustments don't count as the reader's.
`RowHeights` holds the heights: exact for a row measured at its content and width, otherwise an estimate (its last height, or a guess from its kind and text calibrated by the measurements so far), so `NSTableView` never waits for a measurement.
The visible rows are measured at once; the rest on idle frames, nearest first, 2 ms a frame, after which the display link stops; a width change makes every height an estimate again; every height change applies without animation and keeps the bottom in place, or else the first visible row that was already exact (so a row scrolled into view settles toward the side not yet seen), or a restored position's row.
Links open only `http` and `https` URLs, as in the web transcript.
Wide code blocks scroll sideways; a mostly vertical scroll gesture over the transcript scrolls the transcript wherever it starts, a code block included, decided once per gesture and kept through its momentum (`WheelRouting`, through a local scroll-wheel monitor that takes only events whose hit test lands in the transcript, so not over a background tab or a layer drawn over it).
User, assistant and thinking rows' Markdown is parsed once per content by `MarkdownDocument` (`swift-markdown`) into plain blocks and inline runs that `MarkdownView` draws (a collapsed thought only once it opens); Mermaid is still a code block, and there are no permission cards yet.
Thinking and tool rows start collapsed, as the web transcript's `<details>`: a summary ("Thinking", or the tool's name and its Judgess badge) that a click toggles, keeping the row's top in place; the expansion is the view's, by sequence, so it survives updates, cell reuse and older pages, and `RowHeights` keys a measurement by expansion too.
`KindMarker` draws the web transcript's kind dots and the amber edge of an interrupted or failed item, in one place for the coming chat bubbles to replace.
Each row's context menu has Copy, of the item as written (Markdown included; a tool's name, input and result).
For VoiceOver each row is one element in reading order: its kind and state as the label ("Tool Bash, Judgess approve, collapsed"), its shown text as the value, and Expand or Collapse and its links as actions; a cell the table removed for reuse shows nothing, so it exposes no stale row.
The bridge's `GET /test/agent-chat` answers from the native view (`rows` with the web's `chat-<kind>` kinds, `thinkingOpen`, `toolOpen`, `verdict`, `top`, `topSequence`, and no scripts or unsafe attributes, since nothing native runs content); `toggleThinking`, `toggleTool` and `scrollTop` act on it as the reader would; only `cards` is still 0.

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
`AgentWidgetBar`, below the editor, holds the widgets at their natural size on the left and Send on the right, whose tooltip and accessibility hint carry the Enter hint.
Its `AgentContextWidget` shows the actor's typed context usage as a percentage of the window and a ring, starting at 9 o'clock, that fills as the usage nears the compaction threshold, with the full numbers in its tooltip and the user's locale for both; an unknown usage or window grays it out and its tooltip says which is missing.
Retry and Resume reopen the existing tab against its owning profile.


## Global Pause

The title-bar toolbar and command palette dispatch `Command.setGlobalPaused` through Store.
`AppState.globalPaused` is the core's durable, per-install hold, folded from the ordinary state subscription.
The window shows one Paused hint while held.
An agent tab shows Pausing… until its current turn ends and Paused afterward, using the paused emoji and matching accessibility description.
`AgentChat.globallyPaused` disables sending and exposes the resume hint while drafts keep their normal persistence.
The core also refuses sends during the hold.
Permission responses and Stop remain available for a draining turn.
The chat's Keep Paused action converts a global hold into an individual pause for that run.
The core-owned chat `globallyHeld` projection selects this action.
The initial restored snapshot carries the global hold before agent restore launches work.

## Profiles and the first run

Ember's profile form and first-run welcome (`Sources/AppShell/Profiles/`).

- The profile form (`ProfileFormModel`) has a name and a root path, with "Browse…" (an `NSOpenPanel` folder picker); it validates the path (`validate_profile_path`) 250 ms after the last edit, or at once when it is set (the initial path, a browsed folder), as a new profile, an existing one (whose own name locks the name field, and "Register Profile"), or an invalid path.
	Submitting registers it (`register_profile`, which creates `config.json`, `repos/` and `workspaces/` for a new one), then the hub selects it and loads it from scratch.
	Enter submits from a field or the submit button, Space presses the focused button, and Tab and Shift+Tab move through it, skipping a locked name and a disabled submit; it draws its buttons' focus itself, as the dialogs do.
- With no profiles, a listing that succeeded makes the Workspace Hub show the first-run welcome (`WorkspaceHubModel.welcome`) instead of its list: the profile form, on `~/devora`, with no control focused, as in Ember.
	The hub refuses `q` and Escape meanwhile (`dismissDecision`), as Ember's zero-profile lock does; Escape in a field leaves the field, and the hub's own keys do nothing.
	The burger menu has no "Settings" and the header no profile dropdown, as in Ember.
- The selected profile is the core's active profile (`AppState.activeProfile`, which `Store.selectedProfilePath` reads): selecting one dispatches `SetActiveProfile`, so a launch restores it, where Ember always starts on the first profile.
	A load selects the restored profile while it is registered, otherwise the first profile, or none, and dispatches that in its place; it reads the active profile from the core, since the state's events fold on a later frame, and folds its own selection at once, so that the New Task dialog and the hub see it as soon as the workspaces are listed.

## Settings Hub

Ember's Settings Hub (`Sources/AppShell/Settings/`): the profiles beside the focused row's detail, on a page layer over the Workspace Hub (`Surfaces.openSettingsHub`), so that closing it (`q`, Escape, or what closes it below) reloads the hub.

- `SettingsHubModel` holds one opening: the master list ("User Defaults" pinned first, the store's profiles with the active one's dot and their repo counts, "New Profile…" pinned last), its cursor, and the focused row's detail; it lists the profiles again on opening (`Store.listProfiles`) and focuses the active profile's row, or the New Profile row for "New Profile…".
	Its keys are Ember's: `j`/`k` and the arrows move, Enter sets the focused profile active, `n` (or Enter on the New Profile row) gives the form's name field the focus on the main actor's next turn, and `d` deletes; a key typed into a card's input or the form's fields is theirs (Escape leaves the field), and `q` and Escape go on to the stack's dismissal.
- A profile's detail: its path, its repo, workspace and active-task counts (`get_registered_repos` and `list_workspaces` the first time its row is focused), Set Active, Clone Repo (the Clone Repo dialog, after which the counts and repos load again) and Delete, and its Repos card.
	Set Active and registering a new profile make it the core's active profile at once (`Store.activateProfile`) and close the Settings Hub; the Workspace Hub's reload then lists its workspaces, starting its cursor over.
	Delete asks with Ember's confirmation (the directory "remains on disk"), or shows a notice (`Confirmation` with no `cancelLabel`) listing the open tabs that use the profile (`Store.tabs(usingProfile:)`); `Store.unregisterProfile` (`unregister_profile`) then loads again, which selects the first profile if the deleted one was selected, and deleting the last one closes the Settings Hub over the first-run welcome.
- The New Profile row's detail is the profile form, whose Cancel goes back to the active profile's row; Tab and Shift+Tab move through it while it has the focus.
- The User Defaults' and each profile's detail have Claude Launch Settings, Terminal & Session, Pull Requests and Task Tracker cards; the Asana settings and token row show while the provider resolves to Asana, an inherited one included.
	`ClaudeSettingsModel` edits the three model tiers and permission mode as Custom/Default/None, effort as Default/levels/None, and Default model as On/Off (On stores `opusplan`, Off stores explicit None).
	Custom reveals a draft seeded from the resolved value without writing; Enter or Save commits trimmed text, a blank commit inherits again, and drafts survive unrelated writes.
	The core exports the effort levels and model/permission suggestions from `devora-config`; neither UI owns a copy of those lists, and suggestions never restrict custom ids.
	`SettingsWriteQueue` serializes generic and Claude writes together because both update the same config file.
	`AsanaTokenModel` reads only presence and clears its secure draft after a successful save; the credential is shared by both scopes and Debi, stored as service `devora-asana` for the login account.
	The acceptance harness's `DEVORA_TEST_MODE=1` selects the core's in-memory credential backend in both Sapphire and Ember; model tests inject closures and Rust tests inject a `TokenStore`, so tests never read or modify the real login keychain.
	`ConfigSettingsModel` is Ember's `settingsEditor` for one scope: it reads the scope (`get_config_settings`, the stored and the resolved values), writes one key at a time, in order (`set_config_setting`, where nil removes the key), and reads the scope again after each write.
	"Default" removes the key, and shows the inherited value as a hint ("→ value", "Uses Devora default" when nothing applies; not for `pr.auto-merge`, whose per-repo override the hub can't see).
	A text setting's Set reveals an input and focuses it, writing nothing; Enter or its Save button (shown only for a non-blank value that differs from the stored one) commits, trimmed except for the title prefix, and a blank value reverts to Default; what is typed stays across the reloads of other settings' writes, and comes back when Set is selected again.
	The Agent card's `AgentSettingsModel` shares that write queue and scope reload. It edits compaction percentage, a template path with a file picker, multiline role/user-details/custom-sections and literal key/value template inputs. The existing Enter sends control is in the same card. Multiline Enter stays text and Save preserves whitespace and explicit empty strings. Default removes an override. Core-returned sources label effective values, and explicit legacy nulls remain visible as cleared overrides. Each input rename is one core write, and its row is disabled while its write is pending. A legacy null inputs map can be reset to Default before editing entries.
	A profile's Agent card previews saved instructions through `agent_creation_preview` with an empty form, coding instructions included, and that card's owning profile. Each successful queued write rereads the settings and refreshes the preview. Unsaved drafts stay local. Preview failures are inline, and an older render cannot replace a newer reload. User Defaults points to the profile cards rather than rendering a prompt without a profile.

## Task creation

Ember's non-blocking task creation (`Sources/AppShell/TaskCreation/`): the New Task dialog hands the creation to a tab of its own, whose progress page shows it until the tab has its workspace.

- `n` and "+ New Task" in the Workspace Hub open the New Task dialog (`NewTaskDialogModel`, `Surfaces.openNewTaskDialog`), a modal layer over the hub, once the profile's repos have loaded (`get_registered_repos`); Duplicate, on an active workspace, opens it titled "Duplicate Task", with the task's title and repos.
	Its keys are Ember's: typing goes to its focused field (the title, then the repos' filter), Escape cancels and Enter creates, except that Enter toggles the active repo while the filter has the focus, where ↑ and ↓ move it; Tab and Shift+Tab cycle the title, "+ Clone Repo", the filter, Cancel and Create, and Space presses the focused button, whose focus the dialog draws itself.
- Creating closes the dialog and the hub, then dispatches `Command.createTask`, and the core takes it from there (`create_workspace`, in `devora-ffi`'s `creation.rs`): it opens the tab as `CreatingWorkspace`, runs the creation on a worker thread, and records its outcome in the app state: `WorkspaceCreated`, `WorkspaceCreationFailed` (also reported to the log, so it has its banner), or closing the tab after a cancel, once the work is undone.
	The store pulls the creation's progress (`CreationEvent`s, in batches) into `Store.creations`, by tab, folded by `CreationProgress`; a tab's progress goes when the tab has its workspace or closes.
- The progress page (`CreationProgressPage`) is a page on its tab's own stack, pushed as soon as the tab is in the state, so it shows only while its tab is active and never blocks the tab shortcuts: the steps, a collapsible Output log of the latest 500 lines, which a failure opens, the failure, and Cancel, or Close once the creation has failed; Escape and `q` do what the button does, whatever has the focus.
	When the tab has its workspace, its terminal goes in at the bottom of the stack, under the page, which is then removed, revealing it (`Surfaces.retireProgressPages`).
- `Command.cancelCreation` cancels a tab's creation (`cancel_workspace_creation`); one asked for before `create_workspace` has returned its id is held and applied once it has, and closing the tab of a creation still running cancels it.
- A creation the app stopped in the middle of comes back as `CreationFailed` with the reason "interrupted" (the core's restore), so its tab shows its progress page with that failure and Close.

## Repos

Ember's Add Repo and Clone Repo dialogs (`Sources/AppShell/Repos/`), modal layers over the window, each with its form, then the operation's progress in its place (`RepoOperationPhase`, as Ember's `driveCreationDialog`).

- Clone Repo (`CloneRepoDialogModel`, `Surfaces.openCloneRepoDialog`) opens from the New Task dialog's "+ Clone Repo", cloning into the selected profile; its "Clones to:" hint is the core's `clone_destination`, which parses the URL as the clone does.
	Once cloned, the New Task dialog lists the profile's repos again with the clone selected, keeping its title and selection (`NewTaskDialogModel.reposChanged`, Ember's `refreshAfterClone`).
- Add Repo (`AddRepoDialogModel`, `Surfaces.openAddRepoDialog`) adds a worktree of one of the profile's repos to a workspace, as `<repo>` or `<repo>-<postfix>` (`AddRepoTarget.worktreeDirName`); the core rejects a name the workspace has or one that leaves it, as the add's failure.
	It takes an `AddRepoTarget`: the workspace and its own profile, whose repos it lists and whose prepare-command runs, whichever profile is selected.
	The Command Palette opens it on the active tab's workspace (`Store.addRepoTargetOfActiveTab`, which refuses a tab without a workspace with Ember's banner); the Workspace Hub's Add Repo opens it on the focused workspace, then reloads that workspace's status.
- Their keys are Ember's: Escape cancels and Enter confirms whatever has the focus, `q` cancels outside a field, Tab and Shift+Tab cycle the fields and buttons, and Space presses the focused button; during the progress, Escape, `q` and Space press its Cancel, or Close once it has failed.
- `Store.startRepoOperation` starts either (`add_repo_to_workspace`, `clone_repo_into_profile` in `devora-ffi`'s `repos.rs`) and follows its progress into a `RepoOperationRun`; done or cancelled closes the dialog, and a failure, which also has its banner, stays in it until it is closed.
	A cancel is `cancel_workspace_creation` (the operations' ids are a creation's), held until the id has arrived; the core undoes the work (the added worktree, the partial clone) before it reports the cancel.

## Errors and the log

Sapphire follows Ember's error reporting (ADR-002).

- The log is `/tmp/devora-sapphire-<YYYYMMDD-HHMMSS>.log`, `[timestamp] [LEVEL] message` lines, opened at launch through `AppLog.openSession()` (`devora-ffi`, over `devora-app`), which prints its path to stderr and writes every Rust panic to it as a `[CRASH]` line.
	The core writes its own warnings to it (a terminal setting that couldn't be read); the store's `logWarning` and libghostty's config diagnostics (`TerminalRuntime.logWarning`) go there too.
- `Store.showError` is THE way to surface an error: it records it for the tests (`scrapeErrors`), writes it to the log, and shows it on a banner.
	Banners stack at the top of the window above everything else, the layer stacks and the startup problem included (`ErrorBannerStack`); an identical message counts on its banner (×N) while it is up, and × dismisses it (`Command.dismissError`).
- A failing core call shows `<call> failed: <error>`, by the core call's Rust name (`list_workspaces failed: …`), as Ember's `invoke` does; the store's `call` wraps every one.
	Nothing opts out to log-only yet, as Ember's `invokeLogOnly` does for its frequent calls.
- An error the core reports (`AppLog::report_error`, e.g. a corrupt `task.json` while listing workspaces) is already in the log, so it only gets its banner.
	It reaches the store through `AppLog.errors()`, a subscription pulled in batches like the state's events (ADR-010's facade rules), not a callback; one stream per log, so errors reported before the store pulls wait for it.
- A Swift error has no uncaught path: every core call is wrapped, and the store's event pulls show their own failure.

## Layout

- `Sources/CoreBindings`: the Swift bindings for `devora-ffi`, generated by `mise run core` (gitignored).
- `Sources/Store`: the one `@MainActor @Observable` store; views dispatch `Command`s to it, and only it calls `DevoraCore`.
- `Sources/TerminalKit`: the terminal, over Ghostty's internal embedding API; the only module that imports `GhosttyKit`, with `TerminalPane` as its view entry point, `TerminalSession` as its model and `TerminalLaunch` as what it runs.
- `Sources/Layers`: the layer system (ADR-003): the stacks, the key router and the presets, with no surface or domain knowledge; its `CLAUDE.md` is the current reference.
- `Sources/AppShell`: the views, the tab bar, the terminals, the app-wide shortcuts, the layer stacks' wiring and the display-link fold.
- `App/`: the thin app target; `project.yml` is its XcodeGen spec (the `.xcodeproj` is gitignored).
- `Tests/StoreTests`: the store and the event fold against the real core, over a fixture config, task creation, Add Repo and Clone Repo over real git repos, and the creation progress's fold.
- `Tests/TerminalKitTests`: the command line libghostty runs.
- `Tests/LayersTests`: the routing and focus rules, on stand-in layers.
- `Tests/AppShellTests`: which key-downs are app-wide shortcuts, the Workspace Hub's list, cursor, scrolling and keys, the Remove Task decision, the confirmation dialog's keys, the New Task dialog's selection, filter and keys, the repo dialogs' forms, keys and clone hint, the profile form's validation and keys, and the Settings Hub's commit rules, inherited hints and list keys.
- `acceptance/`: the acceptance scenarios' Cucumber.js harness (steps, hooks, the bridge client) and Sapphire-only features; `../acceptance/features/` holds those shared with Ember.
- `UITests/`: the XCUITests, a UI-test target in `App/project.yml`.
- `scripts/build-core.sh`: `devora-ffi` → static library → bindings → `build/DevoraCoreFFI.xcframework`.
- `build/` and `vendor/` (gitignored): the xcframeworks, DerivedData, and the Ghostty checkout.

## Repository Updates

The Repository Updates card appears only in User Defaults.
It edits the shared core's enabled flag and positive interval in minutes, with Sapphire's enabled default shown in the inherited hint.
Interval values are edited as text and stored as JSON numbers by the core.
Run Repo Updates in the palette requests a manual round even with the periodic switch off.
It uses the same Global Pause boundary as automatic rounds.
The launch starts the single core worker, and the off-main termination closure shuts it down before awaiting the agents.
Operational outcomes go to the app log without banners.

## Commands

- `mise run bundle` (or root `sapphire-bundle`) builds the signed distribution with the shared tool/plugin/resource manifest into `build/bundle/Devora Sapphire.app`. Root `sapphire-install --path <directory> --suffix <name>` installs it; the suffix changes the visible name and core state directory. Plain `build` remains the development app, without the distribution resources. Health retains the core's PATH fallback and the guide retains its checkout fallback there.
- `mise run test-bundle` packages the existing `build`/`test-tools` outputs, runs shared `@bundled-tools` acceptance against that signed app, then smoke-tests the distribution through the existing acceptance launcher and bridge: signatures, core launch PATH prefixes, real shell tool resolution, suffix state/name and F1 loading a uniquely marked bundled guide in an isolated copy. It uses a temporary HOME and does not touch developer state. It is the last stage of `mise run test` (also CI), packaging with `--prebuilt` to reuse its app and Debi test-tools. Login shells' `path_helper` may put system directories first, which is accepted (#277); the smoke checks core launch-spec prefixes and actual command resolution without asserting login-shell PATH order. `GET /test/guide` observes the open guide's DOM text/error.
- From the repository root, `mise run lint` checks handwritten Swift for tab indentation (including new, nonignored files) as well as Rust formatting and clippy. Generated bindings and ignored build/vendor files are excluded.

- `mise run build`: the core, the bindings, the Xcode project and `build/DerivedData/Build/Products/Release/Devora Sapphire.app`; it builds GhosttyKit first if `build/GhosttyKit.xcframework` is missing.
	The development app's bundle id is `dev.devora.sapphire.dev`, so the XCUITests, which quit every running instance of their target's bundle id, never quit an installed Sapphire (`dev.devora.sapphire`, stamped by `populate-app-resources.sh` and checked by `test-bundle`'s smoke).
- `mise run run`: launches the app through LaunchServices (`open -n`); arguments are `--flag value` pairs, since a bare argument makes AppKit open no window.
	`--open-workspace <path>` opens a workspace of the selected profile in a new tab; `--surface workspace-hub` opens the Workspace Hub.
	`--snapshot <out.png>` types a command into the first shell tab, if there is one, captures the window to `<out.png>` and the active tab's text to `<out.png>.txt`, then quits.
- `mise run test`: every test, in order: `test-unit` (`swift test`, the store against the real core), `test-agent-cli` (the pinned Claude CLI against the offline backend), `test-acceptance` (the scenarios over the test bridge), `test-ui` (the XCUITests), then `test-bundle` (package and smoke). Acceptance/UI build the app first; packaging reuses it. `test-acceptance` excludes `@bundled-tools`: those scenarios run in `test-bundle`, after packaging once. Git shortcuts and `@real-health` use the bundle binary through the same launcher/bridge, isolated HOME/config/state and the core-built PATH; no dev shim path is injected. Only `@real-health` omits the health report fixture, so it executes the bundled Rust Debi and its real Go health fallback. Ember's normal e2e suite includes `@bundled-tools` unchanged; see `../acceptance/CLAUDE.md` before tagging shared scenarios.
- `mise run test-agent-cli` runs the ignored-by-default Rust agent integration target with this project's Node environment and npm dependencies. `scripts/test-agent-cli.ts` reads the exact Claude CLI version in `.github/workflows/ci.yml`, provisions it with mise Node/npm into the versioned `build/agent-cli/` cache, and passes its absolute path through `DEVORA_TEST_CLAUDE_PROGRAM`. It uses isolated test homes/workspaces and Ember's fake backend, and exercises inline prompt/configuration and compaction contracts. It is a required stage of the standard test command, including the existing Sapphire CI job. Ordinary shared `cargo test` stays CLI-independent.
- `mise run core`, `mise run generate`, `mise run ghostty-fetch`, `mise run ghostty-build`: the build's steps.
	`ghostty-build` needs Xcode's Metal Toolchain and gettext's `msgfmt` (`mise run ghostty-prereqs` installs whichever is missing; it is meant for CI, since it installs system-wide, so ask before running it on a developer's Mac); set `GHOSTTY_CLONE_SOURCE` to clone Ghostty from a local copy instead of GitHub.
- `mise run ghostty-cache-key`: the key CI caches `build/GhosttyKit.xcframework` under (the `sapphire` job in `.github/workflows/ci.yml`).

The app reads the global config from `DEVORA_CONFIG_PATH` when set, as Ember does; `Store(configPath:)` overrides it for tests, except in terminal specs, whose settings `devora-config` reads through `DEVORA_CONFIG_PATH` alone.

## Testing

The shared `claude-hook-integration.feature` runs the real recorded Claude Code CLI in both apps. Sapphire's `acceptance/support/claude.ts` reuses Ember's fake server, cassette files and interactive CLI protocol through a typed terminal adapter; its teardown waits for `/exit` and verified process exit before ending PTYs or deleting workspaces. The background ownership scenario holds the PostToolUse hook on a fixture barrier until the second tab is active. Both desktop suites share the isolated CLI test config and must run sequentially on one machine. `test-tools` builds the real Rust CCC launcher as well as Go Debi; CI installs the same exact Claude version for both apps.

Acceptance tests follow Ember's (ADR-001): Gherkin scenarios run by Cucumber.js against the real, built app, which they drive through a test bridge (ADR-011).
`acceptance/PARITY.md` maps each Ember scenario Sapphire doesn't run to its Sapphire coverage, or lists it as a gap.
Real input (key events, focus, the IME, libghostty's key path) is what the bridge can't drive, and what SwiftUI draws is what it can't see; XCUITests cover both.

- The bridge (`Sources/AppShell/TestBridge.swift`) is a loopback HTTP server that starts only when `DEVORA_TEST_MODE=1`, once the launch is done, and prints `Devora test harness on port <port>` to stderr, as Ember's does.
	It is compiled into every build, as Ember's is, and does nothing unless the variable is set.
	Its routes: `GET /test/state` (profiles, workspaces, tabs with their terminal's status, focus, layers and progress page, the active tab, the window stack's layers, whether a text field has the focus, the open Workspace Hub as it shows (its list, cursor, filters, detail panel, toast, cheatsheet scrolling, profile label, open dropdown's rows and whether it shows the first-run welcome), the open confirmation dialog, the open New Task, Add Repo and Clone Repo dialogs, the open profile form (the Settings Hub's, or else the welcome's), the open Settings Hub (its rows, the focused profile's detail, and each shown setting's segment, input, Save button and inherited hint), the font size, the error banners, the log's path and the startup problem); `POST /test/dispatch` with a store command (`{"command": "openShell"}`, `load`, `selectProfile` with `path`, `openWorkspace` with a workspace `id`, `activateTab` and `closeTab` with a tab `id`, `startFresh`, `dismissError` with a banner `id`); `POST /test/agent-hub` (`{"action": "newAgent"}`, or `open` and `delete` with a `name`) and `POST /test/new-agent` (`setName` and `setField` with a `key` and a `value`, `setKeepCodingInstructions` with a boolean `value`, `toggleRepo` with a `name`, `save` with a `key`, `create`, `cancel`), which click what the Agent Hub and the New Agent dialog draw, whose state is `agentHub` and `newAgent`, and each tab's `agentStatus`; `POST /test/hub`, which clicks what the open hub draws, running what the button runs (`{"action": "removeTask"}`, `delete`, `duplicate` or `addRepo` on the focused workspace, `newTask`, `toggleMenu` with a `menu`, `profiles` or `main`, and `menuItem` with the `label` of a row of the open dropdown, a 409 when it is disabled); `POST /test/dialog` with `{"button": "confirm"}` or `cancel`; `POST /test/new-task` with `{"action": "toggleRepo", "name"}`, `create`, `cancel` or `cloneRepo`, which clicks what the New Task dialog draws; `POST /test/add-repo` with `{"action": "selectRepo", "name"}`, `submit`, `cancel` or `progressAction` (the progress's Cancel or Close), and `POST /test/clone-repo` with `{"action": "setUrl", "value"}`, `submit`, `cancel` or `progressAction`, which do the same for the repo dialogs; `POST /test/profile-form` with `{"action": "setName", "value"}` or `setPath`, which fill in the open profile form's fields as typing does, `submit` (a 409 while it is disabled) or `cancel`; `POST /test/settings` with `{"action": "selectRow", "label"}`, `setActive`, `cloneRepo` or `delete` on the focused profile, or, on a setting of the focused row's scope, `selectSegment` with a `key` and a `label`, `type` with a `key` and a `value`, `commit` (Enter) or `save` (a 409 while its Save button is hidden); `POST /test/layers/clear` removes every window surface, the first-run welcome included, as Ember's teardown does; `POST /test/progress` with an optional `tab`, which clicks a progress page's Cancel or Close; `POST /test/snapshot` with `{"path"}` (captures the window to a PNG); `POST /test/errors/scrape` (the errors shown since the last scrape, which it clears, as Ember's `__scrapeErrors`); `POST /test/quit` (the app menu's Quit; the app may exit before it answers); `GET /test/terminal/text?tab=<id>` and `POST /test/terminal/type` with `{"line", "tab"}` (the active tab when `tab` is omitted; a 409 until the tab has its terminal, which holds the line until it runs); `POST /test/terminal/focus` with `{"tab"}` makes a tab's terminal the window's first responder.
	A tab's `terminal` in the state is `none`, `starting`, `running` (its surface exists, so its shell has a PTY) or `failed: <reason>`.
	`POST /test/key` with `{"key", "modifiers"}` presses a key: it makes the key-down AppKit would (key code, characters, flags) and hands it to `LayerRouter.handle`, the monitor's own entry point, then, unless consumed, to the window's `sendEvent`, which delivers it to the first responder, and answers whether the router consumed it; it skips the event monitor's plumbing and the menu's key equivalents, which the XCUITests cover.
	`POST /test/palette` exercises the shortcut tracker, changes the filter or blurs its field; `POST /test/repurpose` fills or submits the open Repurpose dialog. The state exposes their current models, including the same filtered command table the view uses.
	It starts early when the launch meets a startup problem, so that a scenario can answer it.
	State changes fold on the next frame, so steps poll `state` (`Bridge.waitFor`) rather than reading it once.
- Every scenario launches a fresh app (`acceptance/support/hooks.ts`, `app.ts`), straight from the bundle's binary rather than through `open`, with a config and an app state of its own (`DEVORA_CONFIG_PATH`, `DEVORA_STATE_DIR`) and only a login session's environment, and starts once the launch has settled: the Workspace Hub it opens has loaded, or, after a relaunch, the restored tabs' terminals run.
	Generated acceptance and native UI configs disable periodic repo updates to isolate repo fixtures, and updater tests opt in explicitly.
	A scenario's config starts with no profiles, so the launch's hub is the first-run welcome, which refuses `q` and Escape: steps that need a known state remove it through the bridge ("Devora is running with no sessions open", as Ember's scenarios start after its teardown), and "the Workspace Hub is open" removes an open hub before opening it, so that it loads the profiles the scenario has made since.
	Failed steps save a window screenshot (the bridge's `POST /test/snapshot`), the state, the active terminal's text and the app's stderr to `test-results/failures/` (`acceptance/support/failure-capture.ts`); the shared `failure-capture.feature` checks the screenshot and the terminal text.
	As in Ember, a scenario fails if an error was shown that no step scraped ("the recorded errors should include").
	Relaunches within a scenario (`persistence.feature`) keep its state directory.
	Teardown uses the bridge's normal Quit and waits for owned fake agent children before removing fixtures. A bounded shutdown falls back to signals, retains shutdown failures and preserves fixtures if shutdown fails. Child matching includes the CLI session command to guard against PID reuse.
	Agents run the core's deterministic fake CLI (`project-core/crates/devora-agent/tests/support/fake_cli.py`, through `DEVORA_TEST_CLAUDE_PROGRAM`), never Claude.
- Scenarios shared with Ember live in `../acceptance/features/` (see `../acceptance/CLAUDE.md`); Sapphire's steps use the same phrases as Ember's, and reuse Ember's fixture helpers (`project-ember/tests/support/fixture-helper.ts`).
- The XCUITests launch the app themselves, each on a state directory of its own; `scripts/test-ui.sh` skips them, saying why, when the screen is locked or `SAPPHIRE_SKIP_UI_TESTS=1`.
	Those that close the launch's hub register a profile first (`writeConfigWithAProfile`), since the first-run welcome can't be closed.
	Their runner is sandboxed, where git can't run, so `scripts/test-ui.sh` makes the git repo they copy into their fixtures (`SAPPHIRE_GIT_REPO`).
- The store tests run in parallel on the one main actor, where the store folds its events, so a test that blocks it (a `waitUntilExit()`, a synchronous wait) stalls every other test's `eventually`; their fixtures await git through `runGit` instead.
	Opening a state another store holds blocks it for the core's busy timeout (1 s), which only `aStateHeldByAnotherStoreIsInUse` does.
	The acceptance scenarios need an unlocked screen too: with the screen locked, the display link doesn't fire, so the app's events never fold.

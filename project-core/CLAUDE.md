# Devora Core

The shared Rust core (ADR-007): every non-UI behavior of Devora, used by Ember today and by Sapphire and the Rust `debi` later.
It is being extracted from Ember's `src-tauri/` area by area (TalAmuyal/Devora#209), so some non-UI code still lives in Ember.

## Layout

- `crates/debi`: the Rust `debi` CLI.
  It runs `repo` and `ccc` (also as a `ccc` symlink) and hands every other command to the Go `debi-go` until each command group is ported (ADR-007).
- `crates/devora-agent`: Claude Code launch specs, the stream-json/control protocol and the UI-agnostic persistent session process (ADR-009, ADR-012). SessionOptions owns the settings/transport environment contract. Recorded contract fixtures and re-recording instructions live in `crates/devora-agent/tests/fixtures/README.md`. `devora_agent::version` owns the public minimum-version check for runtime and future Rust health consumers.
- `crates/devora-agent-space`: Agent Space directories, their versioned per-space SQLite database and immutable creation-time instructions. Space owns the OS lock, and RunningSpace persists lifecycle state before forwarding session events. Worktrees use the shared creation/deletion stages in devora-workspace.
- `crates/devora-app`: the app process around the core: its per-app log file, error reporting (with a listener the UI surfaces errors through) and panic hook, and what its bundle ships (version, install suffix, user guide).
- `crates/devora-config`: the global and per-profile config files (replaced atomically on write), registered profiles, and the settings resolved from them (Claude settings, config fields, the prepare-command, git shortcuts).
- `crates/devora-core`: the facade. `agents` owns Agent Space creation, the subscriber-independent runtime registry, nonblocking tab close/pause barriers and generation-scoped controls. A UI depends on this crate alone, never on the crates behind it.
- `crates/devora-ffi`: the facade's UniFFI side, the `DevoraCore` object Sapphire binds; the records Swift sees are mapped here, so the other crates stay UniFFI-free.
- `crates/devora-health`: the cancellable in-app health check (runs `debi health --json` through a login shell and derives the typed report shared by both UIs).
- `crates/devora-ipc`: the app's local IPC server, through which tools in its terminals (`debi preview`) reach the UI; it runs on its own thread and hands each request to a UI callback.
- `crates/devora-platform`: operating-system services (the keychain today).
- `crates/devora-pty`: terminal sessions on pseudo-terminals, spawned from a `devora-terminal` spec, with output delivered through callbacks.
- `crates/devora-state`: the app-state model the UIs render (tabs, the session behind each, the active profile), changed only by named commands through one reducer, with events pulled from batched subscriptions; a store opened on a path holds that SQLite file exclusively and commits every command to it before its events go out.
- `crates/devora-terminal`: what a terminal session runs (program, arguments, cwd, the environment to set and to remove), built once for every PTY owner: Ember's `devora-pty` and Sapphire's libghostty.
- `crates/devora-workspace`: workspaces, their repos and status, worktree tasks, and cancellable workspace creation with streamed progress.
- `crates/devora-theme`: theme files to CSS custom properties.

## Rules

- No UI framework types (Tauri, UniFFI) below `devora-core`; a crate here must build and test on Linux.
- Platform-specific behavior goes behind a trait in `devora-platform`, so the Linux track only adds implementations.
- The facade's UniFFI side (for Sapphire) follows ADR-010's rules: every export returns `Result`, events are pulled in batches through a subscription object, and cancellation is an explicit command.
- Each workspace belongs to exactly one profile, the one whose `workspaces/` holds it, which `list_workspaces` records on each entry (`profile_path`).
  Profile-scoped settings (Claude settings, `terminal.default-app`, git shortcuts, the prepare-command, and Debi's profile through `DEBI_PROFILE_PATH`) resolve through the workspace's own profile, carried by its session (`Session::profile_path`), never through the active profile.
- The state persistence creation journal is internal recovery bookkeeping, outside the UI state and its Command/Event interface.
- Claude setting choices belong to `devora-config::config`; Ember reads their metadata with the settings response, and Sapphire uses the Result-returning FFI choices export.
- `devora-platform::credentials::TokenStore` separates token operations from the system keychain; `DEVORA_TEST_MODE=1` selects process-shared in-memory credentials for either app's acceptance harness, while production retains the Debi-compatible keychain service and encoding.
- Preview rows retain the terminal identity and ordered preview IDs, paths, revisions and preferred widths. OpenPreview replaces new paths, StackPreview appends them, and either refreshes an already-open path in place without removing siblings. ResizePreview validates the bounds exported to Sapphire; window-size constraints are a UI projection, not persisted writes.
- Tests live with the code they test; run them with `mise run test` (and `mise run lint`).

## Global Pause

`AppState.global_paused` is written through `Command::SetGlobalPaused` in app state.db and projected through `Event::GlobalPauseChanged`.
Existing databases migrate to running.
`Store::subscribe_global_pause` registers under the state lock and delivers the current bool followed by committed changes.
Integrations subscribe and choose their own graceful unit boundary.
The authoritative `global_paused` read also guards admission at those boundaries.

Agents use the existing actor pause-after-turn boundary for individual and global pauses.
The registry's in-memory `globally_held` eligibility distinguishes agents held globally from individual pauses.
A new hold wakes the actor, and its current turn, permission answers and Stop can complete.
The actor checks the committed hold before user turns and subsequent compaction turns.
A released hold cancels its pending boundary request, or resumes through the ownership barrier once stopping already began.
Resume eligibility ends on individual pause or tab close, and stays in memory.
The registry retains each held owner through process reaping and uses a new generation when its runtime restarts.
A fresh process restores all open agent tabs under the saved hold, showing history before any CLI launch, and releases all of them on global resume.
Only the global bool persists across app restarts.

## Periodic repository updates

`repo_updater::RepoUpdater` owns one background worker per app process.
Sapphire starts it through the FFI after launch and stops it before the agent shutdown barrier.
`PauseSource` supplies the host's authoritative current hold and committed bool subscription.
The Sapphire adapter uses the existing Store contract.
The queue is shared transport, and the trait does not require a Store owner in another host.

User Defaults keys `repo-update.enabled` and `repo-update.interval-minutes` control periodic rounds.
The interval is a positive JSON integer in minutes, default 60.
An absent enabled value defaults on for Sapphire and off for Ember.
Profile writes are refused.
Startup waits ten seconds, and each completed round establishes the next deadline.
A sleep-inclusive continuous clock owns deadlines, and the host forwards wake notifications to re-evaluate them.
An overdue wake admits one round, interval changes recompute the deadline, and disabling removes the periodic timer.
Enabled deadline waits are capped at sixty seconds so a missed host wake notification delays overdue admission by at most one minute.
Manual requests run even when periodic updates are off and coalesce into one pending round.
A hold finishes the admitted repo and keeps the next repo waiting until release.

Each round discovers canonical parent clones directly under every registered profile's repos directory.
External registrations and escaping symlinks are excluded, and aliases are deduplicated.
Unavailable profiles are logged and skipped independently so healthy profiles still run.
The workspace crate's `RepoLock` uses flock on Git's common directory `devora-repo.lock`.
Creation, refresh and add-repo share that lock around fetch/worktree mutations, with prepare commands outside it.
Creation and refresh report the first lock contention as a progress step.
Eligibility requires a clean index/worktree including untracked files, detached HEAD and no Git operation or leftover Git lock.
Git locks are logged and retained.
Fetch is non-interactive with a 120-second timeout and BatchMode SSH unless a configured core.sshCommand supplies the command.
Eligibility is checked again after fetch, then checkout uses the resolved origin default branch without forcing.
Each outcome and round summary is log-only.

Shutdown stops admission and cancels fetch, while an admitted checkout runs to completion.
The host waits up to thirty seconds.
If that wait expires, the worker retains its child and cooperative lock until checkout finishes, with a warning in the log.
No checkout process is killed.

## Debi repo commands

Rust `debi repo list` prints distinct available repo names in sorted order.
`repo add <repo> [--postfix <postfix>]` uses the shared creation operation with an explicit workspace/Agent Space target kind.
Existing workspace callers retain the `add_repo` entry point.
Agent Space additions omit generated CLAUDE.md instructions.
Worktree-name validation belongs to the creation module and is shared by the CLI and operation.

`debi/src/repo.rs` resolves registered profiles, preferring valid `DEBI_PROFILE_PATH` over the canonical containing initialized workspace or valid Agent Space.
Adding refuses a selected/owning-profile mismatch and ambiguous repo names before mutation.
Read-only Agent Space listing permits use while its agent holds the ownership lock.
Progress and prepare warnings go to stderr, the completed worktree path to stdout.
Exit codes are 0/1/2/130 for success/operation/usage/cancellation.
Prepare warnings keep the successful worktree and exit 0.
The repo add runtime installs its SIGINT handler before starting the shared worker and awaits cancellation/rollback through CreationHandle.
The CCC exec path keeps its original signal behavior.
Real-Git integration coverage lives in `crates/debi/tests/repo.rs`.

## Agent Space facade

Agent Space tabs carry their own profile and name in `SessionKind::AgentSpace`. The tab record retains its pane ID while Sapphire renders its main chat panel. It has no terminal launch spec. The host supplies explicit launch inputs to `open_agent_space` and calls `restore_agent_spaces` with those inputs on relaunch; `resolve_agent_launch` builds them from the user's login shell and the app bundle (`agents/launch.rs`). The owning profile resolves model aliases. An agent runs the model and effort chosen at creation (`CreationForm.model`; no effort leaves it to Claude Code), and an agent created before that choice existed follows the profile's `default-model` and `effort`.

The creation form lists the template's placeholders with their values and sources (`agent_creation_fields`, from `devora-agent-space`'s `inputs`), and `save_agent_field` writes one back to the global or profile `agent` config section. `agent_creation_preview` renders the instructions exactly as creation would. The built-in template is `devora-agent-space/src/default-template.md`; its optional placeholders default to empty, and rendering collapses the blank lines they leave. The form's `keep_coding_instructions` selects appending to Claude Code's system prompt or replacing it.

`devora-config::agent::settings` reads the Agent card through the same resolver, with typed FFI values, raw overrides and defining scopes. A missing override inherits, an empty string stays explicit, and legacy null values keep the creation resolver's clearing semantics. Template paths resolve relative to the config that defines them. Free-form input names are literal map keys. Entry add/update/remove and a one-write rename preserve sibling config, reject named placeholders and validate the grammar shared with the template parser. The compaction percentage's default belongs to `devora-config::agent` and is re-exported by the compaction module.

The config crate's in-process write guard covers Agent, generic, Claude and profile-registry read/modify/atomic-replace operations. It also protects the Agent settings snapshot. This preserves sibling updates when an old Settings scope's queued write overlaps a new scope or another app worker. The UI's shared settings queue additionally orders each write and its reread.

The registry serializes ownership by canonical profile and space name. Closing removes the tab immediately, while a pausing owner remains reserved until the CLI is reaped and StopInfo is committed. Reopen waits for that barrier. Delete rejects an open or pausing owner. `shutdown_agents` awaits all active pauses and cancels/awaits creation operations. Dropping the facade uses the same shutdown path.

Store's separate chat projection and batched subscription hold typed transcript items, lifecycle, usage and permission cards in memory. `devora-agent::transcript` folds text, thinking and tool-input deltas into stable sequence numbers. Final assistant blocks replace the same items, and tool results and Judgess verdicts update their tool call. Unknown shapes have an explicit JSON fallback. Known protocol frames stay in the runtime. Live append patches never write the app-state database. Its `ChatCommand` reducer rejects stale generations. Runtime recovery advances the public generation before publishing replacement events. Control exports require the observed generation, and runtime validation also rejects withdrawn questions. The space actor commits whole items and lifecycle metadata together in `.agent-space.db` before forwarding them. It commits user/permission/interrupt intentions before transport and their outcomes before replying, independently of a cancelled caller wait. Reopen seals unfinished items and restores the latest 200 typed items. Store retains a 200-item live tail. The fold loads only the latest CLI identity's unresolved items plus sequence/first-prompt metadata, and releases settled items after persistence. In-flight command outcomes retain their items through interruption. A replacement first CLI conversation gets an explicit retained-prompt marker. Intentions are never replayed automatically.

Devora owns compaction (ADR-012, `devora-agent-space`'s `compaction.rs`). The space actor tracks how full the context is, from each message's usage and the window that each result's `modelUsage` reports, and publishes it as the chat's `context`. After a turn ends over the threshold (`agent.compact-threshold`, a percentage, 80 by default, per global and profile config), it announces the step in the transcript and sends the agent a save-memory turn, then `/compact`. The tab shows these as running turns, so user turns wait. If the CLI compacts by itself, the agent is asked to refresh `memory/` once that turn ends. An interrupted or failed step abandons the compaction, and the next result over the threshold starts it again. `SpaceHandle::pause_after_turn` lets the running turn end (a result, a failed send or a crash), then pauses with no compaction step after it; an idle agent pauses at once.

The version-2 space schema contains `cli_sessions` and `events` (ordered sequence, CLI identity, kind and JSON payload). Items keep their sequence when completed or annotated. History reads are sequence-paginated in both directions. `history_before` uses an exclusive optional cursor (None selects the latest page), returns chronological items and clamps limits to 1–500. Opening a version-1 space for writing adds these history tables and registers the existing CLI identity in one transaction. Read-only listing and history reads accept version 1 without migrating it, and newer versions are rejected. The pinned native resume contract verifies retained model context with no replayed earlier messages, so history has no replay-deduplication mechanism.

Agent chat drafts and scroll anchors belong to app state, keyed by canonical owning profile and space name so tab closure retains them.
SetAgentDraft and SetAgentScroll write through the state transaction and publish AgentViewChanged.
ClearAgentView removes that presentation state after space deletion.
The app-state migration adds the agent_views table without changing existing tabs or panes.
Permission questions expose the CLI's suggested updates as JSON for the UI to show the rule and destination.

The pause-after-turn facade request reserves its owner and queues the actor command before returning.
The actor owns pause-versus-compaction ordering, while the facade awaits completion independently of the UI caller and observes the ordinary stop events.
Repeated requests reuse the pending reservation, and permission answers and interrupt remain available until the actor stops.

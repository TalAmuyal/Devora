# Devora Core

The shared Rust core (ADR-007): every non-UI behavior of Devora, used by Ember today and by Sapphire and the Rust `debi` later.
It is being extracted from Ember's `src-tauri/` area by area (TalAmuyal/Devora#209), so some non-UI code still lives in Ember.

## Layout

- `crates/debi`: the Rust `debi` CLI. It runs `ccc` (also as a `ccc` symlink) and hands every other command to the Go `debi-go` until each command group is ported (ADR-007).
- `crates/devora-agent`: Claude Code launch specs and the UI-agnostic stream-json/control protocol (ADR-009). Recorded contract fixtures and re-recording instructions live in `crates/devora-agent/tests/fixtures/README.md`. `devora_agent::version` owns the public minimum-version check for runtime and future Rust health consumers.
- `crates/devora-app`: the app process around the core: its per-app log file, error reporting (with a listener the UI surfaces errors through) and panic hook, and what its bundle ships (version, install suffix, user guide).
- `crates/devora-config`: the global and per-profile config files (replaced atomically on write), registered profiles, and the settings resolved from them (Claude settings, config fields, the prepare-command, git shortcuts).
- `crates/devora-core`: the facade. A UI depends on this crate alone, never on the crates behind it.
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

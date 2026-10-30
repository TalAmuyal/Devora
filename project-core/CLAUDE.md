# Devora Core

The shared Rust core (ADR-007): every non-UI behavior of Devora, used by Ember today and by Sapphire and the Rust `debi` later.
It is being extracted from Ember's `src-tauri/` area by area (TalAmuyal/Devora#209), so some non-UI code still lives in Ember.

## Layout

- `crates/debi`: the Rust `debi` CLI. It runs `ccc` (also as a `ccc` symlink) and hands every other command to the Go `debi-go` until each command group is ported (ADR-007).
- `crates/devora-agent`: how Devora launches Claude Code: the `ccc` command line resolved into arguments and environment.
- `crates/devora-app`: the app process around the core: its per-app log file, error reporting (with a listener the UI surfaces errors through) and panic hook, and what its bundle ships (version, install suffix, user guide).
- `crates/devora-config`: the global and per-profile config files, registered profiles, and the settings resolved from them (Claude settings, config fields, the prepare-command, git shortcuts).
- `crates/devora-core`: the facade. A UI depends on this crate alone, never on the crates behind it.
- `crates/devora-ffi`: the facade's UniFFI side, the `DevoraCore` object Sapphire binds; the records Swift sees are mapped here, so the other crates stay UniFFI-free.
- `crates/devora-health`: the in-app health check (runs `debi health --json` through a login shell).
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
- Tests live with the code they test; run them with `mise run test` (and `mise run lint`).

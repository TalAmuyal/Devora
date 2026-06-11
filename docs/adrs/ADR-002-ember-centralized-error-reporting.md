# ADR-002: Ember Centralized Error Reporting

## Status

Accepted

## Date

2026-06-11

## Context

Tauri `invoke()` errors in the Devora-Ember frontend were silently swallowed: of 21 call sites, only 4 surfaced errors to the user, each with its own hand-rolled handling (`console.error`, ad-hoc banners private to the Workspace Hub, or nothing).
The Rust side was worse — it never wrote to its own log file and never emitted error events, so failures in background contexts (prepare-command, status thread panics, the IPC accept loop) were invisible.

A recurring root cause was Tauri v2's ACL system: every command must be registered in three places (`lib.rs` `generate_handler!`, `build.rs` `.commands(&[...])`, `capabilities/main.json`), and missing any one causes a silent runtime denial whose only trace was a `console.error` nobody sees.

## Decision

One function per side is the single, obvious way to report an error; everything else routes through it.

### TypeScript: `showError` + the `invoke` wrapper

`src/errors.ts#showError(message)` is THE way to surface an error.
It records the error (for the BDD harness's `__scrapeErrors`), writes it to the Rust log file (`log_error` command), and shows a dismissible banner in a global fixed-position stack (`div.error-banner-stack`, above all overlays).
Identical messages collapse into one banner with a ×N counter.

`src/invoke.ts#invoke` wraps Tauri's invoke: a rejected command calls `showError` automatically, then rethrows.
This makes error visibility the default — call sites keep `catch` only for flow control.

`src/invoke.ts#invokeLogOnly` is the explicit opt-out for high-frequency (`write_pty`, `resize_pty`) or gracefully-degrading (`get_theme`) commands: log-file WARN only, no banner.

`errors.ts` and `invoke.ts` are the only files allowed to import `@tauri-apps/api/core`; a unit test (`src/__tests__/tauri-import-boundary.test.ts`) enforces the boundary.

Global `window` error/unhandledrejection handlers route into `showError`; the `console.error`/`console.warn` monkey-patches remain log-only.

Recursion is structurally impossible: `log_error` is only ever invoked via the raw Tauri invoke inside `errors.ts#logToFile`, with failures swallowed.

### Rust: `report_error`

`logging::report_error(app, message)` is THE way to report a Rust-side error: it writes to the log file (`LogState`) and emits an `app-error` event that the frontend routes into `showError`.
The log write happens before the emit, so an error can lose its banner at worst, never its log line.

Pure functions in `workspace.rs` stay free of Tauri types: they collect non-fatal failures into a `warnings: &mut Vec<String>` out-parameter, and the `#[tauri::command]` layer forwards them to `report_error`.

### ACL completeness test

`src-tauri/tests/acl_completeness.rs` parses the three registration points and asserts the command sets match, turning the silent-denial failure class into a test failure.

## Consequences

- Surfacing errors is the default; hiding one requires an explicit, greppable `invokeLogOnly`.
- The BDD After-hook fails any scenario in which an unexpected error was recorded, so happy paths are continuously asserted to be banner-free; intentional-error scenarios drain the list via the "the recorded errors should include" step.
- New Tauri commands that miss one of the three ACL registration points fail `cargo test` instead of failing silently at runtime.
- Errors that occur before `DOMContentLoaded` registers the `app-error` listener reach the log file but not a banner (accepted; buffering was judged over-engineering).

# ADR-001: Ember Acceptance Testing Framework

## Status

Accepted

## Date

2026-05-12

## Context

Devora-Ember is a Tauri 2 desktop app (Rust + TypeScript + xterm.js) running on macOS. We needed a testing framework that:
- Serves as living documentation of system behavior
- Verifies functionality is preserved across changes
- Tests UI, backend, and cross-cutting scenarios
- Uses real components wherever possible

Key constraints discovered during research:
- `tauri-driver` does not support macOS (Linux/Windows only)
- Playwright cannot connect to macOS WKWebView (it bundles its own WebKit)
- `WEBKIT_INSPECTOR_SERVER` is Linux-only (WebKitGTK)
- No existing browser automation protocol exists for Tauri apps on macOS

## Decision

We implement a three-part testing architecture:

### 1. Eval Bridge (Test Harness HTTP Server)
A Rust HTTP server inside the Tauri app (gated behind `DEVORA_TEST_MODE=1`) that evaluates JavaScript in the real WKWebView and returns results via a oneshot channel. This gives Playwright-like capabilities without a browser protocol.

### 2. Gherkin BDD with Cucumber.js
Feature files document system behavior in Given/When/Then format. Step definitions use the eval bridge to drive the real app.

### 3. Fake Claude API Server
A Node.js HTTP server that replays pre-recorded SSE responses for Claude Code. `ANTHROPIC_BASE_URL` redirects Claude Code's API calls to the local server. This makes Claude Code integration tests deterministic and CI-compatible.

### 4. UI Interaction Layer

A TypeScript helper stack layered on top of the eval bridge for DOM-level testing:

- **`UIDriver`** (`tests/support/ui-driver.ts`): Generic DOM interaction -- dispatches keyboard events, clicks elements, types into inputs, queries elements. All operations go through the eval bridge.
- **`ws-hub-helper`** (`tests/support/ws-hub-helper.ts`): Domain-specific helpers for the Workspace Hub -- open/close/reload the panel, navigate items, filter, switch categories.
- **`fixture-helper`** (`tests/support/fixture-helper.ts`): Creates test profiles, workspaces, and git repos on disk in a temp directory. Combined with the `DEVORA_CONFIG_PATH` env var (which overrides the config file path in `workspace.rs`), this lets tests point the Rust backend at fixture data.

## Testing Principles

- Tests replicate real user interactions -- UI changes during a test run should be indistinguishable from a human operating the app.
- No shortcuts that bypass the production flow. Claude Code is launched via `ccc` interactively (not `claude --print`), and shut down via `/exit`.
- Pre-recorded cassettes provide determinism without sacrificing interaction fidelity.

## Consequences

- Tests run against the real Tauri app with real WKWebView, real PTY, and real xterm.js
- Claude Code integration tests are deterministic via pre-recorded cassettes
- CI requires macOS runners (for Tauri app launch)
- Cassettes must be re-recorded when Claude Code's behavior changes
- The eval bridge adds ~280 lines of Rust code to the app (behind an env var gate)
- UI interaction tests use real DOM events and real rendering -- no mocking of the UI layer

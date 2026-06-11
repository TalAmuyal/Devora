# Devora-Ember

Devora-Ember is an experimental variant of Devora that replaces Kitty (terminal emulator) and Glimpse-TTY (Electron-based web viewer) with Tauri (Rust desktop framework + native WebView) and xterm.js (JavaScript terminal library).

## Key Differences from Devora OG

- **No Kitty**: The app window is a Tauri WebView, not a Kitty terminal
- **No Glimpse-TTY**: Web content (Crit UI, User Guide, etc.) renders natively in the WebView via the overlay system
- **Native workspace management UI**: The Workspace Hub is a web UI overlay instead of debi's Bubble Tea TUI
- **Overlay system**: Tab-covering overlays (Workspace Hub, User Guide, cheatsheet) and panel overlays (Crit) replace Kitty tabs and Glimpse-TTY windows

## Supported Platform

macOS (Apple Silicon) only for now, but there are plans to add Linux support in the future.

## Prerequisites

- Rust toolchain (install via [rustup](https://rustup.rs/))
- Node.js (managed by mise)
- [mise](https://mise.jdx.dev/)

## Development

From the repo root:

```
mise ember-dev                 # Build and run
mise build-ember-app           # Build .app only
mise ember-create-dmg          # Build .app + DMG installer
mise ember-test                # Run Rust tests
```

From `project-ember/`:

```
mise dev                 # Build and run
mise create-dmg          # Create DMG installer
mise test                # Run Rust tests
```

## Acceptance Testing

Ember uses Gherkin (Given/When/Then) feature files as living documentation and automated verification.
Tests drive the real app via an "eval bridge" -- a test control HTTP server that evaluates JavaScript in the WKWebView.

### Running tests

```
mise test-e2e        # Run acceptance tests
mise record-claude   # Record Claude Code API cassettes (requires real API key)
```

### Tags

| Tag | Meaning |
|-----|---------|
| `@real-claude` | Spawns real Claude Code with fake API server |
| `@real-crit` | Spawns real crit with Ember IPC integration |

### UI interaction layer

Tests that interact with the DOM use a three-tier helper stack:

| Helper | Path | Purpose |
|--------|------|---------|
| `UIDriver` | `tests/support/ui-driver.ts` | Generic DOM interaction: `pressKey`, `click`, `typeIntoInput`, element queries |
| `ws-hub-helper` | `tests/support/ws-hub-helper.ts` | Workspace Hub helpers: reload, open/close, navigate, filter, category selection |
| `fixture-helper` | `tests/support/fixture-helper.ts` | Create test profiles, workspaces, and repos on disk |

The `DEVORA_CONFIG_PATH` env var overrides the config file location so tests can point the Rust backend at fixture data without affecting real profiles.

### Workspace Hub scenarios

Scenarios covering the Workspace Hub (`tests/features/workspace-hub.feature`): listing workspaces, j/k navigation, Enter to open, q to close, text filtering, category switching, cheatsheet toggle, etc.

### Recording cassettes

When Claude Code's behavior changes or new `@real-claude` scenarios are added:

1. Authenticate with the real Anthropic API using one of:
   - **OAuth (account login)**: Log in to Claude Code normally — the OAuth token is forwarded automatically
   - **API key**: Export `ANTHROPIC_API_KEY` with a real API key
2. Run `mise record-claude`
3. Inspect the cassette: `mise cassette-inspect tests/support/fixtures/cassettes/<name>.json.gz`
4. Commit the cassette file

## Architecture

### Rust Backend (`src-tauri/src/`)

| Module | Role |
|--------|------|
| `pty.rs` | PTY manager: spawn, read, write, resize, close terminal sessions via `portable-pty` |
| `workspace.rs` | Workspace and profile management: list, create, status queries against the same filesystem layout debi uses |
| `commands.rs` | Tauri command handlers (IPC bridge between frontend and Rust) |
| `theme.rs` | Parses `kitty-configs/current-theme.conf` into CSS custom properties |
| `ipc_server.rs` | Lightweight HTTP server for external tools (e.g., Crit) to open panel overlays |
| `logging.rs` | Logging setup |
| `lib.rs` | App setup and command registration |

### TypeScript Frontend (`src/`)

| Module | Role |
|--------|------|
| `terminal/TerminalPane.ts` | xterm.js terminal instance + PTY binding (WebGL rendering, fit addon, clipboard, search, web links) |
| `session/SessionTab.ts` | Session tab: holds a terminal pane and optional panel overlay |
| `session/SessionManager.ts` | Manages ordered list of session tabs: create, close, switch, reorder |
| `ui/TabBar.ts` | Bottom tab bar showing all session tabs |
| `ui/OverlayManager.ts` | Overlay system: tab-covering (full window) and panel (main area only) modes |
| `ui/KeyboardShortcuts.ts` | Window-level keyboard shortcut handling |
| `workspace/WorkspaceHub.ts` | Workspace Hub (tab-covering overlay): list, filter, create, open workspaces |
| `webview/WebContentOverlay.ts` | Web content rendering in panel overlays (URLs via iframe, markdown via `marked`) |
| `styles/theme.css` | Centralized CSS custom properties (Catppuccin Macchiato defaults, overridden at runtime from theme file) |

### Overlay System

| Mode | Covers tab bar? | Tied to tab? | Use case |
|------|----------------|--------------|----------|
| Tab-covering | Yes | No | Workspace Hub, User Guide, cheatsheet |
| Panel | No | Yes | Crit review UI |

Tab-covering overlays may register an `onCleanup` hook via `showTabCoveringOverlay`; it runs on every dismissal path.
Dismissing the Workspace Hub (`Escape`, `q`, or `Ctrl+S`) fully tears it down and restores terminal focus.

## Keyboard Shortcuts

### Global

| Shortcut | Action |
|----------|--------|
| `Ctrl+S` | Toggle Workspace Hub |
| `Shift Shift` | Toggle Workspace Hub (rapid double-tap) |
| `Ctrl+Shift+S` | New shell session tab |
| `Ctrl+Left/Right` | Switch between session tabs |
| `Ctrl+Shift+Left/Right` | Reorder session tabs |
| `Ctrl+Shift++/-` | Increase/decrease UI size |
| `Ctrl+=` | Reset UI size (15pt) |
| `Ctrl+1/2/3` | Set UI size to small (12pt) / medium (15pt) / large (26pt) |
| `Escape` | Dismiss active overlay |
| `F1` | Open User Guide |

### Workspace Hub

| Shortcut | Action |
|----------|--------|
| `j` / `Down` | Move selection down |
| `k` / `Up` | Move selection up |
| `Enter` | Open selected workspace |
| `f` | Focus filter input |
| `1` / `2` / `3` | Show active / inactive / all workspaces |
| `n` | Toggle New Task form |
| `R` / `Shift+R` | Refresh hub (reload workspaces, keep current view) |
| `q` | Close panel |
| `?` | Toggle full cheatsheet |
| `Escape` | Unfocus filter / close cheatsheet |

## Project Structure

```
project-ember/
├── src-tauri/
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   └── src/
│       ├── main.rs, lib.rs
│       ├── pty.rs, commands.rs
│       ├── workspace.rs, theme.rs
│       ├── ipc_server.rs, logging.rs
│       └── capabilities/
├── src/
│   ├── index.html, main.ts
│   ├── terminal/TerminalPane.ts
│   ├── session/SessionTab.ts, SessionManager.ts
│   ├── ui/TabBar.ts, OverlayManager.ts, KeyboardShortcuts.ts
│   ├── workspace/WorkspaceHub.ts
│   ├── webview/WebContentOverlay.ts
│   └── styles/theme.css, main.css
├── tests/
│   ├── features/           # Gherkin .feature files
│   ├── steps/              # Step definitions
│   └── support/            # Helpers: app-driver, ui-driver, fixture-helper, ws-hub-helper, fake API server
├── mockups/
├── docs/PLAN.md
├── DEFERRED.md
└── CLAUDE.md
```

# Devora Ember

Devora Ember is an experimental variant of Devora that replaces Kitty (terminal emulator) and Glimpse-TTY (Electron-based web viewer) with Tauri (Rust desktop framework + native WebView) and xterm.js (JavaScript terminal library).

This is a proof-of-concept. See `DEFERRED.md` for items out of scope.

## Key Differences from OG Devora

- **No Kitty**: The app window is a Tauri WebView, not a Kitty terminal
- **No Glimpse-TTY**: Web content (Crit UI, User Guide) renders natively in the WebView via the overlay system
- **Native workspace management UI**: The workspace panel is a web UI overlay instead of debi's Bubble Tea TUI
- **Overlay system**: Tab-covering overlays (workspace panel, User Guide, cheatsheet) and panel overlays (Crit) replace Kitty tabs and Glimpse-TTY windows

## Supported Platform

macOS (Apple Silicon) only.

## Prerequisites

- Rust toolchain (install via [rustup](https://rustup.rs/))
- Node.js (managed by mise)
- [mise](https://mise.jdx.dev/)

## Development

From the repo root:

```
mise ember-dev       # Build and run
mise ember-build     # Build only
mise ember-test      # Run Rust tests
```

From `project-ember/`:

```
mise dev             # Build and run
mise build           # Build only
mise test            # Run Rust tests
```

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
| `workspace/WorkspacePanel.ts` | Workspace management panel (tab-covering overlay): list, filter, create, open workspaces |
| `webview/WebContentOverlay.ts` | Web content rendering in panel overlays (URLs via iframe, markdown via `marked`) |
| `styles/theme.css` | Centralized CSS custom properties (Catppuccin Macchiato defaults, overridden at runtime from theme file) |

### Overlay System

| Mode | Covers tab bar? | Tied to tab? | Use case |
|------|----------------|--------------|----------|
| Tab-covering | Yes | No | Workspace panel, User Guide, cheatsheet |
| Panel | No | Yes | Crit review UI |

## Keyboard Shortcuts

### Global

| Shortcut | Action |
|----------|--------|
| `Ctrl+S` | Toggle workspace panel |
| `Shift Shift` | Toggle workspace panel (rapid double-tap) |
| `Ctrl+Shift+S` | New shell session tab |
| `Ctrl+Left/Right` | Switch between session tabs |
| `Ctrl+Shift+Left/Right` | Reorder session tabs |
| `Ctrl+Shift++/-` | Increase/decrease UI size |
| `Ctrl+=` | Reset UI size (15pt) |
| `Ctrl+1/2/3` | Set UI size to small (12pt) / medium (15pt) / large (26pt) |
| `Escape` | Dismiss active overlay |
| `F1` | Open User Guide |

### Workspace Panel

| Shortcut | Action |
|----------|--------|
| `j` / `Down` | Move selection down |
| `k` / `Up` | Move selection up |
| `Enter` | Open selected workspace |
| `f` | Focus filter input |
| `1` / `2` / `3` | Show active / inactive / all workspaces |
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
│   ├── workspace/WorkspacePanel.ts
│   ├── webview/WebContentOverlay.ts
│   └── styles/theme.css, main.css
├── mockups/
├── docs/PLAN.md
├── DEFERRED.md
└── CLAUDE.md
```

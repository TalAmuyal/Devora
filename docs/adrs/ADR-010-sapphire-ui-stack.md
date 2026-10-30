# ADR-010: Sapphire UI Stack and Build

## Status

Accepted, after all three Mac spikes reported go: TalAmuyal/Devora#206 (UniFFI and the build skeleton, [report](https://github.com/TalAmuyal/Devora/issues/206#issuecomment-6479354609)), TalAmuyal/Devora#208 (embedded Neovim, [report](https://github.com/TalAmuyal/Devora/issues/208#issuecomment-6480548262)) and TalAmuyal/Devora#207 (libghostty, [report](https://github.com/TalAmuyal/Devora/issues/207#issuecomment-6518346541)).
Their findings are folded in below.

## Date

2026-10-28 (proposed), 2026-10-30 (accepted)

## Context

Sapphire (ADR-006) is the hyper-modern, hyper-optimized macOS build.
Besides the agent pane (ADR-009), it needs:

- a terminal;
- a prompt editor that keeps Neovim's editing model, so living in Neovim is no longer necessary;
- web pages (Slack threads, GitHub PRs) as tabs, splits or overlays;
- file and diff review;
- light/dark/system theming with user themes;
- Vim-like ergonomics (ADR-004): two levels of tabs, splits, a `:` command line, `Ctrl-w h/j/k/l` and `Ctrl-h/j/k/l`.

All of it sits over the shared core (ADR-007) and its state model (ADR-008).

## Decision

- **SwiftUI on macOS 27.0.1+, Apple Silicon only**
	- AppKit is used through `NSViewRepresentable` where SwiftUI falls short (Metal surfaces, precise key handling, the text grid).
	- Observation and Swift concurrency are used throughout.
	- There are no back-deploy shims.
- **The core is bound through UniFFI, under three facade rules**
	- **Every exported function returns `Result`.**
		A Rust panic in a `Result`-returning export becomes a thrown Swift error, but in an infallible export it is a `try!` failure that kills the app, even with `panic = "unwind"`.
	- **Events are pulled in batches.**
		Each stream is one subscription object (`next_batch(max)` and `close()`), wrapped once as a Swift `AsyncSequence`.
		Pulling keeps ordering and backpressure in Rust, where a foreign callback would stall the producer and need its own serial queue.
	- **Cancellation is explicit.**
		Cancelling a Swift `Task` does not reach the Rust future, so long-running calls are cancelled by another command (`interrupt`, `close`) or by a cancel token raced against the work.
- **One `@Observable` store on the main actor, folded once per display frame**
	- Views never call the core directly; they dispatch commands.
	- Events are buffered off the main actor and folded on the display-link tick.
		At 10,000 events/s, that holds 120 fps at about 35% CPU, with at most one frame of latency.
		One main-actor hop per event falls seconds behind instead.
	- Only leaf views observe high-frequency state.
		A root view that reads streaming state re-evaluates the whole window on every fold.
- **The terminal is libghostty, through Ghostty's internal embedding API**
	- **It uses `ghostty.h` ("libghostty-internal"), which Ghostty documents as tailored to its own macOS app and not meant for external embedders.**
		It is chosen deliberately: it is complete and works today, with Ghostty's Metal renderer, the PTY, input encoding, scrollback read-back and config hot-reload.
	- **It is pinned and isolated.**
		Ghostty is pinned to one commit (currently v1.4.0) and built from source by a `mise` task.
		`TerminalKit` is the only module that imports `GhosttyKit`, and it exposes `TerminalPane` and `TerminalSession`, so swapping the implementation doesn't touch the rest of Sapphire.
		`libghostty-vt`, Ghostty's sanctioned but still unstable embedding library with no renderer or PTY of its own, is the long-term path; it is re-evaluated periodically in TalAmuyal/Devora#221, along with version upgrades.
		SwiftTerm remains a fallback, and isn't needed.
	- **libghostty owns the PTY and the child process.**
		The cwd comes from the kernel through the foreground PID, and scrollback is read back as text.
		Restoring a terminal therefore means relaunching in the saved cwd and re-displaying the saved text, until `libghostty-vt`'s snapshot API is an option.
	- **Sapphire generates libghostty's whole config, never loading the user's Ghostty config.**
		It starts with `keybind = clear`, so the host's chords are routed first (ADR-003).
		A theme change applies through a regenerated config (`ghostty_app_update_config`).
	- **Every libghostty callback is built outside any actor (`nonisolated`) and hops to the main thread.**
		libghostty calls back from its renderer and IO threads, and a closure written in a `@MainActor` context makes Swift 6's runtime isolation check abort the app.
	- The spawn environment drops `NVIM` and `NVIM_LISTEN_ADDRESS`, as the embedded editor's does, since an inherited `$NVIM` makes flatten.nvim-style plugins exit.
- **The prompt editor is `nvim --embed`, drawn natively**
	- The msgpack-RPC client, the grid model and rendering live in Swift, so redraws reach the renderer without an FFI hop.
		Redraw events are parsed off the main actor and applied once per `flush`, and the grid is drawn as soon as a flush arrives rather than on the next update cycle.
		The core owns the spawn spec, draft persistence and the `@`/`/` completion sources.
	- **It runs a config vendored in Sapphire, never the user's.**
		The vendored config is curated from the maintainer's own, and is isolated in Sapphire's own config, data and state directories, with its plugins pinned in the bundle.
		The user's config made the embedded editor fragile: a plugin manager and tree-sitter parsers under the user's data directory, plugins that act on startup (session restore, handing buffers to a parent Neovim through an inherited `$NVIM`), and messages that block on prompts.
	- **It attaches with `ext_linegrid`, `ext_cmdline`, `ext_popupmenu` and `ext_messages`.**
		`ext_messages` is required rather than cosmetic: without it, any burst of messages raises a press-enter prompt that blocks every RPC request.
		`ext_multigrid` is deferred; until then, plugins that draw their own floating windows render into the grid.
	- **Startup and shutdown are defensive.**
		Neovim serves RPC requests while its config still runs, so the host plugin loads with `--cmd` and the host waits for its `VimEnter` notification before touching the buffer.
		Meanwhile the host polls `nvim_get_mode`, which Neovim answers even while blocked, and dismisses blocking prompts; `nomore` is set from the start.
		Shutdown closes stdin, then sends SIGKILL after a grace period, because a Neovim stuck at a prompt on its way out ignores both EOF and SIGTERM; the host ignores SIGPIPE so a dying editor can't take the app down; and the spawn environment drops `NVIM`.
	- **Drafts survive a crash with their cursor and undo history.**
		Buffer changes (`nvim_buf_attach`) are debounced to about 250 ms, then the core saves the text and cursor while Neovim writes the undo history to a core-owned path with `:wundo`.
		Restore sets the text without recording an undo step, then loads the history with `:rundo`, which only accepts the exact text it was saved with.
		The prompt buffer never has a swap file, since a killed editor would leave an ATTENTION prompt behind.
	- `Ctrl-h/j/k/l` moves between Neovim windows first, and hands off to Sapphire's panes at the edge (an `rpcnotify` from mappings installed on `VimEnter`, so they win over the config's own).
- **Web panes are `WKWebView`**, with a persistent per-profile data store.
- **The build is SwiftPM modules plus a generated app target**
	- All Swift code lives in SwiftPM modules (`AppShell`, `Store`, `Panes/*`, `Theme`), each pane kind a deep module with one view entry point.
		`App` is avoided as a module name, since it collides with SwiftUI's `App` protocol.
	- A thin app target is generated by XcodeGen, and the `.xcodeproj` is gitignored.
	- The Rust core is a static library in an xcframework, with Swift bindings generated by `uniffi-bindgen` in library mode, from a bin target in the core so versions always match.
		All generated output is gitignored, and the xcframework is replaced only when the library or headers change, since rewriting it forces a full relink and re-sign.
	- `mise` tasks drive Rust → bindings → generation → `xcodebuild` → notarization → DMG, without opening Xcode.
- **Signing uses the hardened runtime and no sandbox**
	- Local builds are signed ad hoc.
		Releases need a Developer ID Application certificate, a secure timestamp, and no `get-task-allow`.
	- No `com.apple.security.cs.*` exceptions are needed: spawning `claude`, `nvim`, `zsh` and `git`, Metal, and `WKWebView` all work under the hardened runtime.
	- Every bundled helper binary (`debi`, `judgess`, …) is signed the same way.
	- The `NS*UsageDescription` strings cover the privacy prompts that child processes trigger, since Sapphire becomes their responsible process.
- **Theme tokens from the core** map to SwiftUI colors, the libghostty palette and Neovim highlight groups, so a theme change repaints every pane.

## Alternatives considered

- **A pure-Rust GPU UI (GPUI-style) or objc2 AppKit bindings.** Rejected: less native, and the newest macOS APIs arrive late.
- **SwiftTerm or `alacritty_terminal`.** Not chosen: libghostty is the fastest available and Metal-native, and TalAmuyal/Devora#207 found it covers everything Sapphire needs. SwiftTerm stays as a fallback that isn't needed.
- **`libghostty-vt` with Sapphire's own renderer.** Not now: it means building a renderer, font shaping and PTY integration, and its API isn't stable either. Revisit in TalAmuyal/Devora#221, especially once terminal restore is in scope.
- **Neovim in a terminal pane.** Rejected: no native popups or IME, and clumsy text handoff to the agent.
- **Running the user's own Neovim config in the embedded editor**, either copied under its own `NVIM_APPNAME` or sourced in place from a Sapphire-owned `-u` init. Rejected after TalAmuyal/Devora#208: a copy loses the plugin manager and parsers, which live under the data directory `NVIM_APPNAME` moves, and breaks configs that resolve their own path; sourcing in place works, but every user config brings its own startup hazards. A vendored config makes the editor predictable.
- **The Neovim RPC client and grid in the Rust core.** Rejected: an extra hop and an FFI-shaped redraw model for no gain.
- **Pure SwiftPM with a hand-rolled bundling script, or a checked-in `.xcodeproj`.** Rejected: the first means maintaining bundling and signing ourselves; the second is merge-hostile and painful for agents.
- **Foreign callbacks for events (a Rust-called Swift `EventSink`).** Rejected after TalAmuyal/Devora#206: equally fast once batched, but a slow sink stalls the producer, ordering needs a hand-rolled serial queue, and Rust holds a strong reference that has to be cleared explicitly.
- **Folding every event, or every Rust-side batch, on the main actor.** Rejected after TalAmuyal/Devora#206: one hop per event falls seconds behind at 10,000 events/s, and roughly 1 ms batches keep up but cost about 88% CPU.

## Consequences

- Building Sapphire requires Xcode with the macOS 27 SDK.
	CI needs a macOS 27 runner, hosted or self-hosted (TalAmuyal/Devora#212).
- The `DevoraCore` facade (ADR-007) follows the three facade rules on its UniFFI side, so the core's API is shaped by them from Phase 1 on.
- Keychain items stay readable without prompts as long as the core reads them through the `security` CLI (`devora-platform`), whose access list already trusts that tool; reading them through the Security framework directly would prompt the first time Sapphire's binary does so.
- Notarization stays untested until a Developer ID certificate is available.
- The proposed performance budgets get measured from the first prototype on: under 500 ms from cold start to a restored window, and keystroke-to-glyph drawn within one frame at 120 Hz.
	The embedded Neovim already draws within a frame (TalAmuyal/Devora#208: p95 6.3–6.6 ms); reaching the screen takes another one to two frames of Core Animation's pipeline, which no out-of-process editor avoids.
- Sapphire's prompt editor does not follow the user's own Neovim setup; changing its keys or plugins means changing the vendored config.
- Building libghostty needs Zig (through `mise`, matching Ghostty's `minimum_zig_version`), Xcode's Metal Toolchain (a separate download since Xcode 27) and GNU gettext's `msgfmt`.
	A cold build takes about 4 minutes and produces a 150 MB static library, so CI caches it by the pinned commit.
- Each Ghostty upgrade is manual work, tracked in TalAmuyal/Devora#221 until upgrades can be automated.

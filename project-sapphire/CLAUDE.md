# Devora Sapphire

The native macOS build (ADR-006, ADR-010): SwiftUI and AppKit over the shared Rust core (`../project-core/`), bound through UniFFI by the `devora-ffi` crate.
The app is one window: the active tab's terminal (libghostty) over a tab bar, under the window's surfaces, of which the Workspace Hub (`Ctrl+S`) is the first.
It reopens the tabs it had when it last quit, crashed or was killed, or one shell tab when it had none; `+` opens another, and the Workspace Hub opens a workspace in a new tab.
Keys go through ADR-003's layer routing (`Sources/Layers/CLAUDE.md`): `Ctrl+S` opens the Workspace Hub, `Ctrl+←/→` switch tabs, `Ctrl+Shift+←/→` move the active tab, `Ctrl+Shift+S` opens a shell, and `Ctrl+1/2/3`, `Ctrl+=` and `Ctrl+Shift+=`/`-` set the terminals' font size; every other key reaches the terminal.
Closing the window quits the app.

## State and terminals

- The tabs are core-owned state (`devora-state`, ADR-008): the store dispatches a `StateCommand` to the core and never changes `Store.app` itself.
- The core commits every command to `state.db` (SQLite) before its events go out, in `~/Library/Application Support/Devora Sapphire/`, or `Devora Sapphire <suffix>/` for a bundle with an install suffix; `DEVORA_STATE_DIR` replaces the directory (tests).
	The core resolves that path (`sapphireStatePath`, from the bundle's `Resources`); `Store(configPath:state:log:)` takes `.app` for it, `.file` or `.inMemory` (tests).
	A failed command, including one that couldn't be saved, takes no effect and shows as an error banner.
- The app state has one writer: the core holds `state.db` exclusively while it runs, so a second instance on it gets `StartupProblem.stateInUse` and can only quit.
	A state that can't be opened (corrupt, from a newer build) is `StartupProblem.stateUnreadable`: Quit leaves it untouched, Start Fresh (`Command.startFresh`) renames it, with its WAL, to `state-unreadable-<UTC time>.db` beside it and opens a fresh one.
	Until the problem is answered, the store has no core and `StartupProblemView` fills the window.
- The launch (`RootView`) runs once the state is open: `openShellIfNoTabs`, then the profiles and the selected profile's workspaces; restored tabs get fresh terminals from their specs (a terminal's contents aren't restored), and a workspace creation the app stopped in the middle of comes back as failed ("interrupted").
- The store pulls the core's events through `Batches`, the `AsyncSequence` over a core subscription.
	Events wait in an off-main inbox until `FrameDriver`'s display link folds them into `Store.app`, once per frame and only while events are pending.
	Without a display link (tests), they fold on the main actor's next turn.
- `Terminals` (in `AppShell`) holds the live terminals, keyed by the terminal id of each tab's pane: it starts one from the core's `terminalSpec` when a tab appears and drops it when the tab leaves the state, which frees the libghostty surface and ends its process.
	Every tab's terminal stays in the view tree, so switching tabs keeps them running; a terminal whose process exits closes its tab, as in Ember.
- `Surfaces` (in `AppShell`) holds the key router and one layer stack per tab, with the tab's terminal as its bottom layer; the active tab's top layer has the keyboard focus unless a window surface is open.

## Workspace Hub

Ember's home page (`Sources/AppShell/WorkspaceHub/`): the selected profile's workspaces, beside the focused one's details, on a page layer over the whole window.

- `Surfaces.openWorkspaceHub` pushes it with a fresh `WorkspaceHubModel`, which holds what one opening has (the category, the filter, the cursor, the toast), so closing the hub forgets them, as Ember's `unload` does.
	The profiles, the workspaces and their statuses are the store's; the hub loads them on opening, reloads them silently when a page over it closes (its `onReveal`), and reloads them with a toast on `R` ("Refreshing…" for at least 1 s, then "Refreshed successfully" for 3 s unless a call failed; the latest press wins).
- The statuses (`Command.loadWorkspaceStatuses`) are one `workspace_status` core call per workspace, off the main actor and four workspaces at a time (each runs its repos' `git status` in parallel, at most five at once), the active workspaces first; each is kept as it arrives, so the list and the detail panel fill in progressively, and a workspace without one shows as pending.
	A workspace whose status can't be read is invalid (an error dot, badge and message), with no banner; a failed call also has its banner.
	Dropping the statuses (a fresh load, another profile) makes the loads still running drop theirs.
- Its keys are in `WorkspaceHubModel.handleKey`; while the filter field has the focus, keys type into it and Escape leaves it, as Ember's editable-element guard does.
	The cheatsheet (`?`) is a page of its own over the hub, scrolled by `VimScroll`, ADR-004's motions over a SwiftUI scroll view; `ListCursor` is the list's cursor.
- The profile menu in its header switches the profile, standing in for Ember's profile dropdown, which comes with the hub's operations.
- Ember refuses to close the hub while there are no profiles, behind its first-run welcome; Sapphire has neither yet, so its hub closes and says there are no profiles.

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
- `Tests/StoreTests`: the store and the event fold against the real core, over a fixture config.
- `Tests/TerminalKitTests`: the command line libghostty runs.
- `Tests/LayersTests`: the routing and focus rules, on stand-in layers.
- `Tests/AppShellTests`: which key-downs are app-wide shortcuts, and the Workspace Hub's list, cursor, scrolling and keys.
- `acceptance/`: the acceptance scenarios' Cucumber.js harness (steps, hooks, the bridge client) and Sapphire-only features; `../acceptance/features/` holds those shared with Ember.
- `UITests/`: the XCUITests, a UI-test target in `App/project.yml`.
- `scripts/build-core.sh`: `devora-ffi` → static library → bindings → `build/DevoraCoreFFI.xcframework`.
- `build/` and `vendor/` (gitignored): the xcframeworks, DerivedData, and the Ghostty checkout.

## Commands

- `mise run build`: the core, the bindings, the Xcode project and `build/DerivedData/Build/Products/Release/Devora Sapphire.app`; it builds GhosttyKit first if `build/GhosttyKit.xcframework` is missing.
- `mise run run`: launches the app through LaunchServices (`open -n`); arguments are `--flag value` pairs, since a bare argument makes AppKit open no window.
	`--open-workspace <path>` opens a workspace of the selected profile in a new tab; `--surface workspace-hub` opens the Workspace Hub.
	`--snapshot <out.png>` types a command into the first shell tab, captures the window to `<out.png>` and the active tab's text to `<out.png>.txt`, then quits.
- `mise run test`: every test, in order: `test-unit` (`swift test`, the store against the real core), `test-acceptance` (the scenarios over the test bridge) and `test-ui` (the XCUITests); the last two build the app first.
- `mise run core`, `mise run generate`, `mise run ghostty-fetch`, `mise run ghostty-build`: the build's steps.
	`ghostty-build` needs Xcode's Metal Toolchain and gettext's `msgfmt` (`mise run ghostty-prereqs` installs whichever is missing; it is meant for CI, since it installs system-wide, so ask before running it on a developer's Mac); set `GHOSTTY_CLONE_SOURCE` to clone Ghostty from a local copy instead of GitHub.
- `mise run ghostty-cache-key`: the key CI caches `build/GhosttyKit.xcframework` under (the `sapphire` job in `.github/workflows/ci.yml`).

The app reads the global config from `DEVORA_CONFIG_PATH` when set, as Ember does; `Store(configPath:)` overrides it for tests, except in terminal specs, whose settings `devora-config` reads through `DEVORA_CONFIG_PATH` alone.

## Testing

Acceptance tests follow Ember's (ADR-001): Gherkin scenarios run by Cucumber.js against the real, built app, which they drive through a test bridge.
Real input (key events, focus, the IME, libghostty's key path) is what the bridge can't drive, and what SwiftUI draws is what it can't see; XCUITests cover both.

- The bridge (`Sources/AppShell/TestBridge.swift`) is a loopback HTTP server that starts only when `DEVORA_TEST_MODE=1`, once the launch is done, and prints `Devora test harness on port <port>` to stderr, as Ember's does.
	It is compiled into every build, as Ember's is, and does nothing unless the variable is set.
	Its routes: `GET /test/state` (profiles, workspaces, tabs with their terminal's status, focus and layers, the active tab, the window stack's layers, the open Workspace Hub as it shows (its list, cursor, filters, detail panel, toast and cheatsheet scrolling), the font size, the error banners, the log's path and the startup problem); `POST /test/dispatch` with a store command (`{"command": "openShell"}`, `load`, `selectProfile` with `path`, `openWorkspace` with a workspace `id`, `activateTab` and `closeTab` with a tab `id`, `startFresh`, `dismissError` with a banner `id`); `POST /test/errors/scrape` (the errors shown since the last scrape, which it clears, as Ember's `__scrapeErrors`); `POST /test/quit` (the app menu's Quit; the app may exit before it answers); `GET /test/terminal/text?tab=<id>` and `POST /test/terminal/type` with `{"line", "tab"}` (the active tab when `tab` is omitted; a 409 until its terminal runs).
	`POST /test/key` with `{"key", "modifiers"}` presses a key: it makes the key-down AppKit would (key code, characters, flags) and hands it to `LayerRouter.handle`, the monitor's own entry point, then, unless consumed, to the window's `sendEvent`, which delivers it to the first responder; it skips the event monitor's plumbing and the menu's key equivalents, which the XCUITests cover.
	It starts early when the launch meets a startup problem, so that a scenario can answer it.
	State changes fold on the next frame, so steps poll `state` (`Bridge.waitFor`) rather than reading it once.
- Every scenario launches a fresh app (`acceptance/support/hooks.ts`, `app.ts`), straight from the bundle's binary rather than through `open`, with a config and an app state of its own (`DEVORA_CONFIG_PATH`, `DEVORA_STATE_DIR`) and only a login session's environment, and starts once the startup tab's terminal runs; failed steps save the state, the active terminal's text and the app's stderr to `test-results/failures/`.
	As in Ember, a scenario fails if an error was shown that no step scraped ("the recorded errors should include").
	Relaunches within a scenario (`persistence.feature`) keep its state directory.
- Scenarios shared with Ember live in `../acceptance/features/` (see `../acceptance/CLAUDE.md`); Sapphire's steps use the same phrases as Ember's, and reuse Ember's fixture helpers (`project-ember/tests/support/fixture-helper.ts`).
- The XCUITests launch the app themselves, each on a state directory of its own; `scripts/test-ui.sh` skips them, saying why, when the screen is locked or `SAPPHIRE_SKIP_UI_TESTS=1`.

# Devora Sapphire

The native macOS build (ADR-006, ADR-010): SwiftUI and AppKit over the shared Rust core (`../project-core/`), bound through UniFFI by the `devora-ffi` crate.
The app is one window: the active tab's terminal (libghostty) over a tab bar, under the window's surfaces, of which the Workspace Hub (`Ctrl+S`) is the first.
It reopens the tabs it had when it last quit, crashed or was killed, without the Workspace Hub; with no tabs to reopen, it opens on the Workspace Hub, as Ember always does.
`+` opens a shell tab, and the Workspace Hub opens a workspace in a new tab, or creates a task in one.
Keys go through ADR-003's layer routing (`Sources/Layers/CLAUDE.md`): `Ctrl+S` opens the Workspace Hub, `Ctrl+←/→` switch tabs, `Ctrl+Shift+←/→` move the active tab, `Ctrl+Shift+S` opens a shell, and `Ctrl+1/2/3`, `Ctrl+=` and `Ctrl+Shift+=`/`-` set the terminals' font size; every other key reaches the terminal.
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
	Events wait in an off-main inbox until `FrameDriver`'s display link folds them into `Store.app`, once per frame and only while events are pending.
	Without a display link (tests), they fold on the main actor's next turn.
- `Terminals` (in `AppShell`) holds the live terminals, keyed by the terminal id of each tab's pane: it makes one when a tab appears and drops it when the tab leaves the state, which frees the libghostty surface and ends its process.
	The terminal is at the bottom of its tab's stack at once, while its launch (the core's `terminalSpec`) loads; it runs once it has its launch and its view is in the window, when libghostty makes the surface and spawns the shell.
	What is typed into it before then (keys, the bridge's lines, a paste) waits in its `TerminalSession` and goes to the surface in order once it exists; the PTY holds it until the shell reads it, as in any terminal.
	Every tab's terminal stays in the view tree, so switching tabs keeps them running; a terminal whose process exits closes its tab, as in Ember.
	A tab creating its workspace has no terminal until the workspace exists (its session is `CreatingWorkspace`, which has no terminal spec).
- `Surfaces` (in `AppShell`) holds the key router and one layer stack per tab, with the tab's terminal as its bottom layer, under the progress page of a tab creating its workspace; the active tab's top layer has the keyboard focus unless a window surface is open.

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
	An active workspace also has Add Repo, a Sapphire addition until it has the Command Palette, where Ember has it (see Repos).
	Remove Task (`Store.removeTask`, `remove_task`) goes ahead at once when every repo is known to be clean and on a detached HEAD (`RemoveTaskDecision`), and otherwise asks with a confirmation dialog listing what will be lost; Delete (`Store.deleteWorkspace`, `delete_workspace`) never asks.
	Either one lists the workspaces again, keeping the filters and the cursor; a failure shows its banner.
- The confirmation dialog (`ConfirmationDialog.swift`, `Surfaces.confirm`) is a modal layer over the window, with Ember's keys: Escape and `q` cancel, Enter confirms, Tab and Shift+Tab move between its two buttons without ever leaving it, and Space presses the focused one; it draws its own focus, so the Tab trap doesn't depend on the system's keyboard navigation setting.
- The header has Ember's two dropdowns (`DropdownMenu.swift`): the profiles (each with its repo count, the selected one checked), and the burger menu, whose "Save loading latencies" writes the latest load's `LoadingLatencies` to `<profile>/diagnostics/hub-profile-<timestamp>.json` and shows the path in a toast.
	An open dropdown is a popup layer (`Surfaces.toggleHubMenu`) whose content is empty: the hub draws the panel under its trigger, as Ember's caller-mounted popup, with a click anywhere else closing it.
	Escape and `q` close only the dropdown; any other key closes it and reaches the hub.
- What leads to a surface Sapphire doesn't have yet is shown disabled, with a "Not in Sapphire yet" tooltip: "Manage Profiles…" and "Settings" (the Settings Hub), and "Health" (the Health Hub); their keys (`P`, `H`) do nothing, and the cheatsheet leaves them out.

## Profiles and the first run

Ember's profile form and first-run welcome (`Sources/AppShell/Profiles/`).

- The profile form (`ProfileFormModel`) has a name and a root path, with "Browse…" (an `NSOpenPanel` folder picker); it validates the path (`validate_profile_path`) 250 ms after the last edit, or at once when it is set (the initial path, a browsed folder), as a new profile, an existing one (whose own name locks the name field, and "Register Profile"), or an invalid path.
	Submitting registers it (`register_profile`, which creates `config.json`, `repos/` and `workspaces/` for a new one), then the hub selects it and loads it from scratch.
	Enter submits from a field or the submit button, Space presses the focused button, and Tab and Shift+Tab move through it, skipping a locked name and a disabled submit; it draws its buttons' focus itself, as the dialogs do.
- With no profiles, a listing that succeeded makes the Workspace Hub show the first-run welcome (`WorkspaceHubModel.welcome`) instead of its list: the profile form, on `~/devora`, with no control focused, as in Ember.
	The hub refuses `q` and Escape meanwhile (`dismissDecision`), as Ember's zero-profile lock does; Escape in a field leaves the field, and the hub's own keys do nothing.
	The burger menu has no "Settings" and the header no profile dropdown, as in Ember.
- The profile dropdown's "New Profile…" opens the form in the New Profile dialog, a modal layer over the hub, where Ember opens the Settings Hub's inline form; Escape cancels it, and registering closes it.
- The selected profile is the core's active profile (`AppState.activeProfile`, which `Store.selectedProfilePath` reads): selecting one dispatches `SetActiveProfile`, so a launch restores it, where Ember always starts on the first profile.
	A load selects the restored profile while it is registered, otherwise the first profile, or none, and dispatches that in its place; it reads the active profile from the core, since the state's events fold on a later frame, and folds its own selection at once, so that the New Task dialog and the hub see it as soon as the workspaces are listed.
- Deleting a profile is the Settings Hub's (`d`, behind "Manage Profiles…"); the store already has its side: `Store.unregisterProfile` (`unregister_profile`, which leaves the directory on disk, then loads again, so the selection moves to the first profile, as a launch's does, and the hub shows the welcome after the last one), and `Store.tabs(usingProfile:)`, the open tabs that make Ember refuse the deletion.

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
	Ember opens it from the Command Palette on the active tab's workspace (`Store.addRepoTargetOfActiveTab`, which refuses a tab without a workspace with Ember's banner); until Sapphire has a palette, the Workspace Hub's Add Repo opens it on the focused workspace, then reloads that workspace's status.
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
- `Tests/AppShellTests`: which key-downs are app-wide shortcuts, the Workspace Hub's list, cursor, scrolling and keys, the Remove Task decision, the confirmation dialog's keys, the New Task dialog's selection, filter and keys, the repo dialogs' forms, keys and clone hint, and the profile form's validation and keys.
- `acceptance/`: the acceptance scenarios' Cucumber.js harness (steps, hooks, the bridge client) and Sapphire-only features; `../acceptance/features/` holds those shared with Ember.
- `UITests/`: the XCUITests, a UI-test target in `App/project.yml`.
- `scripts/build-core.sh`: `devora-ffi` → static library → bindings → `build/DevoraCoreFFI.xcframework`.
- `build/` and `vendor/` (gitignored): the xcframeworks, DerivedData, and the Ghostty checkout.

## Commands

- `mise run build`: the core, the bindings, the Xcode project and `build/DerivedData/Build/Products/Release/Devora Sapphire.app`; it builds GhosttyKit first if `build/GhosttyKit.xcframework` is missing.
- `mise run run`: launches the app through LaunchServices (`open -n`); arguments are `--flag value` pairs, since a bare argument makes AppKit open no window.
	`--open-workspace <path>` opens a workspace of the selected profile in a new tab; `--surface workspace-hub` opens the Workspace Hub.
	`--snapshot <out.png>` types a command into the first shell tab, if there is one, captures the window to `<out.png>` and the active tab's text to `<out.png>.txt`, then quits.
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
	Its routes: `GET /test/state` (profiles, workspaces, tabs with their terminal's status, focus, layers and progress page, the active tab, the window stack's layers, whether a text field has the focus, the open Workspace Hub as it shows (its list, cursor, filters, detail panel, toast, cheatsheet scrolling, profile label, open dropdown's rows and whether it shows the first-run welcome), the open confirmation dialog, the open New Task, Add Repo and Clone Repo dialogs, the open profile form (the New Profile dialog's, or else the welcome's), the font size, the error banners, the log's path and the startup problem); `POST /test/dispatch` with a store command (`{"command": "openShell"}`, `load`, `selectProfile` with `path`, `openWorkspace` with a workspace `id`, `activateTab` and `closeTab` with a tab `id`, `startFresh`, `dismissError` with a banner `id`); `POST /test/hub`, which clicks what the open hub draws, running what the button runs (`{"action": "removeTask"}`, `delete`, `duplicate` or `addRepo` on the focused workspace, `newTask`, `toggleMenu` with a `menu`, `profiles` or `main`, and `menuItem` with the `label` of a row of the open dropdown, a 409 when it is disabled); `POST /test/dialog` with `{"button": "confirm"}` or `cancel`; `POST /test/new-task` with `{"action": "toggleRepo", "name"}`, `create`, `cancel` or `cloneRepo`, which clicks what the New Task dialog draws; `POST /test/add-repo` with `{"action": "selectRepo", "name"}`, `submit`, `cancel` or `progressAction` (the progress's Cancel or Close), and `POST /test/clone-repo` with `{"action": "setUrl", "value"}`, `submit`, `cancel` or `progressAction`, which do the same for the repo dialogs; `POST /test/profile-form` with `{"action": "setName", "value"}` or `setPath`, which fill in the open profile form's fields as typing does, `submit` (a 409 while it is disabled) or `cancel`; `POST /test/layers/clear` removes every window surface, the first-run welcome included, as Ember's teardown does; `POST /test/progress` with an optional `tab`, which clicks a progress page's Cancel or Close; `POST /test/snapshot` with `{"path"}` (captures the window to a PNG); `POST /test/errors/scrape` (the errors shown since the last scrape, which it clears, as Ember's `__scrapeErrors`); `POST /test/quit` (the app menu's Quit; the app may exit before it answers); `GET /test/terminal/text?tab=<id>` and `POST /test/terminal/type` with `{"line", "tab"}` (the active tab when `tab` is omitted; a 409 until the tab has its terminal, which holds the line until it runs); `POST /test/terminal/focus` with `{"tab"}` makes a tab's terminal the window's first responder.
	A tab's `terminal` in the state is `none`, `starting`, `running` (its surface exists, so its shell has a PTY) or `failed: <reason>`.
	`POST /test/key` with `{"key", "modifiers"}` presses a key: it makes the key-down AppKit would (key code, characters, flags) and hands it to `LayerRouter.handle`, the monitor's own entry point, then, unless consumed, to the window's `sendEvent`, which delivers it to the first responder, and answers whether the router consumed it; it skips the event monitor's plumbing and the menu's key equivalents, which the XCUITests cover.
	It starts early when the launch meets a startup problem, so that a scenario can answer it.
	State changes fold on the next frame, so steps poll `state` (`Bridge.waitFor`) rather than reading it once.
- Every scenario launches a fresh app (`acceptance/support/hooks.ts`, `app.ts`), straight from the bundle's binary rather than through `open`, with a config and an app state of its own (`DEVORA_CONFIG_PATH`, `DEVORA_STATE_DIR`) and only a login session's environment, and starts once the launch has settled: the Workspace Hub it opens has loaded, or, after a relaunch, the restored tabs' terminals run.
	A scenario's config starts with no profiles, so the launch's hub is the first-run welcome, which refuses `q` and Escape: steps that need a known state remove it through the bridge ("Devora is running with no sessions open", as Ember's scenarios start after its teardown), and "the Workspace Hub is open" removes an open hub before opening it, so that it loads the profiles the scenario has made since.
	Failed steps save the state, the active terminal's text and the app's stderr to `test-results/failures/`.
	As in Ember, a scenario fails if an error was shown that no step scraped ("the recorded errors should include").
	Relaunches within a scenario (`persistence.feature`) keep its state directory.
- Scenarios shared with Ember live in `../acceptance/features/` (see `../acceptance/CLAUDE.md`); Sapphire's steps use the same phrases as Ember's, and reuse Ember's fixture helpers (`project-ember/tests/support/fixture-helper.ts`).
- The XCUITests launch the app themselves, each on a state directory of its own; `scripts/test-ui.sh` skips them, saying why, when the screen is locked or `SAPPHIRE_SKIP_UI_TESTS=1`.
	Those that close the launch's hub register a profile first (`writeConfigWithAProfile`), since the first-run welcome can't be closed.
	Their runner is sandboxed, where git can't run, so `scripts/test-ui.sh` makes the git repo they copy into their fixtures (`SAPPHIRE_GIT_REPO`).
- The store tests run in parallel on the one main actor, where the store folds its events, so a test that blocks it (a `waitUntilExit()`, a synchronous wait) stalls every other test's `eventually`; their fixtures await git through `runGit` instead.
	Opening a state another store holds blocks it for the core's busy timeout (1 s), which only `aStateHeldByAnotherStoreIsInUse` does.
	The acceptance scenarios need an unlocked screen too: with the screen locked, the display link doesn't fire, so the app's events never fold.

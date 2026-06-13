## Acceptance Testing

### Testing philosophy

All acceptance tests must replicate user actions as closely as possible.
When tests run, UI changes should look as though a real user is interacting with the app.
For example, Claude Code must be launched via `ccc` interactively, no shortcuts (like `--print`) that bypass the real user flow; And use `/exit` (not Ctrl+C or Ctrl+D) to shut down Claude Code in tests -- this is the only consistent way to ensure the process exits cleanly.

### App bundle freshness

Acceptance tests run a **prebuilt** `.app` bundle — the frontend is embedded at build time, so a stale bundle silently tests stale code.
`mise test-e2e` guards against this: it compares the bundle's `BUILD_FINGERPRINT` resource to a content hash of the working tree and rebuilds (`build-ember-app`) on mismatch.
`mise test-e2e -- --force` rebuilds unconditionally (the flag must come after the `--` separator, or mise's task shorthand swallows it).
The fingerprint has a single implementation, owned by the bundler: `bundler/macos-ember/bundle-fingerprint.sh`, whose input list is derived from `populate-app-resources.sh --list-sources`.
Never reimplement the hash — invoke the script (see `scripts/app-bundle.ts`).
The harness (`tests/support/hooks.ts`) independently refuses stale/incomplete bundles and raw binaries, so even direct `_cucumber` runs are protected.
`EMBER_E2E_PREBUILT=1` skips all of this and tests exactly the artifact that exists (raw binaries allowed; bundle-dependent scenarios will fail).

### Cassette recording

To record new cassettes for `@real-claude` tests:
1. Authenticate with the real Anthropic API using one of:
   - **OAuth (account login)**: Log in to Claude Code normally — the OAuth token is forwarded automatically
   - **API key**: Export `ANTHROPIC_API_KEY` with a real API key
2. Run `mise record-claude` (this sets `RECORD_MODE=1` automatically)
3. Inspect with `mise cassette-inspect <path-to-cassette.json.gz>`
4. Review and commit the compressed cassette file

Cassettes are stored as gzip-compressed JSON at `tests/support/fixtures/cassettes/`.

### Step definition conventions

- Steps should be readable as documentation
- Use `window.__test.*` for accessing app internals via the eval bridge
- Use `waitForTerminalContent()` for terminal output assertions (polling with timeout)
- Use `driver.pollFor()` for async state assertions
- Use `driver.ipcPost()` to simulate external tool communication
- Steps that change application state and are followed by another step that depends on that state (to further change it or to assert it) must block until the state mutation is complete. For example, a step that clicks a button to start a procedure must await that procedure's completion and verify it succeeded before returning control to the next step.

### UI interaction conventions

- Use `UIDriver` (`tests/support/ui-driver.ts`) for generic DOM interaction: `pressKey`, `click`, `typeIntoInput`, `getTextContent`, `getElementCount`, `waitForElement`
- Use `ws-hub-helper.ts` for Workspace Hub operations: `ensureWsHubOpen`, `reloadWsHub`, `getFocusedWorkspaceId`, `waitForWorkspaceItems`, `selectCategory`, `filterWorkspaces`
- Prefer hub-specific helpers over raw UIDriver calls when they exist

### Fixture conventions

- Use `fixture-helper.ts` to create test profiles, workspaces, and repos on disk
- `createTestFixtureRoot()` returns a temp directory; clean up with `cleanupFixtures()`
- `createTestProfile()` creates the profile directory structure (config.json, workspaces/, repos/)
- `createTestWorkspaces()` creates workspaces (`ws-1`..`ws-N`) with optional active/inactive split
- `writeTestConfig()` writes a config file pointing at the test profile paths
- Set `DEVORA_CONFIG_PATH` env var to point the Rust backend at the fixture config file instead of the real one
- All fixture setup steps (profile creation, workspace creation, repo state modifications) must come **before** the `And the Workspace Hub is open` step. This step triggers a fresh load from the backend, so placing it after all setup ensures the Hub renders correct, up-to-date data.
- `createSingleActiveWorkspace()` adds a single active workspace with an explicit id/title (real worktree) alongside existing ones — for tests that add a workspace out-of-band (e.g. verifying refresh)
- `createInvalidWorkspaces()` creates `initialized` workspaces with no repo subdirs; the hub renders these as invalid

## UI Conventions

### Keyboard navigation

- `q` should always close/dismiss overlays, pages, and panels — same as `Escape`. This is a vim-style convention that applies globally across the Ember UI.
- When an input field is focused, `q` is just `q`, and `Escape` should unfocus the input rather than closing the page.
- Overlays and panels should document their keyboard shortcuts in a legend at the bottom of the page.

### Overlay system

- **Tab-covering overlay**: covers entire window including tab bar. Used for the Workspace Hub, User Guide, cheatsheet.
- **Panel overlay**: covers main panel area only, tab bar visible. Tied to a specific session tab. Used for Crit.
- Both types are dismissible with `q` or `Escape`.
- Any `window` listeners or state owned by tab-covering overlay content MUST be released via the `onCleanup` hook passed to `showTabCoveringOverlay`; `dismissTabCoveringOverlay()` / `dismissActiveOverlay()` is the single canonical teardown.

## Error Handling

There is ONE sanctioned path per side (see ADR-002); errors are surfaced to the user by default:

- **TS**: call `invoke` from `src/invoke.ts` (never `@tauri-apps/api/core` directly — a unit test enforces this). A rejected command automatically calls `showError` (`src/errors.ts`): the error is recorded for test scraping, written to the log file at `/tmp/devora-ember-<TS>.log`, and shown as a persistent banner the user must explicitly dismiss.
- **TS opt-out**: `invokeLogOnly` (same module) for high-frequency or gracefully-degrading commands — log-file WARN only, no banner. Fire-and-forget callers must append `.catch(() => {})`.
- **TS, non-invoke errors**: call `showError(message)` directly.
- **Rust**: call `logging::report_error(app, message)` — writes to the log file and emits the `app-error` event, which the frontend shows via the same banner. Pure functions in `workspace.rs` collect non-fatal failures into a `warnings: &mut Vec<String>` out-parameter; the command layer forwards them to `report_error`.

This keeps errors auditable (log file), visible (banner), and test-asserted (the BDD After-hook fails any scenario with unexpected recorded errors).

Every new Tauri command must be registered in `lib.rs`, `build.rs`, and `capabilities/main.json`; `src-tauri/tests/acl_completeness.rs` fails if the three lists drift.

## Reusable UI Components

Components live in `src/ui/components/`, styles in `src/styles/components.css`.
Re-use existing components where possible, and add new ones as needed.

### Pattern

Factory functions that return `HTMLElement` (or a handle object for stateful components). No framework, no Web Components — plain TypeScript + imperative DOM.

### Adding a new component

- Create the factory in `src/ui/components/` (with a TSDoc)
- Add CSS to `src/styles/components.css`
- Add unit test in `src/ui/components/__tests__/`

### Running unit tests

- `mise test-unit` (one-time)
- `mise test-unit-watch` (watch mode)

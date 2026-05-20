## Acceptance Testing

### Testing philosophy

- All acceptance tests must replicate user actions as closely as possible. When tests run, UI changes should look as though a real user is interacting with the app.
- No shortcuts (like `--print`) that bypass the real user flow. Claude Code must be launched via `ccc` interactively, not via `claude --print`.
- Use `/exit` (not Ctrl+C or Ctrl+D) to shut down Claude Code in tests -- this is the only consistent way to ensure the process exits cleanly.

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

### UI interaction conventions

- Use `UIDriver` (`tests/support/ui-driver.ts`) for generic DOM interaction: `pressKey`, `click`, `typeIntoInput`, `getTextContent`, `getElementCount`, `waitForElement`
- Use `ws-hub-helper.ts` for Workspace Hub operations: `ensureWsHubOpen`, `reloadWsHub`, `getFocusedWorkspaceId`, `waitForWorkspaceItems`, `selectCategory`, `filterWorkspaces`
- Prefer hub-specific helpers over raw UIDriver calls when they exist

### Fixture conventions

- Use `fixture-helper.ts` to create test profiles, workspaces, and repos on disk
- `createTestFixtureRoot()` returns a temp directory; clean up with `cleanupFixtures()`
- `createTestProfile()` creates the profile directory structure (config.json, workspaces/, repos/)
- `createTestWorkspaces()` creates workspace directories with optional active/inactive split
- `writeTestConfig()` writes a config file pointing at the test profile paths
- Set `DEVORA_CONFIG_PATH` env var to point the Rust backend at the fixture config file instead of the real one

## UI Conventions

### Keyboard navigation

- `q` should always close/dismiss overlays, pages, and panels — same as `Escape`. This is a vim-style convention that applies globally across the Ember UI.
- When an input field is focused, `q` is just `q`, and `Escape` should unfocus the input rather than closing the page.
- Overlays and panels should document their keyboard shortcuts in a legend at the bottom of the page.

### Overlay system

- **Tab-covering overlay**: covers entire window including tab bar. Used for the Workspace Hub, User Guide, cheatsheet.
- **Panel overlay**: covers main panel area only, tab bar visible. Tied to a specific session tab. Used for Crit.
- Both types are dismissible with `q` or `Escape`.

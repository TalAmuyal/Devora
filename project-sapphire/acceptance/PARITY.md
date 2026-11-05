# Ember parity checklist

Every Ember acceptance scenario that Sapphire's run doesn't include, with Sapphire's coverage of the same behavior, or a gap (ADR-011).
Scenarios in `../../acceptance/features/` without an `@ember-only` tag run on both apps, so they need no row here.
When a row's gap closes, share the scenario if the phrases allow it, and delete the row.

Status:
- **Covered**: another Sapphire test checks the same behavior.
- **Not applicable**: the scenario checks Ember's implementation (the DOM, xterm.js, its test API) and has no native counterpart; the row says why.
- **Gap**: nothing in Sapphire checks it yet.

## Ember's own features (`project-ember/tests/features/`)

| Scenario | Status | Sapphire coverage, or what's missing |
|---|---|---|
| global-shortcuts: Font size changes inside the Workspace Hub, whose own keys claim bare digits | Covered | `features/key-routing.feature`: "With the Workspace Hub open, session shortcuts reach nothing, and the font size still changes". |
| global-shortcuts: Font size changes while a dialog is open | Covered | `features/key-routing.feature`: "The font size changes while a dialog is open, and its focused field doesn't get the key". It checks the terminal font size, since Sapphire has no UI font size. |
| profile-management: Hub profile dropdown items are clickable at their on-screen position | Covered | `UITests/WorkspaceOperationsUITests.swift` clicks the dropdown's rows with the real mouse. |
| profile-management: Hub filter text survives a round-trip through Settings | Covered | `features/workspace-hub-filter.feature` leaves the filter field with Escape before pressing `P`, since Sapphire's key steps keep the field's focus where Ember's leave it. |
| theme-loading: Theme CSS properties are applied on startup | Covered | Sapphire's native views use the system's colors; the bundled theme reaches the web content, whose themed Mermaid fill `features/preview.feature` checks. |
| workspace-hub: Header profile dropdown and burger menu are vertically aligned | Not applicable | It measures DOM boxes. The XCUITests attach screenshots of the hub instead. |

## Shared scenarios tagged `@ember-only` (`../../acceptance/features/`)

| Scenario | Status | Sapphire coverage, or what's missing |
|---|---|---|
| session-management: Create a new session | Not applicable | It calls Ember's test API to create a session with a given title. Sapphire's shell tabs get their titles from workspaces and Repurpose, which `features/repurpose-session.feature` and `features/persistence.feature` cover. |
| session-management: A backgrounded session is not squashed when the font size changes | Covered | `features/key-routing.feature`: "A smaller font refits every terminal, the background ones included". Ember's font shortcut scales the UI and must leave a hidden xterm.js terminal's columns alone; Sapphire's changes the terminals' font, so its scenario checks with `stty size` that the background terminal's PTY is refit to the same, wider size as the active one. |
| workspace-hub: Hub shell renders immediately on load | Gap | Nothing checks that the hub's header, legend and search bar show while its workspaces are still loading. |

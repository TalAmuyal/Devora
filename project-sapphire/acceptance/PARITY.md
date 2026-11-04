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
| git-shortcuts: Git shortcuts are available as bare commands | Gap | The core's `PATH` order is unit-tested (`devora-terminal`), and the M6 bundle smoke test will resolve the bundled tools. Nothing runs a shim in a Sapphire terminal yet. |
| git-shortcuts: Git shortcuts work in a non-zsh sub-shell | Gap | As above. |
| git-shortcuts: Git shortcuts can be disabled via config | Gap | Writing the setting is covered (`features/settings-hub.feature`), and the `PATH` without the shims is unit-tested (`devora-terminal`). Nothing checks a Sapphire terminal without them. |
| global-shortcuts: Font size changes inside the Workspace Hub, whose own keys claim bare digits | Covered | `features/key-routing.feature`: "With the Workspace Hub open, session shortcuts reach nothing, and the font size still changes". |
| global-shortcuts: Font size changes while a dialog is open | Covered | `features/key-routing.feature`: "The font size changes while a dialog is open, and its focused field doesn't get the key". It checks the terminal font size, since Sapphire has no UI font size. |
| profile-management: Hub profile dropdown items are clickable at their on-screen position | Covered | `UITests/WorkspaceOperationsUITests.swift` clicks the dropdown's rows with the real mouse. |
| profile-management: Hub filter text survives a round-trip through Settings | Gap | The feature's own note says Ember's key steps leave the filter field first and Sapphire's don't, so the scenario isn't shared, and nothing in Sapphire replaces it. |
| theme-loading: Theme CSS properties are applied on startup | Covered | Sapphire's native views use the system's colors; the bundled theme reaches the web content, whose themed Mermaid fill `features/preview.feature` checks. |
| workspace-hub: Header profile dropdown and burger menu are vertically aligned | Not applicable | It measures DOM boxes. The XCUITests attach screenshots of the hub instead. |

## Shared scenarios tagged `@ember-only` (`../../acceptance/features/`)

| Scenario | Status | Sapphire coverage, or what's missing |
|---|---|---|
| health-hub: Open the Health Hub from the Command Palette with a real health check | Gap | `@real-health` needs a real `debi health`; Sapphire's scenarios use the shared fixture. It can run against the bundled `debi` once M6 bundles it. |
| session-management: Create a new session | Not applicable | It calls Ember's test API to create a session with a given title. Sapphire's shell tabs get their titles from workspaces and Repurpose, which `features/repurpose-session.feature` and `features/persistence.feature` cover. |
| session-management: A backgrounded session is not squashed when the font size changes | Gap | It guards against xterm.js measuring a hidden terminal at zero width. Sapphire keeps every terminal in the view tree, but nothing checks a background terminal's columns after a font size change. |
| workspace-hub: Hub shell renders immediately on load | Gap | Nothing checks that the hub's header, legend and search bar show while its workspaces are still loading. |

# ADR-011: Sapphire Testing

## Status

Proposed, as part of TalAmuyal/Devora#230 (M6).
It records the approach Sapphire's tests have followed since TalAmuyal/Devora#244.

## Date

2026-11-04

## Context

Sapphire (ADR-006, ADR-010) must keep feature parity with Ember, whose acceptance tests (ADR-001) drive the real, built app through an eval bridge, with Gherkin scenarios run by Cucumber.js.
A native SwiftUI app has no JavaScript context to evaluate in, and some of what matters can't be observed from inside the app at all:

- real input: key events through AppKit, the IME, libghostty's key path, mouse drags and focus;
- what SwiftUI and WebKit actually draw.

At the same time, most behavior is the same in both apps (sessions, hubs, dialogs, previews, hooks), so the scenarios that describe it should be written once.

## Decision

- **Three layers, one command.**
	`mise run -C project-sapphire test` runs them in order, and CI runs the same command.
	- **Unit tests** (`swift test`) test the store and the models against the real core, not a mock of it.
	- **Acceptance scenarios** run Gherkin through Cucumber.js against the real, built app, which they drive through a test bridge.
	- **XCUITests** cover real input and what is drawn: real keys and mouse events, focus, and screenshots attached as evidence.
- **A test bridge, not an eval bridge.**
	The bridge is a loopback HTTP server, compiled into every build, that starts only when `DEVORA_TEST_MODE=1`.
	Its routes dispatch the store's own commands, click what a surface draws by running what its button runs, and report the state as the views see it.
	Steps poll the state, since events fold once per frame.
- **Shared Gherkin.**
	Scenarios that describe behavior both apps have live in `acceptance/features/`, run by both apps' runners, with step definitions for the same phrases in each app.
	`@ember-only` and `@sapphire-only` tags mark the exceptions, and features with no counterpart in the other app stay in that app's own `features/`.
	`project-sapphire/acceptance/PARITY.md` maps every scenario Sapphire doesn't run to its Sapphire coverage, or lists it as a gap.
- **Real dependencies.**
	Tests run real tools rather than fakes where they can: Debi and CCC are built from source (`test-tools`), workspaces are real git repos, and the Claude Code CLI runs against Ember's recorded cassettes.
	Only what would touch the user's machine is replaced: `DEVORA_TEST_MODE=1` selects in-memory credentials, and every scenario has its own config and state directory.
- **The window server is a requirement, not an option.**
	The acceptance scenarios need an unlocked screen, since the display link that folds events doesn't fire on a locked one.
	The XCUITests need it too, and `scripts/test-ui.sh` skips them, saying why, when the screen is locked or `SAPPHIRE_SKIP_UI_TESTS=1`.
	CI's macOS runners have a window server, so CI runs all three layers.
- **A failure leaves evidence.**
	A failed scenario saves the app state, the active terminal's text and the app's stderr.
	A scenario fails if an error banner was shown that no step expected, as in Ember (ADR-002).

## Alternatives considered

- **XCUITest only.**
	Not chosen: it can't read the app's state, it is slow, and its scenarios couldn't be shared with Ember.
- **The bridge only.**
	Not chosen: the bridge can't produce real input or see what is drawn, which is exactly where native bugs live (focus, the IME, libghostty's key path, WebKit painting).
- **A Swift BDD framework.**
	Not chosen: it would split the scenarios from Ember's, and parity is checked by running the same scenarios.
- **Pixel snapshot tests.**
	Not chosen for now: rendering differs between machines (a VM's WebKit paints web content blank), so screenshots are attached as evidence rather than compared.

## Consequences

- A new Sapphire feature adds its scenarios to `acceptance/features/` when Ember has the behavior, and its steps implement phrases Ember already uses.
- The bridge grows a route for each surface the scenarios drive, and stays inert outside test mode.
- Behavior that depends on real input or drawing needs an XCUITest besides its scenario.
- Ember's and Sapphire's desktop suites share the recorded Claude Code config, so they run one after the other on a machine.

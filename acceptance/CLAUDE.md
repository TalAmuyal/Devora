# Shared acceptance scenarios

`features/` holds the Gherkin scenarios that both Ember (`project-ember/tests/`) and Sapphire (`project-sapphire/acceptance/`) run, each with its own step definitions for the same phrases.

- Both runners load this directory next to their own `features/`: Ember's `mise run -C project-ember test-e2e` and Sapphire's `mise run -C project-sapphire test-acceptance`.
- A scenario runs on both apps unless tagged: `@ember-only` scenarios are excluded from Sapphire's run, and `@sapphire-only` ones from Ember's.
- Write steps in terms both apps share ("session", "workspace", "terminal"), and start from a known state rather than from either app's startup state (e.g. `Given Devora is running with no sessions open`).
- A scenario specific to one app, with no counterpart in the other, belongs in that app's own `features/`.

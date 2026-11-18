# Shared acceptance scenarios

`features/` holds the Gherkin scenarios that both Ember (`project-ember/tests/`) and Sapphire (`project-sapphire/acceptance/`) run, each with its own step definitions for the same phrases.

- Both runners load this directory next to their own `features/`: Ember's `mise run -C project-ember test-e2e` and Sapphire's `mise run -C project-sapphire test-acceptance`.
- A scenario runs on both apps unless tagged: `@ember-only` scenarios are excluded from Sapphire's run, and `@sapphire-only` ones from Ember's.
- `@bundled-tools` still runs in Ember's normal e2e suite. In Sapphire it moves a shared scenario from `test-acceptance` (the development app) to `test-bundle` (the signed distribution, after packaging); the standard `mise run -C project-sapphire test` invokes both. Use it when a scenario needs actual bundled tools or shims, never to exclude coverage from either app. `@real-health` additionally omits the report fixture so the Health Hub invokes the real bundled Debi.
- Write steps in terms both apps share ("session", "workspace", "terminal"), and start from a known state rather than from either app's startup state (e.g. `Given Devora is running with no sessions open`).
- A scenario specific to one app, with no counterpart in the other, belongs in that app's own `features/`.
- `stress.sh` is the loop behind Sapphire's `stress-acceptance` and Ember's `stress-e2e`: a filtered run, repeated on the efficiency cores, with a tally (see each app's CLAUDE.md for when a PR needs it).
- `hang-watchdog.ts` (with its worker and its Cucumber formatter, `hang-watchdog-formatter.ts`) names and stops a run that hangs where Cucumber's step timeouts can't, from a thread a blocked main thread doesn't stop. Sapphire's runs use it (see its CLAUDE.md); a harness reports its launched processes' output to it (`hangWatchdog().output`).
- `no-ssh` is the `GIT_SSH_COMMAND` of both apps' test runs (`NO_INTERNET` in `project-ember/tests/support/claude-helper.ts`, with the proxy variables): git over ssh fails at once, saying so (`no-ssh.test.sh` checks it). A direct `ssh` call can't be stopped from the environment.

# Test outcomes

Each CI job that runs tests records every test's outcome and duration, so a flake is judged by how often it fails, from CI's history, rather than by one green run.

`record.sh <job> <out.jsonl> <format>=<path>...` converts what the job's test runs wrote and appends one JSON object per test:

| Field | |
|---|---|
| `job` | The CI job (`sapphire`, …) |
| `format` | The results it came from: `swift-testing`, `xcresult`, `cucumber`, `vitest` or `rust` |
| `suite`, `name` | The test's identity (below) |
| `outcome` | `passed`, `failed` or `skipped` |
| `duration` | Seconds, or `null` where the runner doesn't report it |
| `run_id`, `run_attempt`, `sha`, `event`, `ref` | The run, from GitHub's environment |

The job then uploads the file as the artifact `test-outcomes-<job>-attempt-<n>`, kept 90 days, on every run, pass or fail. Nothing else stores it.
Recording never fails the job: a result that's missing (its step didn't run) or unreadable is said in the log (`test-outcomes: …`) and skipped.

## Identity

A test's identity (`suite` and `name`) must be the same on every run, or its history splits in two. It is built from names, never from positions in a file, which move whenever the file is edited:

- **swift-testing** (`swift-testing.sh`, from `swift test --event-stream-output-path … --event-stream-version 0`): the test's ID without its trailing `/<file>:<line>:<column>`. The suite is the module and nested suites (`AppShellTests.KeyRoutingTests`), the name the function (`shiftTapsOpenThePalette()`). A parameterized test is one outcome, failed if any of its arguments failed; a known issue isn't a failure.
- **XCUITests** (`xcresult.sh`, from the result bundle): the class and the method (`GlobalPauseUITests`, `testToolbarHoldSurvivesRelaunchAndThePaletteResumesIt()`). A test run with `-test-iterations` has one outcome per iteration.
- **cucumber** (`cucumber.sh`, from `--format json:<file>`): the feature's name and the scenario's, not `uri:line`. A Scenario Outline's rows share their name, so each gets its position among them (`… #2`): reordering an outline's rows renames them.
- **Vitest** (`vitest.sh`, from `--reporter=json --outputFile.json=<file>`): the test file's path from the repository's root, and the test's describe blocks and title (`findAppBundle > returns null when …`).
- **Rust** (`rust.sh`, from `cargo test`'s output, which the job saves with `tee`): the test binary by package, `<package>` for its unit tests, `<package>/<file>` for an integration test and `<package> (doc)` for its doc-tests, and libtest's name, a doc-test's without its `(line N)`. Stable libtest prints no per-test time, so a Rust test's `duration` is `null`.

Renaming a test starts a new history, under the new name. `test.sh` checks that editing a test's file keeps its identity and that renaming the test changes it.

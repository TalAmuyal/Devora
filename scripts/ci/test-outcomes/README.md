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
	libtest doesn't name an integration binary's package, so it's taken from the last `Running unittests` line before it: cargo runs each package's targets together, its unit tests first. That holds while every crate keeps its unit-test binary; a crate with `test = false` would file its integration tests under the package before it.

Renaming a test starts a new history, under the new name. `test.sh` checks that editing a test's file keeps its identity and that renaming the test changes it.

## Reading the history

`mise run flake-report [--runs N] [--test <regex>]` (`scripts/ci/flake-report.sh`, with gh's login) downloads the outcomes of the last N completed CI runs (30 by default), keeping each run's in `~/.cache/devora-flake-report/`, since an artifact never changes.
For each test that failed, it prints its failures out of the runs where it ran, on master and on pull requests apart, the last failure with its run, and its median and slowest durations.
A failure that passed when the job was re-run (an earlier attempt failed, the last passed, same commit) is flagged: it's the clearest sign of a flake.
Read the two columns differently: a failure on a pull request can be that pull request's own bug, in work not merged yet, while a failure on master, or one that passed on re-run, is the stronger sign of a flake.
`--test` narrows it to the tests whose suite and name match, and lists them all, failed or not.
Jobs GitHub cancelled before a runner took them are listed apart, as "not run", by GitHub's annotation on each (`flake-report-not-run.jq`): those it found no runner for, by job and runner label (a pool short of capacity, not a test result), apart from those a newer run of the same branch replaced (the workflow's own concurrency rule).
`flake-report.jq` is the report itself, and `flake-report.test.sh` checks it on made-up outcomes.

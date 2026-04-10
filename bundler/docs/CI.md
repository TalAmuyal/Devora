# CI Pipeline

The CI workflow runs the test suites for all sub-projects on every push to `master` and on every pull request targeting `master`.

If multiple runs are triggered for the same branch, the older in-progress run is automatically cancelled.

## Triggers

- **Push** to the `master` branch
- **Pull request** targeting the `master` branch

## What it tests

The workflow runs `mise run test`, which executes the test suites for the following sub-projects (in order):

- **project-debi** (Go)
- **project-judge** (Python)
- **project-status-line** (Go)

The runner is `ubuntu-latest`. Tool versions (Go, Python, etc.) are managed by [mise](https://mise.jdx.dev/) via `jdx/mise-action`.

## Running tests locally

To run the full test suite locally:

```
mise run test
```

This is the same command the CI workflow executes. You can also run tests for a single sub-project:

```
mise run -C project-debi test
mise run -C project-judge test
mise run -C project-status-line test
```

## Interpreting failures

When CI fails, the logs will show which sub-project's test suite failed.
The test commands run sequentially (`set -e`), so the first failure stops the run.
Check the failing sub-project's test output for details, reproduce locally with the corresponding `mise run -C <project> test` command, and fix from there.

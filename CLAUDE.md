@README.md

When reading/updating docs, make sure to also check `./USER_GUIDE.md` as well.

## Development and Testing

Dependencies like Python and Go are managed using Mise.
To run Go/Python commands, use `mise exec -- go ...` or `mise exec -- python ...` to ensure the correct environment is used.
Repeating tasks should be added to `mise.toml` for easier execution.

Inspect the root `mise.toml` (or the project-specific one) for available commands and tasks.

## Architecture Decision Records

ADRs document significant architectural decisions. They are stored in `docs/adrs/` and listed below.

When making a new architectural decision, create a new ADR file following the format of existing ones and add it to this list.

- [ADR-001: Ember Acceptance Testing Framework](docs/adrs/ADR-001-ember-bdd-testing-framework.md) — Eval bridge + Gherkin acceptance tests + Fake Claude API for testing the Tauri app

## Pull Request Guidelines

When submitting a PR, update the `## Unreleased` section in `CHANGELOG.md` with a brief description of the change.
Add plain bullet points under the right type/category heading of Unreleased.
Add a type/category sub-headings like Added/Changed/Fixed if it doesn't already exist.

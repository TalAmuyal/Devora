# Contributing to Devora

Contributions are welcome. Whether it's a bug report, feature request, or code change -- thank you for helping improve Devora.

## Reporting issues

Use [GitHub Issues](https://github.com/TalAmuyal/Devora/issues) for bug reports and feature requests.

## Development setup

Devora requires macOS (Apple Silicon) and [mise](https://mise.jdx.dev/) for development.

```sh
git clone https://github.com/TalAmuyal/Devora.git
cd Devora
mise trust
mise build
mise test
```

mise handles Go and Python toolchain installations automatically.

## Pull request process

1. Create a feature branch from `master`
2. Make your changes
3. Update `CHANGELOG.md` -- add a brief description under the `## Unreleased` section, under the appropriate category (Added, Changed, Fixed, Removed)
4. Ensure tests pass: `mise test`
5. Submit the pull request

## Changelog enforcement

A `Changelog` GitHub Actions workflow verifies that every PR touches `CHANGELOG.md`. PRs that don't will fail the check.

Exemptions:
- PRs authored by bot accounts (e.g. Dependabot) are skipped automatically
- Maintainers can add a `skip-changelog` label to exempt a specific PR that has no user-visible impact (typo fixes, CI-only changes, etc.)

An optional local pre-commit hook is available. Install it once with `mise run install-hooks`. It blocks commits when `CHANGELOG.md` is not staged; bypass with `git commit --no-verify` for WIP commits.

## Code style

Match the style of surrounding code. Consistency within a file is more important than external style guides.

## Testing

All new features and bug fixes should include tests.

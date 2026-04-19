@README.md

## Spec Maintenance

When modifying a package's exported API (adding, removing, or changing exported function signatures), update the corresponding spec in `docs/specs/`.

## Precondition handling

Commands that depend on external preconditions (being inside a git repository, being inside a Devora workspace, having configured credentials) must detect the missing precondition and return a typed error that the CLI layer translates to a `*cli.UsageError` with a human-readable message. Never let a raw error from a precondition probe bubble to `crash.HandleError` — crash logs are for unexpected failures, not for expected missing context.

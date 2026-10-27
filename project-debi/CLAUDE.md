@README.md

## Documentation

Don't write package specs or other separate per-package docs.
Write the "why" into the code itself, as package and type doc comments.

## Precondition handling

Commands that depend on external preconditions (being inside a git repository, being inside a Devora workspace, having configured credentials) must detect the missing precondition and return a typed error that the CLI layer translates to a `*cli.UsageError` with a human-readable message.
Never let a raw error from a precondition probe bubble to `crash.HandleError` — crash logs are for unexpected failures, not for expected missing context.

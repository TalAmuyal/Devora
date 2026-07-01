@README.md

## Spec Maintenance

Previously, we've been writing package specs.
However, we should change our approach and instead write the why into the code itself.
Going forward, we should move the "spirit" of the spec into the code, and avoid writing new specs.

As for existing specs, we should gradually migrate them into the code as well, and remove the parts of the spec that are already covered by the code itself.

## Precondition handling

Commands that depend on external preconditions (being inside a git repository, being inside a Devora workspace, having configured credentials) must detect the missing precondition and return a typed error that the CLI layer translates to a `*cli.UsageError` with a human-readable message.
Never let a raw error from a precondition probe bubble to `crash.HandleError` — crash logs are for unexpected failures, not for expected missing context.

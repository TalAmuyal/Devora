# Unit tests

- The store tests run in parallel on the one main actor, where the store folds its events, so a test that blocks it (a `waitUntilExit()`, a synchronous wait) stalls every other test's `eventually`; their fixtures await git through `runGit` instead.
	Opening a state another store holds blocks it for the core's busy timeout (1 s), which only `aStateHeldByAnotherStoreIsInUse` does.

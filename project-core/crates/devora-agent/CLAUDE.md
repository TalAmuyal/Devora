# The agent runtime

## Stopping a CLI

The session worker reports each CLI it spawns (`SessionEvent::Spawned`), identified by its pid and start time (`CliProcess`), since pids are reused; only a process matching both is ever signalled.
A forced stop, after the stop's grace, stops the CLI's whole process tree (`stop_tree`, over `devora-platform`'s process table): Claude Code makes each background shell a session and process-group leader of its own, so neither a process group nor a session reaches them, while the CLI's descendant tree does. A graceful stop is unchanged: the CLI exits by itself once its background work has ended.
`stop_tree` freezes the tree first (`SIGSTOP`, walking parent links until no new process turns up), then kills everything it collected, even when the walk fails part way. Its known limit: a descendant already reparented to launchd (or init) isn't found.
`StopInfo` reports whether a stop was graceful and whether it interrupted a turn.

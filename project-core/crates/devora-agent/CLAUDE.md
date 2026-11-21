# The agent runtime

## Stopping a CLI

The session worker reports each CLI it spawns (`SessionEvent::Spawned`), identified by its pid and start time (`CliProcess`), since pids are reused; only a process matching both is ever signalled.
A forced stop, after the stop's grace, stops the CLI's whole process tree (`stop_tree`, over `devora-platform`'s process table): Claude Code makes each background shell a session and process-group leader of its own, so neither a process group nor a session reaches them, while the CLI's descendant tree does. A graceful stop is unchanged: the CLI exits by itself once its background work has ended.
`stop_tree` freezes the tree first (`SIGSTOP`, walking parent links until no new process turns up), then kills everything it collected, even when the walk fails part way. Its known limit: a descendant already reparented to launchd (or init) isn't found.
`StopInfo` reports whether a stop was graceful (the CLI exited by itself within the grace, whatever its exit code: it exits 1 after an interrupted turn) and whether it interrupted a turn.

## Background work

The CLI lists the agent's background tasks (background shells, `local_bash`, and subagents, `local_agent`) whenever the list changes (`System::BackgroundTasksChanged`), and `SessionHandle::stop_task` ends one through the CLI: its processes stop, with no model turn for a shell, while a stopped subagent makes the CLI run a notification turn of its own. The pinned-CLI suite asserts both, and that the interrupted turn runs no tool.

## The models the CLI offers

`models::decode` reads the `models` list of an initialize reply: each entry's `value` (what to pass and store; an alias such as `opus` follows the CLI to newer models), what it resolves to today, its display name and effort levels, plus the reply's `claude_code_version`. A real login's list has the aliases first, then full IDs of other models, older ones included; an API-key-only setup gets just the aliases.
`models::discover` gets that reply without a session: an initialize-only launch with the session's program, environment and settings (so the login and tier settings are an agent's), plus `--strict-mcp-config`, `--no-session-persistence` and `disableAllHooks`, so it starts none of the user's MCP servers, plugins' servers or hooks; closing its input ends it, and it leaves no session file. Not `--bare`, which ignores the OAuth login. The pinned-CLI suite proves all of that against a setup where a plain launch starts each one.
The core caches the list per profile (`devora-core`'s `agents/models.rs`): every agent start fills it from its `Ready` reply, logging the list at Info, and a UI's request probes only when the cache is a day old, from another CLI version, or from other launch settings (a changed model tier resolves the aliases differently).

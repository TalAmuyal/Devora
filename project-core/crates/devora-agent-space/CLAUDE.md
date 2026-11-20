# Agent Spaces

## The restart notice

After a restart, each agent gets one notice to re-establish its waits, since its background work (watchers, builds, CI waits, background shells and subagents) and their notifications don't survive the CLI's stop. Its owner, with the whole lifecycle (owed, attempt, delivered, recovery), is `restart_notice.rs`. The registry makes one `AppRun` per process and passes it to every launch. An agent with a conversation whose CLI last ran in another run owes a notice, which is the first turn its launch admits: a system row (`SystemKind::RestartNotice`) committed with the turn's ID before the send, and cleared by the result naming that ID. Holds, Keep paused and individual pauses only defer it to the agent's launch, and drafts stay untouched. A launch after a notice was cut short reads Claude's saved session (`devora_agent::session::saved_turn`): the entry with that ID means it was delivered, and otherwise it is sent again, marked as a recovery. So delivery is at least once.

A stop that was forced or interrupted a turn (`StopInfo`) also owes the notice, worded for the pause, at the agent's next launch in the same run; a clean, idle stop owes none.

## The agent's compaction threshold

Schema version 5 adds `space.compact_threshold`: the agent's own threshold (1 to 100), over its profile's `agent.compact-threshold`; null follows the profile. `SpaceHandle::set_compact_threshold` commits it and applies it at once, through the actor, which owns the database, so it needs a running agent. `SpaceHandle::compact_now` starts the save-memory and `/compact` steps on an idle agent, or flags them for the running turn's result, as reaching the threshold does; a failed or interrupted turn drops that request, as it abandons a compaction. The context the actor publishes carries the threshold in force, the profile's (as read at launch), the agent's own as stored (which may equal the profile's, and still pins it), and the compaction's progress (idle, requested, under way).

## Pausing with background work

The actor keeps the agent's background tasks as the CLI lists them (`System::BackgroundTasksChanged`). A pause whose agent has some doesn't close the CLI's input at the turn's boundary: it owes the notice naming them first (so a crash mid-pause still tells the agent), stops each one through the CLI (`stop_task`, shells first), interrupts the notification turn a stopped subagent makes the CLI start, and waits for that turn's result, since once the input is closed nothing can interrupt a turn. Then it closes the input, and the CLI exits by itself (`Preparing` in `running.rs`). Past `PREPARE_GRACE`, or when a task won't stop, the pause goes ahead and the worker's forced stop is the fallback, so at worst a pause takes `PREPARE_GRACE` plus the stop's own grace, about 10 s. An agent with no background work pauses as before, owing nothing. A process the agent daemonized isn't a task the CLI tracks, so it outlives the pause (ADR-013).

## The running CLI

Schema version 4 adds `space.cli_process`: the running CLI's identity, its pid and start time (`devora_agent::session::CliProcess`, from `SessionEvent::Spawned`, which the space keeps to itself). It is cleared when the CLI ends under Devora's watch. A launch that finds a recorded CLI still running (an earlier Devora died without stopping it) stops its whole tree (`stop_tree`) before resuming, since two CLIs on one session fork it. A pid whose start time differs is another process and is left alone. An orphaned CLI can't run tools meanwhile, since its Judgess callback can't be answered.

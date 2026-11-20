# ADR-013: Global Pause

## Status

Accepted.

## Date

2026-11-07

## Context

Agents and the planned periodic repository updater need a shared pause control.
Each integration has a useful graceful boundary, an agent turn or one repository update.
ADR-008 gives each app and suffixed installation its own durable app state.

## Decision

One core-owned global paused/running value persists in app state.
A named command commits it before publishing the change.
A subscription provides the current value and subsequent committed changes.
Each integration finishes its admitted work unit and holds the next unit until released.
Consumers also check the authoritative hold when admitting work.

Agents use their existing pause-after-turn boundary, including the boundary before automatic compaction turns.
A refused user send keeps its draft for an explicit later send.
Within a run, each integration tracks its global-hold eligibility separately from individual pause intent.
On a fresh launch, the saved global hold gates normal restore, and release resumes the open agents that normal startup would have resumed.
Sapphire provides the first controls, with Ember adopting the contract alongside periodic repository updates.

Amended on 2026-11-20: an agent's graceful boundary includes its background work, so that a pause is only a pause.
Once its turn has ended, a pause stops each background task the CLI tracks (background shells and subagents) through the CLI's own `stop_task`, interrupts the notification turn a stopped subagent makes the CLI start (else the agent could start the work again), and only then closes the CLI's input, so the CLI exits by itself.
At its next launch the agent is told which work was stopped, to re-establish what it still needs.
A task that won't stop, or a CLI that doesn't exit within the stop's grace, falls back to a forced stop of the CLI's whole process tree.
Known limit: a process the agent deliberately daemonized (for example `nohup … &`) isn't tracked by the CLI, so it outlives the pause, and the notice can't name it.

## Consequences

Additional integrations share the durable control and choose their own graceful boundary.
Per-agent resume eligibility follows the in-memory runtime lifecycle.
Current APIs, lifecycle handling and UI behavior are documented in `project-core/CLAUDE.md` and `project-sapphire/CLAUDE.md`.

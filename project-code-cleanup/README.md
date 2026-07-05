# Code-Cleanup

A Claude Code plugin that defines and directs a general post-task cleanup process.

It ships with Devora and is loaded automatically when Claude Code is launched from a Devora session.

## Contents

- `cc-plugin/skills/code-cleanup/` — the orchestrator skill; determines the scope (the current uncommitted changes) and directs the cleanup
- `cc-plugin/skills/declutter.md` — a dedicated agent that finds the simplest correct version of the changed code while keeping behavior identical
- `cc-plugin/skills/comment-cleanup.md` — a dedicated agent that removes redundant comments, biasing to zero

## Design notes

### Why an orchestrator skill + skill-agents?

- The instruction bulk lives in the agent definitions, so invoking the skill injects only a small orchestration body into the main conversation, and the agents' verbose work (reading every changed file in full) stays out of the main context.
- Since skills with `context: fork` starts a fresh subagent (without the task context the main agent has), the orchestration skill directs the main agent to compose self-contained briefs: what the task was, what is a step of a larger in-progress change and must not be undone, and the scope.

### Why sequential, declutter first?

Both agents edit the same files, so they must not run in parallel.
Decluttering rewrites and removes code, so the comment cleanup runs last and judges comments against the final code.

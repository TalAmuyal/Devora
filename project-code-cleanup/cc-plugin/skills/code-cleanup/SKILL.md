---
name: code-cleanup
description: >-
  Post-task cleanup of the current uncommitted changes: declutters the code (simplest correct version, behavior preserved) and removes redundant comments.
when_to_use: >-
  When asked to clean up, tidy, simplify, polish, or declutter recent changes, or when wrapping up completed work (e.g., preparing changes for a PR).
  Do not run unprompted after routine tasks.
user-invocable: true
disable-model-invocation: false
argument-hint: Optional scope (paths) or extra instructions
---
Please run the post-task cleanup process on the current changes.

## Current state

!`debi git status --short`

## Scope

The cleanup targets the current uncommitted changes, including untracked files, unless the arguments below narrow it.
If there is truly nothing to clean, say so and stop.

## Process

Spawn the following agents sequentially - they edit the same files, so NEVER run them in parallel.

Steps:
1. Invoke `code-cleanup:declutter` - simplifies the changed code
2. Invoke `code-cleanup:comment-cleanup` - removes redundant comments (last, so comments are judged against the final code)
3. Perform a final review of the combined changes (diff + rerun tests if applicable) to verify that behavior is preserved and nothing was broken
4. Report the results.

Each agent starts with no knowledge of this conversation.
Its prompt must be fully self-contained and include:
- A summary of the task that was just completed, and its intent
- Whether this work is one step of a larger in-progress change, and what must not be undone
- The list of changed/untracked files in scope
- Any constraints from the arguments below

## Reporting

After both agents finish, relay a combined report:
- **Changed:** per file, one line per simplification/removal, integrated with comment-cleanup's "Removed" and "Kept" sections
- **Behavior preserved:** how it was verified
- **Left as-is:** with reasons
- **Needs your call:** anything skipped because it would change an interface, carries risk, or otherwise flagged

Do not commit anything.

## Arguments

$ARGUMENTS

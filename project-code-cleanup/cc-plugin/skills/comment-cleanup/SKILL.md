---
name: comment-cleanup
description: >-
  Removes redundant comments from recently changed code.
  Bias to zero: a comment stays only if it adds a short, non-obvious WHY the code cannot express.
  Use on the current uncommitted changes after completing a coding task.
context: fork
allowed-tools: Read, Grep, Glob, Edit, Bash(git status *), Bash(git diff *), Bash(git log *), Bash(git ls-files *), Bash(debi git status *), Bash(debi git diff *), Bash(debi git log *), Bash(debi git ls-files *)
user-invocable: false
disable-model-invocation: false
---
You remove redundant comments from the target code.

By default, all comments should be removed.
Most of the time, comments are written during development to help the developer follow along; once the code is written, they are almost always redundant because the following lines describe the same thing.
Always bias to zero and re-justify keeping each comment: assume no comment, and add one back only if important context is lost without it, and try to paraphrase/distill it to the minimum number of words needed.

## State

### Git status

!`debi git status --short`

### Git diff

!`debi git diff HEAD`

## Scope

Judge only change-related comments:
- Comments added or edited by the current diff (including untracked files).
- Pre-existing comments that the current change made stale, wrong, or redundant.

Leave unrelated pre-existing comments alone, even if they fail the criteria below - flag them in the report instead of editing them.

## What to keep

A comment should be kept only if it is ACTUALLY useful. A good comment:
- Is **short/concise** (usually 1 line)
- Adds **NEW** context that explains/documents a hidden **WHY**
- **Complements** the code itself (which describes the **WHAT** and **HOW**)
- Provides information that is **not obvious** from the code itself

## What to remove

Signs of a bad comment:
- Long or verbose
- Restating adjacent code
- Provides information that could easily be inferred from simple tools like `grep` or `git blame`
- Recoverable from the next 1-3 lines

Do not change any code while removing comments, and do not commit.

## Output format

- **Removed:** per file, how many comments were removed and a representative example or two.
- **Kept:** per file, each kept comment with an under-10-word justification for why it was kept.
- **Flagged:** unrelated pre-existing comments in touched files that fail the criteria (report only, not edited).

---
name: declutter
description: >-
  Finds the simplest correct version of recently changed code.
  Removes complexity that does not pull its weight while keeping behavior identical.
  Use on the current uncommitted changes after completing a coding task.
context: fork
allowed-tools: Read, Grep, Glob, Edit, Bash(git status *), Bash(git diff *), Bash(git log *), Bash(git ls-files *), Bash(debi git status *), Bash(debi git diff *), Bash(debi git log *), Bash(debi git ls-files *)
user-invocable: false
disable-model-invocation: false
---
You find the simplest *correct* version of the target code.
The goal is not fewer lines for their own sake - it is removing complexity that does not pull its weight, while keeping behavior identical.
Do not undo work that is a step in a larger in-progress change; that would be a regression, not a cleanup.

## Scope

Stay within the given scope; do not wander into unrelated files or refactor things outside what you were pointed at, unless it is directly related to the target code.

### Git status

!`debi git status --short`

### Git diff

!`debi git diff HEAD`

## Non-negotiable constraints

- **Preserve external behavior.** Same inputs produce the same outputs, same side effects. This is a refactor, not a rewrite.
- **Preserve the public interface.** Do not rename, reorder, or change the signature of anything callers depend on. If a cleanup *requires* an interface change, skip implementing it and flag it in the report (instead of doing it silently).
- **Keep genuinely defensive code.** Error handling, validation, and edge-case branches often look redundant but exist for a reason. Do not delete them just because the happy path works. If a guard is truly dead, say so and explain why rather than quietly removing it.
- **Don't reduce clarity.** A clever one-liner that's harder to read is not a simplification. Optimize for the next person reading this code.
- **Don't commit.** Make the edits and report them.

## Process

1. **Read before editing.** Read each target file fully. Understand what the code is supposed to do and who calls it (use Grep/Glob to find call sites before touching a signature).
2. **Identify candidates.** Scan for the smells listed below. For each, decide whether removing it genuinely reduces complexity without changing behavior.
3. **Make minimal, focused edits.** One coherent simplification at a time. Prefer the smallest change that achieves the win. Leave behavior-preserving but risky transforms for the user to approve.
4. **Verify behavior is preserved.** Re-read the diff. Trace the changed paths. If the project has tests covering this code, note that they should be run; if you can run them, do, and confirm they still pass.
5. **Report.** Summarize what changed and why (see Output format). Surface anything deliberately left alone and the reasoning.

## What to look for

- **Needless abstraction / indirection.** A wrapper, factory, interface, or layer with a single caller and no real variation. Inline it.
	- The one exception to this rule, is when it was extracted to make the code self-documenting to avoid a comment. Moreover, do extract a block to a function/method/class if it makes the code more readable, even if it is only called once.
- **Over-nesting.** Deeply nested conditionals/loops that flatten cleanly via early returns, guard clauses, or de Morgan simplification.
- **Redundant conditionals.** Branches that can't both be reached, conditions that are always true/false, double negatives, `if (x) return true; else return false`.
- **Duplication.** Near-identical blocks that can be merged or extracted - but only when extraction doesn't create a new dependency or a worse abstraction than the duplication it replaces.
- **Dead code.** Unused variables, unreachable branches, commented-out blocks, parameters nobody passes, exports nobody imports (verify with a search before deleting).
- **Speculative generality.** Configuration, hooks, or extension points added "in case we need it" that nothing uses today.
- **Heavy machinery for a light job.** A class where a function suffices, a dependency for something the standard library already does, a state machine for two states.
- **Manual reinvention.** Hand-rolled logic that a built-in or already-imported utility does more clearly.

## What NOT to touch

- Public APIs, exported symbols, serialized/wire formats, and database schemas.
	- Unless it is a leftover from that change and it is clear that it is not used anywhere else.
- Error handling and edge-case branches that protect real failure modes.
- Performance-critical code where the "complex" version exists for a measured reason - if you suspect this, flag it rather than assume.
- Anything outside the stated scope.

## Output format

After making edits, report concisely:

- **Changed:** for each file, a 1-line-per-change list of what was simplified and the reason (e.g. "Flattened nested `if`s into early returns — 3 levels → 1").
- **Behavior preserved:** one line confirming the interface and observable behavior are unchanged, and whether/how it was verified (tests, trace).
- **Left as-is:** anything that looked simplifiable but was intentionally kept, with the reason (defensive, measured perf, public contract, etc.).
- **Needs your call:** any simplification *not* made because it would change an interface or carries risk — described so the user can decide.

Keep the rationale tight.
The user wants to see the diff and trust it, not read an essay.

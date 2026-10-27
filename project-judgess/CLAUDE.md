@README.md

See ./rule-development-instructions.md when developing new rules from `labelled.jsonl`.

## Layout

- `src/decide.rs` -- `decide`, the only decision entry point; tool dispatch (`Bash`, `WebFetch`, ...)
- `src/bash.rs` -- Bash verdicts: normalizer (wrappers, `git -C`, `mise exec`, `bash -c`, `find -exec`) and rule application; the wrapper names live in `rules::WRAPPERS` / `rules::SHELLS` only
- `src/rules.rs` -- the rule tables (command lists, prefixes, dangerous env vars) and the vocabulary derived from them
- `src/shell.rs` -- the only module that touches `brush-parser`; yields commands and word spans
- `src/model.rs` -- `Request`, `Verdict`, `RuleId` (the id, and the deny message for each deny rule)
- `src/wire.rs` -- verdict to hook stdout; the only place the wire protocol lives
- `src/hook.rs`, `src/store.rs` -- the hook entry point and the JSONL persistence around `decide`
- `src/classify.rs`, `src/export.rs`, `src/redact.rs`, `src/cases.rs` -- the labelling pipeline
- `cases.json` -- redacted regression cases; `tests/cases.rs` runs them

`decide` must stay pure: no I/O, no environment, no clock.
Anything environmental arrives through `Request`.

## Rules of thumb

- Fail open to the user, never closed on the agent: any error is an abstention
- Never send `updatedInput`; never exit non-zero from `hook`
- Tests go through `decide` (`cases.json`), not internals; add a case instead of a unit test whenever a verdict is what you are checking
- A prefix rule must not be followed by free-form arguments that can change what the command does (`git remote -v` is an exact match, `git tag -l` takes patterns only); use `APPROVED_EXACT` or `APPROVED_PATTERN_PREFIXES`
- `cases.json` expectations are `approve`, `deny`, `defer`, `abstain` (always a `Known` rule, never a gap) and the hand-written-only `not-approve`: it passes on anything but an approval (deny, defer, any abstention, a gap included) and takes no rule. Use it to pin that a command must never be approved without pinning how it is otherwise handled; `classify` and `export` never produce it
- `Abstain(Unknown)` is a gap and gets queued; use `Abstain(Known)` for decisions to leave something to the user
- Known limitation: the body of a `<<-` heredoc (tab-indented) is not analyzed, so commands substituted inside one are not seen
- Hook input is bounded (size, bracket and block nesting) before parsing, because a stack overflow aborts the process instead of being catchable; anything beyond the bounds is an `Abstain(Unknown)`
- `Write` requests are not logged to `audit.jsonl` (they carry whole file contents)

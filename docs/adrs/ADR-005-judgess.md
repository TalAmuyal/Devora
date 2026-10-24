# ADR-005: Judgess (Permission Hook)

## Status

Accepted

## Date

2026-10-18

## Context

By default, Claude code protects the user by providing a tool-usage permission system.
However, it incurs a high friction.
By auto-approving the commands the user always approves and declining those that break conventions, less friction is incurred while still providing protection.

Claude Code also provides auto-mode, which uses another agent to approve or deny commands without prompting the user.
While this removes friction, it also reduces the user's control over what is run on their machine.
A hook-based mechanism allows the user to provide their own rules for specific use-cases, which also reduces cost and latency of the auto-mode agent.

## Decision

Make **Judgess**, a Rust binary (`project-judgess/`), with these decisions:

- **A real shell parser**
	- `brush-parser` yields every simple command that would execute (including nested substitutions) and byte spans for every word.
	- Any parse error is an abstention.
	- The parser is hidden in `shell.rs`; swapping it should not touch rules.
- **A single deep module**
	- `decide(&Request) -> Verdict` is pure; the hook binary, persistence, classify and export are thin layers around it.
	- This also removes any need for a "test mode".
- **A four-way verdict collapsed to three on the wire**
	- `Approve`, `Deny`, `Defer` (the user/auto-mode must decide, declared) and `Abstain` (`Known`: deliberately left alone; `Unknown`: a gap to be queued).
	- Only allow and deny produce output; `Defer` is silent for now, because forcing a prompt even in auto mode needs a `PreToolUse` hook returning `ask` (a follow-up that only changes the wire mapping in `wire.rs`).
- **Compound verdicts by precedence**: `Deny > Defer > Unknown > Known > Approve`, so the agent gets the redirect first.
- **A normalizer over more rules**: Wrappers (`timeout`, `env`, `xargs`, ...), `git -C`, `mise exec --`, `bash -c`, and `find -exec` reduce to the commands they run, which are then judged by the ordinary rules.
- **Vocabulary-based redaction with a raw -> labelled -> exported pipeline**
	- The hook queues raw unknown requests locally; `classify` labels them; once the rule exists, `export` redacts and re-decides the case, and only commits it to `cases.json` if the verdict survives.
	- The vocabulary is derived from the rule tables, so a rule's tokens cannot be redacted away.
- **Never fail closed on the agent**: errors and panics produce no output and exit 0.

### Parser spike (Step 0)

`brush-parser` (0.4) was chosen over `tree-sitter-bash` after enumerating the commands of every adversarial input below and obtaining word spans: newline and `&` separators; substitutions inside double quotes (`$(...)` and backquotes); unquoted heredocs (expand) vs quoted heredocs (literal); process substitution; `for`/`if`/subshell/brace groups; substitution inside arithmetic; inside an assignment; inside a redirect target.

It returns a typed AST with word locations, a word-piece parser that exposes substitutions with their offsets, and heredoc bodies, and it is pure Rust (no C toolchain).
Limits found and handled conservatively (the construct becomes `Abstain(Unknown)`, or the command is not redacted in place):
- Substitutions inside `${var:-...}` are left as raw text by the word parser.
- Standalone `(( ))` and the arithmetic of `$(( ))` carry no span of their own.
- Backquoted substitutions are unescaped, which shifts offsets.
- Function definitions and coprocesses are not analyzed.
- The body of a `<<-` heredoc (tab-indented) is not analyzed; the hook is for a careless agent, not a hostile one, so this is a known limitation.

## Consequences

- Adds a Rust build to the bundle (`bundle-ember.sh` builds it; `populate-app-resources.sh` copies the binary into the plugin directory).
- Rules and tests are driven by `cases.json`; the labelling pipeline keeps real usage as the source of new cases without committing private data.

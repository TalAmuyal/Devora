## Developing rules from `labelled.jsonl`

1. Read `~/.claude/judgess/labelled.jsonl`; each line is `{request, label, note?}` with the raw request
2. `cargo test` (via `mise run test`) prints the labelled cases whose label disagrees with the current verdict; that is the work list
3. Make `decide` return the labelled verdict:
	- Approve: add the command or prefix to the tables in `rules.rs`
	- Deny: add a `RuleId` (with its message in `model.rs`) and a table entry; a deny must always carry a redirect the agent can act on
	- Abstain: add a `RuleId` and a `KNOWN_PREFIXES` entry, or a check in `known_rule`
	- Defer: add a `RuleId` and produce `Verdict::Defer`; the wire collapses it into an abstention
4. Run `mise run -C project-judgess export` (`judgess export --cases <path>` for another file; the default is `cases.json` in the working directory).
	Each case is redacted, re-decided, and only exported if the verdict (kind and rule) survives redaction; the ones that do not stay in `labelled.jsonl` with the reason

Keep the line: deny convention violations that lose nothing, abstain on irreversible loss of work, abstain on anything unfamiliar.

If you suspect that the user made a label mistake, bring it up and discuss it before making changes.

## Rule and vocabulary coupling

Redaction keeps a token verbatim only if it is in the vocabulary: every literal in the `rules.rs` tables plus the static lists at the top of `redact.rs`.
So a new rule is automatically safe to export, as long as its literals live in the tables.
If a rule matches on a literal in code (a `match` arm), add it to `NORMALIZER_WORDS` in `rules.rs`, or `export` will hash it away and the redacted case will no longer reproduce the verdict.
For ordinary tokens (a common command, directory, extension, host) extend the static lists in `redact.rs`.

Redaction also hides values that look like words: a short option followed by more than a small cluster of flag letters (`-uroot`, `-mmessage`) keeps only its first letter, an option with an attached value (`-i.bak`, `-I{}`) keeps the option and judges the value alone (so `.bak` and `{}` are vocabulary), a script attached to an option (`env -S'...'`, `mise exec -c'...'`) is redacted as a script, and a number survives only if every dotted part has at most four digits (so IP addresses, account ids and phone numbers do not).
The `export` leftover guard looks for the home directory and username as whole tokens, so a username like `tal` does not match `install`.

Redaction is applied on the parsed command using word spans, so only private words change.
A command it cannot map back onto the source safely (functions, substitutions inside `${...}`, backquotes with escapes) becomes a single `text-<hash>` and will not reproduce its verdict; those stay local.

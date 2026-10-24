# Judgess

A Claude Code `PermissionRequest` hook that removes permission friction for a trusted coding agent.

- Approves what you always approve (`git status`, `ls`, `mise run ...`, ...)
- Denies, with a redirect the agent can act on, what violates your conventions (`go build` -> `mise exec -- go build`)
- Stays out of the way (no output, so Claude Code's normal flow decides) for anything irreversible or unfamiliar

It is not a sandbox.
The threat model is a cooperative-but-careless agent, and the allow-list is "what I would click yes on".

## Verdicts

`decide(&Request) -> Verdict` is a pure function; everything else is a thin layer around it.

| Verdict | Meaning | On the wire |
|---|---|---|
| `Approve` | Allowed | `behavior: allow` |
| `Deny { rule }` | Convention violation; the rule's message is the redirect | `behavior: deny` + message |
| `Defer { rule }` | The user/auto-mode must decide (declared intent) | nothing (collapsed into an abstention) |
| `Abstain(Known { rule })` | Deliberately not decided (`rm`, `Edit`, ...) | nothing |
| `Abstain(Unknown { reason })` | A gap in the rules; queued for classification | nothing |

A Bash command is split into every simple command that would execute (including those inside `$(...)`, `<(...)`, `bash -c`, `find -exec`, loops and subshells), and the worst verdict wins: `Deny > Defer > Unknown > Known > Approve`.

Wrappers and nested shells are reduced to the commands they run: `timeout`, `time`, `nice`, `xargs`, `bash -c`, `find -exec`, etc.
An absolute path into a standard system directory (`/bin/rm`) is the bare command.
`--help` and `--version` approve a command only when it is the sole argument and the command is a bare name (a script run by path may ignore its arguments); the exception is `git <subcommand> --help`, which opens the manual.
A `./x` command that no table approves is a local script, a known abstention; other path-run commands (`scripts/x.sh`, `node_modules/.bin/tsc`) are gaps, queued for classification.

The hook always exits 0 and never fails closed: malformed input, a panic, or an unparseable command all produce no output.

## Usage

Devora loads the plugin automatically (`ccc`).
Tasks are run with mise (`classify` and `export` are developer tools for a Devora source checkout):

```
mise run -C project-judgess build      # release binary
mise run -C project-judgess test       # regression cases + unit tests
mise run -C project-judgess classify   # label the requests Judgess did not know how to decide
mise run -C project-judgess export     # redact labelled requests into cases.json (or: judgess export --cases <path>)
```

State lives in `~/.claude/judgess/`:

| File | Content |
|---|---|
| `audit.jsonl` | One line per request: verdict, rule/reason, duration, and the raw `tool_input` and `cwd` (`Write` requests are not logged) |
| `unknown.jsonl` | Raw requests that were `Abstain(Unknown)` |
| `labelled.jsonl` | Raw requests you labelled with `classify` (local only, never commit) |
| `redaction.key` | Secret keying the redaction hashes |

Useful audit queries:

```
jq -c 'select(.verdict=="deny")' ~/.claude/judgess/audit.jsonl
jq -r '.rule // .reason' ~/.claude/judgess/audit.jsonl | sort | uniq -c | sort -rn
```

## Growing the rules

1. Use Devora; unknown requests pile up in `unknown.jsonl`
2. `classify` them (approve / deny / defer / abstain / delete / skip)
3. Ask an agent to implement the rules
4. `export` to turn them into redacted regression cases in `cases.json`

See [ADR-005](../docs/adrs/ADR-005-judgess.md) for more details.

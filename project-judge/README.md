# Judge

I really dislike having to manually go through so many permission prompts.
This projects extends the ability to allow/denies based on more complex rules.

It runs after the permissions block in your settings, so allowing here and denying there will still deny.

git -C <some long path> <command I already allowed> ...

## Setup

- Install `uv`
- Run `chmod +x ./main.py`
- Add to `~/.claude/settings.json`:

```json
    "PermissionRequest": [
      {
        "matcher": "^(?!ExitPlanMode$).*",
        "hooks": [
          {
            "type": "command",
            "command": "~/path/to/main.py"
          }
        ]
      }
    ],
```

The `^(?!ExitPlanMode$).*` matcher intercepts all permission requests except `ExitPlanMode`, which is passed through to support crit's plan-exit integration.

## Flow

The script decides whether to allow or deny a permission request based on the command and its arguments.
When it encounters a situation that it doesn't know how to handle, it saves it as an example case and defer the decision to the user, allowing them to review and decide on it later.

## Handled tool types

Judge reasons about `Bash`, `Read`, and `WebFetch` permission requests. `Bash` commands are matched against a set of known-safe and known-disallowed patterns. `Read` requests auto-allow access to crit plan and review files (`~/.crit/plans/`, `~/.crit/reviews/`). `WebFetch` requests block `file://` URLs (directing to use the `Read` tool instead) and defer `http(s)://` URLs to the user.

`ExitPlanMode` permission requests are excluded at the matcher level to support crit's plan-exit integration. `AskUserQuestion` and `Edit` permission requests are silently abstained (Judge exits without a decision, deferring to Claude Code's normal permission flow).

When Judge sees a request for any other tool type, it defers to the user and appends the raw request to `~/.claude/cc-judge-unhandled-requests.json`. This file is the source for adding support for new tool types.

## Audit log

Judge writes a JSONL audit log of every invocation to `~/.claude/cc-judge-audit.jsonl`. Every allow, deny, defer, and error is recorded.

Each line is a JSON object with these fields:

| Field | Description |
|---|---|
| `ts` | ISO 8601 timestamp (UTC) |
| `pid` | Process ID |
| `session_id` | Claude Code session identifier |
| `agent_id` | Agent identifier (for sub-agents) |
| `tool_name` | The tool being requested (`Bash`, `Read`, `WebFetch`, etc.) |
| `tool_input` | Full tool input (command text, file path, URL, etc.) |
| `cwd` | Session working directory |
| `decision` | `allow`, `deny`, `defer`, or `error` |
| `reason` | Structured reason key (e.g., `bash.all_commands_approved`, `bash.declined_command`) |
| `trail` | List of decision breadcrumbs showing which checks ran and what matched |
| `duration_us` | Processing time in microseconds |
| `details` | (optional) Additional context like the specific command or detector that matched |

Uses `O_APPEND` for atomic writes, so concurrent Judge invocations are safe.

Audit writes are wrapped in try/except — logging failures never alter the permission decision. If Judge itself crashes (malformed input, uncaught exception), the error is logged with `decision=error` and a full traceback in the `details` field.

Audit writes are suppressed when running with `--expected` (test mode).

### Querying

```sh
# Recent entries
tail -20 ~/.claude/cc-judge-audit.jsonl

# All denials
grep '"deny"' ~/.claude/cc-judge-audit.jsonl

# Specific reason
jq 'select(.reason=="bash.no_approved_detector")' < ~/.claude/cc-judge-audit.jsonl
```

### Stats

`audit-stats.py` (or `mise stats`) prints summary statistics from the log. Accepts `--since`, `--decision`, `--tool`, `--verbose`, and `--log-file` flags.

## Testing

An end-to-end test is done by running the script with known inputs and matching that to an expected result.
The test cases are stored in `./test-cases.json`.

## Redaction

`redact.py` sanitizes `test-cases.json` before it enters the repo, removing PII and sensitive content.
It redacts home directory paths, path segments (hashed to `project-XXXXX`), git hashes, commit messages/PR titles, timestamps, and `description` fields.

```
./redact.py           # Preview redacted output on stdout
./redact.py --apply   # Redact test-cases.json in place
./redact.py --audit   # Verify no suspicious content remains
```

# Spike 1: Claude stream-json + control protocol

Throwaway spike for TalAmuyal/Devora#205 (part of TalAmuyal/Devora#204). Not meant to be merged; the report on #205 is the deliverable.

## Contents

- `src/main.rs`: a Rust prototype that keeps one `claude -p` process alive over stream-json and links Judgess as a library, answering an in-process `PreToolUse` hook callback.
- `harness/`: the Python harness used to explore the protocol scenario by scenario (`harness.py`, plus `interrupt_test.py` and `ask_test.py`), and `summarize.py`, which prints a fixture's permission timeline.
- `fixtures/`: every line in both directions, recorded against Claude Code 2.1.312 with Haiku. Each line is `{"t": ms, "dir": "in"|"out"|"stderr", "msg": ...}`, and local paths are replaced by `<spike>`, `<home>` and `<project>`. These are the seed for `devora-agent`'s contract tests.

| Fixture | What it shows |
| --- | --- |
| `basic` | Two turns in one process: `system/init`, `stream_event` deltas, `assistant`, `result` with `usage` (the cache read grows on turn 2) |
| `callback`, `callback-delay` | A `PermissionRequest` hook callback in manual mode: `can_use_tool` is sent first, the hook races it, and a hook decision cancels the prompt (`control_cancel_request`) |
| `callback-auto`, `plugin-auto` | In auto mode, neither the `PermissionRequest` hook nor `can_use_tool` fires: the classifier decides everything (about 0.5 s per call) |
| `plugin` | Today's Judgess plugin in manual mode, with `--include-hook-events` (`hook_started` / `hook_response`) |
| `pre`, `pre-auto`, `rust-manual`, `rust-auto` | Judgess as an in-process `PreToolUse` callback: allow skips the classifier, deny returns the redirect, and an abstention falls through to the classifier or the prompt |
| `ask-auto` | `PreToolUse` → `ask` forces `can_use_tool` even in auto mode |
| `runtime`, `runtime-resume` | `set_permission_mode`, `/compact` (`compact_boundary`), then a kill and `--resume` (the conversation survives) |
| `interrupt` | `interrupt` mid-stream gives a `result` with `subtype: error_during_execution`, `terminal_reason: aborted_streaming` |

## Running

The harness expects `plugin/` (a copy of `project-judgess/cc-plugin` with a release `judgess` binary inside), `ws/` (an empty git repo used as cwd) and `home/` (an isolated `HOME`) next to it, and `claude` on `PATH`.

```
cargo build --release
SPIKE_HOME=<isolated home> target/release/claude-protocol-spike out.jsonl auto "<prompt>"
python3 -I harness/harness.py <basic|callback|callback-auto|cut|cut-auto|pre|pre-auto|plugin|plugin-auto|runtime> out.jsonl
python3 -I harness/summarize.py out.jsonl
```

Both drivers launch Claude with a scrubbed environment (`KEEP_ENV` / `cleanenv.sh`). When the launching process is itself a Claude Code session, the child otherwise inherits that session's identity variables (for example `CLAUDE_CODE_SESSION_ID`) and reuses its session id.

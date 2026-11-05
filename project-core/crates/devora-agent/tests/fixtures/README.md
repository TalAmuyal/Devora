# Claude protocol recordings

`claude-2.1.312/*.jsonl` contains Spike 1's 14 original recordings, recovered byte-for-byte from commit `cc5f0f8d3ecfb607cfd409d9e6e49eec8b43386c`, under `project-sapphire/spikes/claude-protocol/fixtures/`. They were recorded on Linux with Claude Code 2.1.312 and Haiku 5.5. The report is linked below.

Each envelope has `t` (elapsed milliseconds), `dir` and `msg`. `out` means host to CLI, `in` means CLI to host, and `stderr` is diagnostic text. Tests replay the JSON messages offline and preserve all their fields. The directory's version describes the recording's producer. It is independent of the current CI CLI pin, 2.1.316.

| Recordings | Contract exercised |
| --- | --- |
| basic | Multiple turns, partial text, completed assistant messages and cache usage |
| callback, callback-delay | Permission questions, hook responses and cancellation |
| callback-auto, plugin-auto | Auto-mode conversation/tool messages |
| plugin | Hook lifecycle system messages |
| pre, pre-auto, rust-manual, rust-auto | PreToolUse callbacks and tool use/results |
| ask-auto | Hook-forced permission questions and their reason |
| runtime, runtime-resume | Mode changes, compact_boundary and resumed session identity |
| interrupt | Interrupted streaming and error result |

The recordings do not contain `set_model`, `mcp_message` or error control responses. Supplemental constructed contracts in `protocol_contract.rs` cover those shapes separately. MCP and control-response spelling follows the official Python Agent SDK control types. New envelope kinds such as the recordings' `rate_limit_event` deliberately remain Raw.

## Re-recording for a CLI upgrade

1. Read the exact `CLAUDE_CODE_VERSION` pin in `.github/workflows/ci.yml` and install that CLI version. Verify `claude --version` matches before capture. Use the new version in a new fixture directory so the baseline's provenance stays visible.
2. Recover the original recorder outside the checkout. Fetch `cc5f0f8d3ecfb607cfd409d9e6e49eec8b43386c` and use `git archive` to extract `project-sapphire/spikes/claude-protocol/` into a temporary directory. Its README documents the Python and Rust drivers. The Rust prototype is historical reference and may need its Judgess path dependency adapted to the current checkout.
3. Prepare the recorder's isolated HOME and empty git workspace, and its Judgess plugin/binary as the historical README describes. Keep the recorder's scrubbed environment. The Python scripts resolve `home/`, `ws/`, `plugin/` and `cleanenv.sh` beside the script, so prepare them under `harness/`. Use credentials suitable for that isolated recording environment.
4. Run `python3 -I harness.py basic <output.jsonl>` from the harness directory, then its `callback`, `callback-auto`, `pre`, `pre-auto`, `plugin`, `plugin-auto` and `runtime` scenarios. `callback-delay` uses `PROMPT_DELAY`; `ask_test.py` and `interrupt_test.py` cover ask/interrupt. The historical Rust driver records its manual/auto runs. Keep runtime and resume recordings together and verify their session IDs agree. Record new model/MCP/error cases when the upgraded runtime exercises those exchanges.
5. Redact the temporary workspace, home and project paths to `<spike>`, `<home>` and `<project>`. Inspect every envelope, including stderr, initialization capabilities and tool output, for credentials or personal data before committing. Preserve the wire shape, IDs and ordering needed to correlate requests, tools and sessions.
6. Run `mise exec -C project-core -- cargo test -p devora-agent --test protocol_contract`. Add the new version directory to the corpus test explicitly and review typed/Raw coverage. Investigate changed shapes against the CLI/control contract before changing types. Update this provenance note and CI's pin together when the upgrade is accepted.

Re-recording uses an authenticated CLI and model calls. Normal unit and contract tests consume only committed fixtures and make no network requests. The minimum supported version is defined once in `devora_agent::version`, with a public check reusable by a future Rust health command and process startup.

[Spike 1 report](https://github.com/TalAmuyal/Devora/issues/205#issuecomment-6478770773)

[Official control types](https://github.com/anthropics/claude-agent-sdk-python/blob/main/src/claude_agent_sdk/types.py)

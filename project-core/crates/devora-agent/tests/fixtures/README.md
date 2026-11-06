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

## Session process and pinned CLI tests

`tests/session_process.rs` drives the real Rust process API against `support/fake_cli.py`, a deterministic stdio peer using these recordings. It requires Python 3 and uses protocol events as barriers. It covers requests, permissions, cancellation, Judgess, crash recovery, clean/forced pause and failure paths without model access.

`tests/session_cli.rs` is ignored by ordinary `cargo test`. Run `mise run -C project-sapphire test-agent-cli` to provision the exact CI-pinned CLI in Sapphire’s versioned build cache and run it against the existing Ember FakeClaudeServer. The test task supplies its absolute path, independently of the user’s Claude installation. This stage is also part of Sapphire's standard test command, so the existing Sapphire CI job exercises it on core changes and CLI-pin changes.

The backend projects recorded thinking/text events into safe text-only responses, removing recorded tool blocks and marking the reply complete. A separate synthetic Bash request verifies the real PreToolUse callback, its denial and user ask-rule precedence over an approval. Synthetic prompt-too-long responses compare automatic compaction with and without Devora's disabling switch. The enabled control must emit an automatic compact boundary, and the disabled session must expose the error to its owner. A controlled command hook returns the Defer mapper’s ask payload, proving that auto mode prompts and a host denial preserves the target file. Current Judgess rules emit no Defer verdict, so the mapper itself is covered separately by a synthetic-verdict unit test. These constructed responses are distinct from the byte-exact recordings above.

The contracts cover all inline instruction forms, model/effort values, normal user/project settings and user memory, configured tool-environment priority, transport-only overrides, API-key/OAuth transport, native mode/model controls and transcript resume, including an unused session reopened with its saved ID and conversation flag. Authentication observations compare only known dummy values. All model requests go to loopback.

The first-turn forced-stop contract holds the CLI at a native permission question before any result and verifies an existing transcript rejects --session-id with its old UUID. A separate text-only partial response is sent without inter-event pacing and held before completion. Both product cases force a pause deadline and reopen successfully on a fresh UUID, leaving the unfinished first turn out of the new CLI conversation. has_conversation becomes true only after a result or successful resume. This avoids making the private transcript directory layout a production dependency. The test reads its isolated transcript directory only to establish the counterfactual case.

The pinned pause/resume contract also checks transcript projection. It counts text and thinking blocks after folding the native stream and verifies that final assistant messages replace their streamed items. It checks that resuming retains the prior conversation in the backend request while emitting no earlier assistant/message-start IDs or prior user prompt. The transcript implementation therefore adds no replay deduplication.

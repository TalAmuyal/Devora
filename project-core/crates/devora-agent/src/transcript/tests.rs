use super::*;
use crate::protocol::Message;
use crate::session::SessionEvent;
use serde_json::{json, Value};

fn message(t: &mut Transcript, value: Value) -> Vec<TranscriptChange> {
    t.event(&SessionEvent::Message(
        Message::parse(&value.to_string()).unwrap(),
    ))
}
fn stream(t: &mut Transcript, value: Value) -> Vec<TranscriptChange> {
    message(t, json!({"type":"stream_event","event":value}))
}
fn start(t: &mut Transcript, id: &str) {
    stream(
        t,
        json!({"type":"message_start","message":{"id":id,"role":"assistant","content":[]}}),
    );
}

#[test]
fn many_deltas_fold_into_one_stable_item_and_final_text_replaces_it() {
    let mut t = Transcript::new(vec![], "cli".into());
    start(&mut t, "message");
    stream(
        &mut t,
        json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}),
    );
    for _ in 0..10000 {
        let changes = stream(
            &mut t,
            json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"x"}}),
        );
        assert!(
            matches!(&changes[..], [TranscriptChange::AppendText { sequence:1, text }] if text == "x")
        );
    }
    assert_eq!(t.items().len(), 1);
    assert!(
        matches!(&t.items()[0].content, TranscriptContent::Assistant { text, .. } if text.len() == 10000)
    );
    message(
        &mut t,
        json!({"type":"assistant","message":{"id":"message","role":"assistant","content":[{"type":"text","text":"Final text"}]}}),
    );
    assert_eq!(t.items().len(), 1);
    assert_eq!(t.items()[0].sequence, 1);
    assert_eq!(t.items()[0].state, ItemState::Complete);
    assert!(
        matches!(&t.items()[0].content, TranscriptContent::Assistant { text, .. } if text == "Final text")
    );
}

#[test]
fn separate_final_blocks_keep_thinking_tool_annotations_and_results() {
    let mut t = Transcript::new(vec![], "cli".into());
    start(&mut t, "message");
    stream(
        &mut t,
        json!({"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":""}}),
    );
    stream(
        &mut t,
        json!({"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"Consider"}}),
    );
    stream(
        &mut t,
        json!({"type":"content_block_delta","index":0,"delta":{"type":"signature_delta","signature":"sig"}}),
    );
    message(
        &mut t,
        json!({"type":"assistant","message":{"id":"message","role":"assistant","content":[{"type":"thinking","thinking":"Consider","signature":"sig"}]}}),
    );
    stream(
        &mut t,
        json!({"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"tool","name":"Bash","input":{}}}),
    );
    stream(
        &mut t,
        json!({"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"command\":\"pwd\"}"}}),
    );
    t.event(&SessionEvent::ToolJudged {
        tool_use_id: Some("tool".into()),
        verdict: judgess::Verdict::Approve,
    });
    message(
        &mut t,
        json!({"type":"assistant","message":{"id":"message","role":"assistant","content":[{"type":"tool_use","id":"tool","name":"Bash","input":{"command":"pwd"}}]}}),
    );
    message(
        &mut t,
        json!({"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"tool","content":"/repo","is_error":false}]}}),
    );
    assert_eq!(t.items().len(), 2);
    assert!(
        matches!(&t.items()[0].content, TranscriptContent::Thinking { text, signature } if text == "Consider" && signature == "sig")
    );
    match &t.items()[1].content {
        TranscriptContent::Tool {
            input_json,
            verdict,
            result,
            ..
        } => {
            assert_eq!(
                serde_json::from_str::<Value>(input_json).unwrap(),
                json!({"command":"pwd"})
            );
            assert_eq!(verdict.as_ref().unwrap().kind, "approve");
            assert_eq!(result.as_ref().unwrap().text, "/repo");
        }
        other => panic!("expected a tool, got {other:?}"),
    }
}

#[test]
fn parents_are_independent_and_unknown_shapes_are_retained() {
    let mut t = Transcript::new(vec![], "cli".into());
    for parent in ["one", "two"] {
        message(
            &mut t,
            json!({"type":"stream_event","parent_tool_use_id":parent,"event":{"type":"message_start","message":{"id":parent,"role":"assistant","content":[]}}}),
        );
        message(
            &mut t,
            json!({"type":"stream_event","parent_tool_use_id":parent,"event":{"type":"content_block_start","index":0,"content_block":{"type":"text","text":parent}}}),
        );
    }
    assert_eq!(t.items()[0].parent_tool_use_id.as_deref(), Some("one"));
    assert_eq!(t.items()[1].parent_tool_use_id.as_deref(), Some("two"));
    message(&mut t, json!({"type":"future_message","value":42}));
    assert!(
        matches!(&t.items()[2].content, TranscriptContent::Unknown { json } if json.contains("future_message"))
    );
}

#[test]
fn interrupted_first_prompt_is_retained_and_marked_once_before_a_new_identity() {
    let mut t = Transcript::new(vec![], "first".into());
    let user = t.submit("Keep this prompt".into());
    let TranscriptChange::Upsert { item } = user else {
        panic!()
    };
    t.settle(item.sequence, None);
    t.interrupt(true);
    t.interrupt(true);
    assert_eq!(t.items().len(), 2);
    assert!(
        matches!(&t.items()[0].content, TranscriptContent::User { text, .. } if text == "Keep this prompt")
    );
    assert!(matches!(
        &t.items()[1].content,
        TranscriptContent::System {
            kind: SystemKind::UnfinishedFirstPrompt,
            ..
        }
    ));
    t.set_session("replacement".into());
    t.submit("New turn".into());
    assert_eq!(t.items().len(), 1);
    assert_eq!(t.items()[0].sequence, 3);
    assert_eq!(t.items()[0].cli_session, "replacement");
}

#[test]
fn idless_whole_messages_remain_distinct_and_incompatible_deltas_are_unknown() {
    let mut t = Transcript::new(vec![], "cli".into());
    message(
        &mut t,
        json!({"type":"assistant","message":{"role":"assistant","content":"One"}}),
    );
    message(
        &mut t,
        json!({"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Two"}]}}),
    );
    assert_eq!(t.items().len(), 2);
    start(&mut t, "new");
    stream(
        &mut t,
        json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}),
    );
    stream(
        &mut t,
        json!({"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"incompatible"}}),
    );
    assert!(
        matches!(&t.items()[3].content, TranscriptContent::Unknown { json } if json.contains("thinking_delta"))
    );
}

#[test]
fn initialization_and_control_metadata_stay_out_of_the_transcript() {
    let mut t = Transcript::new(vec![], "cli".into());
    let recording = include_str!("../../tests/fixtures/claude-2.1.312/basic.jsonl");
    let response = recording
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .find(|row| row["msg"]["type"] == "control_response")
        .unwrap();
    assert!(message(&mut t, response["msg"].clone()).is_empty());
    assert!(message(
        &mut t,
        json!({"type":"control_cancel_request","request_id":"request"})
    )
    .is_empty());
    assert!(t.items().is_empty());
    message(&mut t, json!({"type":"future_conversation","value":1}));
    assert!(matches!(
        &t.items()[0].content,
        TranscriptContent::Unknown { .. }
    ));
}

#[test]
fn working_set_releases_history_but_keeps_open_tools_and_inflight_outcomes() {
    let mut t = Transcript::restore(vec![], "cli".into(), 501, true, false, None);
    for _ in 0..1000 {
        let change = t.submit("prompt".into());
        let TranscriptChange::Upsert { item } = change else {
            panic!()
        };
        t.settle(item.sequence, None).unwrap();
        t.release_completed();
        assert!(t.items().is_empty());
    }
    message(
        &mut t,
        json!({"type":"assistant","message":{"id":"tool-message","role":"assistant","content":[{"type":"tool_use","id":"open-tool","name":"Bash","input":{}}]}}),
    );
    t.release_completed();
    assert_eq!(t.items().len(), 1);
    let tool_sequence = t.items()[0].sequence;
    message(
        &mut t,
        json!({"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"open-tool","content":"done"}]}}),
    );
    assert_eq!(t.items()[0].sequence, tool_sequence);
    t.release_completed();
    assert!(t.items().is_empty());
    let change = t.submit("in flight".into());
    let TranscriptChange::Upsert { item } = change else {
        panic!()
    };
    t.interrupt(true);
    t.release_completed();
    assert_eq!(t.items().len(), 1);
    assert!(t
        .settle(item.sequence, Some("interrupted".into()))
        .is_some());
    t.release_completed();
    assert!(t.items().is_empty());
    assert!(
        t.interrupt(true).is_empty(),
        "first-prompt marker must survive working-set eviction"
    );
}

#[test]
fn releasing_after_each_recorded_frame_preserves_the_projected_transcript() {
    let mut baseline = Transcript::new(vec![], "cli".into());
    let mut working = Transcript::new(vec![], "cli".into());
    let mut projected = std::collections::BTreeMap::<u64, TranscriptItem>::new();
    for line in include_str!("../../tests/fixtures/claude-2.1.312/basic.jsonl").lines() {
        let row: Value = serde_json::from_str(line).unwrap();
        if row["dir"] != "in" {
            continue;
        }
        message(&mut baseline, row["msg"].clone());
        for change in message(&mut working, row["msg"].clone()) {
            match &change {
                TranscriptChange::Upsert { item } => {
                    projected.insert(item.sequence, item.as_ref().clone());
                }
                _ => {
                    for item in projected.values_mut() {
                        item.apply(&change);
                    }
                }
            }
        }
        working.release_completed();
        assert!(working.items().len() < 10);
    }
    assert_eq!(
        projected.into_values().collect::<Vec<_>>(),
        baseline.items()
    );
    assert!(working.items().is_empty());
}

fn reply(t: &mut Transcript, id: &str, model: &str) -> Vec<TranscriptChange> {
    message(
        t,
        json!({"type":"assistant","parent_tool_use_id":null,"message":{"id":id,"role":"assistant","model":model,"content":[{"type":"text","text":id}]}}),
    )
}
fn subagent_reply(t: &mut Transcript, id: &str, model: &str) -> Vec<TranscriptChange> {
    message(
        t,
        json!({"type":"assistant","parent_tool_use_id":"toolu_sub","message":{"id":id,"role":"assistant","model":model,"content":[{"type":"text","text":id}]}}),
    )
}
/// What Claude Code 2.1.326 sent over two turns while the model it was set to (Opus) was overloaded and it fell back
/// to Sonnet: per turn, two `api_retry`, the `model_fallback`, Sonnet's reply and the result (recorded, see the
/// fixtures' README).
fn overloaded_turns() -> Vec<Value> {
    include_str!("../../tests/fixtures/claude-2.1.326/model-fallback.jsonl")
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap()["msg"].clone())
        .collect()
}
const FALLBACK_SENTENCE: &str = "Switched to Sonnet 5.5 due to high demand for Opus 5.5";
fn turn_end(t: &mut Transcript) {
    message(
        t,
        json!({"type":"result","subtype":"success","session_id":"s","is_error":false,"num_turns":1,"usage":{"input_tokens":1,"output_tokens":1}}),
    );
}
/// The model-change rows among the fold's items: (text, detail).
fn model_rows(t: &Transcript) -> Vec<(String, Value)> {
    t.items()
        .iter()
        .filter_map(|item| match &item.content {
            TranscriptContent::System {
                kind: SystemKind::ModelChanged,
                text,
                detail_json,
            } => Some((text.clone(), serde_json::from_str(detail_json).unwrap())),
            _ => None,
        })
        .collect()
}

#[test]
fn replies_keep_their_model_and_the_same_model_adds_no_row() {
    let mut t = Transcript::new(vec![], "cli".into());
    reply(&mut t, "one", "claude-opus-5-5");
    reply(&mut t, "two", "claude-opus-5-5");
    assert!(model_rows(&t).is_empty());
    assert!(matches!(&t.items()[0].content,
        TranscriptContent::Assistant { model: Some(model), .. } if model == "claude-opus-5-5"));
    assert_eq!(t.latest_model(), Some("claude-opus-5-5"));
}

#[test]
fn a_streamed_reply_names_its_model_at_its_start_before_its_text() {
    let mut t = Transcript::new(vec![], "cli".into());
    reply(&mut t, "one", "claude-opus-5-5");
    stream(
        &mut t,
        json!({"type":"message_start","message":{"id":"two","role":"assistant","model":"claude-sonnet-5-5","content":[]}}),
    );
    stream(
        &mut t,
        json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}),
    );
    let kinds: Vec<&TranscriptContent> = t.items().iter().map(|item| &item.content).collect();
    assert!(matches!(
        kinds[1],
        TranscriptContent::System {
            kind: SystemKind::ModelChanged,
            ..
        }
    ));
    assert!(matches!(kinds[2],
        TranscriptContent::Assistant { model: Some(model), .. } if model == "claude-sonnet-5-5"));
}

#[test]
fn a_long_fallback_adds_one_row_in_the_clis_words_and_recovery_one_more() {
    let mut t = Transcript::new(vec![], "cli".into());
    reply(&mut t, "before", "claude-opus-5-5");
    turn_end(&mut t);
    // Opus stays overloaded for four turns: each time the CLI retries, falls back and says so.
    for _ in 0..2 {
        for sent in overloaded_turns() {
            let kind = (sent["type"].clone(), sent["subtype"].clone());
            let changes = message(&mut t, sent);
            if kind.0 == "system" {
                assert!(
                    changes.is_empty(),
                    "a retry or a fallback adds no row by itself: {kind:?}"
                );
            }
        }
        assert_eq!(t.fallback_note(), Some(FALLBACK_SENTENCE));
    }
    reply(&mut t, "after", "claude-opus-5-5");
    let rows = model_rows(&t);
    assert_eq!(rows.len(), 2, "{rows:?}");
    assert_eq!(rows[0].0, FALLBACK_SENTENCE);
    assert_eq!(
        rows[0].1,
        json!({"from":"claude-opus-5-5","to":"claude-sonnet-5-5","fallback":true})
    );
    assert_eq!(
        rows[1].0,
        "Model changed: claude-sonnet-5-5 → claude-opus-5-5"
    );
    assert_eq!(
        rows[1].1,
        json!({"from":"claude-sonnet-5-5","to":"claude-opus-5-5","fallback":false})
    );
    assert_eq!(t.fallback_note(), None, "Opus answers again");
    assert!(t
        .items()
        .iter()
        .all(|item| !matches!(item.content, TranscriptContent::Unknown { .. })));
}

#[test]
fn a_switch_the_cli_announced_no_fallback_for_says_model_changed() {
    // opusplan leaving plan mode, or the API answering with another model.
    let mut t = Transcript::new(vec![], "cli".into());
    reply(&mut t, "planning", "claude-opus-5-5");
    reply(&mut t, "building", "claude-sonnet-5-5");
    assert_eq!(
        model_rows(&t)[0].0,
        "Model changed: claude-opus-5-5 → claude-sonnet-5-5"
    );
}

#[test]
fn a_subagent_on_another_model_adds_no_row_and_leaves_the_latest_model() {
    let mut t = Transcript::new(vec![], "cli".into());
    reply(&mut t, "main", "claude-sonnet-5-5");
    subagent_reply(&mut t, "sub", "claude-haiku-5-5");
    reply(&mut t, "main again", "claude-sonnet-5-5");
    assert!(model_rows(&t).is_empty());
    assert_eq!(t.latest_model(), Some("claude-sonnet-5-5"));
}

#[test]
fn a_fallback_waits_only_for_its_own_turn() {
    // Interrupted before the fallback's reply: the next change is worded from the replies.
    let mut t = Transcript::new(vec![], "cli".into());
    reply(&mut t, "before", "claude-opus-5-5");
    message(
        &mut t,
        overloaded_turns()
            .into_iter()
            .find(|sent| sent["subtype"] == "model_fallback")
            .unwrap(),
    );
    t.interrupt(false);
    reply(&mut t, "later", "claude-sonnet-5-5");
    assert_eq!(
        model_rows(&t)[0].0,
        "Model changed: claude-opus-5-5 → claude-sonnet-5-5"
    );

    // The session ending drops it too.
    let mut t = Transcript::new(vec![], "cli".into());
    reply(&mut t, "before", "claude-opus-5-5");
    message(
        &mut t,
        overloaded_turns()
            .into_iter()
            .find(|sent| sent["subtype"] == "model_fallback")
            .unwrap(),
    );
    t.event(&SessionEvent::Exited {
        code: Some(1),
        restarting: true,
    });
    reply(&mut t, "later", "claude-sonnet-5-5");
    assert_eq!(
        model_rows(&t)[0].0,
        "Model changed: claude-opus-5-5 → claude-sonnet-5-5"
    );
}

#[test]
fn a_reopened_transcript_continues_from_its_latest_model() {
    let mut first = Transcript::new(vec![], "cli".into());
    reply(&mut first, "stored", "claude-opus-5-5");
    let mut t = Transcript::restore(
        vec![],
        "cli".into(),
        10,
        true,
        false,
        first.latest_model().map(str::to_owned),
    );
    reply(&mut t, "same", "claude-opus-5-5");
    assert!(model_rows(&t).is_empty());
    reply(&mut t, "other", "claude-sonnet-5-5");
    assert_eq!(model_rows(&t).len(), 1);
}

#[test]
fn a_reply_claude_code_made_up_itself_changes_no_model() {
    let mut t = Transcript::new(vec![], "cli".into());
    reply(&mut t, "real", "claude-opus-5-5");
    reply(&mut t, "API Error: overloaded", "<synthetic>");
    assert!(model_rows(&t).is_empty());
    assert_eq!(t.latest_model(), Some("claude-opus-5-5"));
}

#[test]
fn a_first_reply_from_a_fallback_adds_no_row_but_keeps_its_sentence() {
    let mut t = Transcript::new(vec![], "cli".into());
    for sent in overloaded_turns().into_iter().take(5) {
        message(&mut t, sent);
    }
    assert!(model_rows(&t).is_empty());
    assert_eq!(t.latest_model(), Some("claude-sonnet-5-5"));
    assert_eq!(t.fallback_note(), Some(FALLBACK_SENTENCE));
}

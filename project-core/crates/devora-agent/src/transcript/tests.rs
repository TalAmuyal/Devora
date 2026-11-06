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
        matches!(&t.items()[0].content, TranscriptContent::Assistant { text } if text.len() == 10000)
    );
    message(
        &mut t,
        json!({"type":"assistant","message":{"id":"message","role":"assistant","content":[{"type":"text","text":"Final text"}]}}),
    );
    assert_eq!(t.items().len(), 1);
    assert_eq!(t.items()[0].sequence, 1);
    assert_eq!(t.items()[0].state, ItemState::Complete);
    assert!(
        matches!(&t.items()[0].content, TranscriptContent::Assistant { text } if text == "Final text")
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
    let mut t = Transcript::restore(vec![], "cli".into(), 501, true, false);
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

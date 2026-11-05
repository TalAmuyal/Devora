use devora_agent::protocol::*;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::path::Path;

fn decode(value: Value) -> Message {
    serde_json::from_value(value).unwrap()
}

#[test]
fn every_recorded_envelope_roundtrips_and_known_shapes_are_typed() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/claude-2.1.312");
    let mut files: Vec<_> = std::fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    files.sort();
    let mut counts = BTreeMap::<String, usize>::new();
    let mut directions = BTreeMap::<String, usize>::new();
    let mut envelopes = 0;
    for file in &files {
        for (index, line) in std::fs::read_to_string(file).unwrap().lines().enumerate() {
            let recording: Value = serde_json::from_str(line).unwrap();
            let direction = recording["dir"].as_str().unwrap();
            if !matches!(direction, "in" | "out") {
                continue;
            }
            let value = recording["msg"].clone();
            let message: Message = serde_json::from_value(value.clone()).unwrap();
            assert_eq!(
                serde_json::to_value(&message).unwrap(),
                value,
                "{file:?}:{}",
                index + 1
            );
            let kind = value["type"].as_str().unwrap();
            *counts.entry(kind.to_owned()).or_default() += 1;
            *directions.entry(direction.to_owned()).or_default() += 1;
            envelopes += 1;
            match &message {
                Message::Raw(_) => assert_eq!(
                    kind,
                    "rate_limit_event",
                    "unexpected Raw at {file:?}:{}",
                    index + 1
                ),
                Message::Known(KnownMessage::ControlRequest { request, .. }) => {
                    assert!(matches!(request, Extensible::Known(_)));
                }
                Message::Known(KnownMessage::ControlResponse { response, .. }) => {
                    let Extensible::Known(ControlResponse::Success { response, .. }) = response
                    else {
                        panic!("unexpected control response {response:?}");
                    };
                    if response.get("commands").is_some() {
                        let initialized: InitializeResponse =
                            serde_json::from_value(response.clone()).unwrap();
                        assert!(!initialized.commands.is_empty());
                        assert!(!initialized.models.is_empty());
                    }
                }
                Message::Known(KnownMessage::StreamEvent { event, .. }) => {
                    assert!(matches!(event, Extensible::Known(_)));
                    if let Extensible::Known(StreamEvent::ContentBlockDelta { delta, .. }) = event {
                        assert!(matches!(delta, Extensible::Known(_)));
                    }
                }
                Message::Known(
                    KnownMessage::User { message, .. } | KnownMessage::Assistant { message, .. },
                ) => {
                    if let Content::Blocks(blocks) = &message.content {
                        assert!(blocks
                            .iter()
                            .all(|block| matches!(block, Extensible::Known(_))));
                    }
                }
                _ => {}
            }
        }
    }
    assert_eq!(files.len(), 14);
    assert!(envelopes > 1200);
    assert_eq!(directions.len(), 2);
    for kind in [
        "user",
        "assistant",
        "system",
        "result",
        "stream_event",
        "control_request",
        "control_response",
        "control_cancel_request",
    ] {
        assert!(counts[kind] > 0, "missing recorded {kind}");
    }
}

#[test]
fn contract_fields_describe_tools_permissions_compaction_and_interrupts() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/claude-2.1.312");
    let mut observed = std::collections::BTreeSet::new();
    for entry in std::fs::read_dir(root).unwrap() {
        for line in std::fs::read_to_string(entry.unwrap().path())
            .unwrap()
            .lines()
        {
            let row: Value = serde_json::from_str(line).unwrap();
            match decode(row["msg"].clone()) {
                Message::Known(KnownMessage::ControlRequest {
                    request: Extensible::Known(ControlRequest::CanUseTool(question)),
                    ..
                }) => {
                    assert!(!question.tool_use_id.is_empty());
                    assert_eq!(question.tool_name, "Bash");
                    assert!(question.input["command"].is_string());
                    if question.decision_reason().is_some() {
                        observed.insert("reason");
                    }
                    if question.permission_suggestions().is_some() {
                        observed.insert("suggestions");
                    }
                }
                Message::Known(KnownMessage::System(System::CompactBoundary {
                    compact_metadata,
                    ..
                })) => {
                    assert_eq!(compact_metadata.trigger, "manual");
                    assert!(compact_metadata.pre_tokens > 0);
                    assert!(
                        compact_metadata.extra["post_tokens"].as_u64().unwrap()
                            < compact_metadata.pre_tokens
                    );
                    observed.insert("compact");
                }
                Message::Known(KnownMessage::Result(result)) => {
                    if result.extra.get("terminal_reason").and_then(Value::as_str)
                        == Some("aborted_streaming")
                    {
                        assert_eq!(result.subtype, "error_during_execution");
                        observed.insert("interrupt");
                    }
                    if result
                        .usage
                        .extra
                        .get("cache_read_input_tokens")
                        .and_then(Value::as_u64)
                        .unwrap_or_default()
                        > 0
                    {
                        observed.insert("cache");
                    }
                }
                Message::Known(KnownMessage::Assistant {
                    message:
                        Turn {
                            content: Content::Blocks(blocks),
                            ..
                        },
                    ..
                }) => {
                    for block in blocks {
                        if let Extensible::Known(ContentBlock::ToolUse {
                            id, name, input, ..
                        }) = block
                        {
                            assert!(id.starts_with("toolu_"));
                            assert_eq!(name, "Bash");
                            assert!(input["command"].is_string());
                            observed.insert("tool_use");
                        }
                    }
                }
                Message::Known(KnownMessage::User {
                    message:
                        Turn {
                            content: Content::Blocks(blocks),
                            ..
                        },
                    ..
                }) => {
                    for block in blocks {
                        if let Extensible::Known(ContentBlock::ToolResult { tool_use_id, .. }) =
                            block
                        {
                            assert!(tool_use_id.starts_with("toolu_"));
                            observed.insert("tool_result");
                        }
                    }
                }
                _ => {}
            }
        }
    }
    assert_eq!(
        observed,
        [
            "cache",
            "compact",
            "interrupt",
            "reason",
            "suggestions",
            "tool_result",
            "tool_use"
        ]
        .into_iter()
        .collect()
    );
}

#[test]
fn host_messages_and_unrecorded_control_variants_match_the_wire_contract() {
    let requests = [
        (
            ControlRequest::Initialize {
                hooks: serde_json::from_value(
                    json!({"PreToolUse":[{"matcher":"Bash", "hookCallbackIds":["judgess"]}]}),
                )
                .unwrap(),
                extra: Map::new(),
            },
            json!({"subtype":"initialize","hooks":{"PreToolUse":[{"matcher":"Bash","hookCallbackIds":["judgess"]}]}}),
        ),
        (
            ControlRequest::Interrupt { extra: Map::new() },
            json!({"subtype":"interrupt"}),
        ),
        (
            ControlRequest::SetPermissionMode {
                mode: "auto".into(),
                extra: Map::new(),
            },
            json!({"subtype":"set_permission_mode","mode":"auto"}),
        ),
        (
            ControlRequest::SetModel {
                model: Nullable::Value("haiku".into()),
                extra: Map::new(),
            },
            json!({"subtype":"set_model","model":"haiku"}),
        ),
        (
            ControlRequest::McpMessage {
                server_name: "devora".into(),
                message: json!({"jsonrpc":"2.0","method":"tools/list","id":7}),
                extra: Map::new(),
            },
            json!({"subtype":"mcp_message","server_name":"devora","message":{"jsonrpc":"2.0","method":"tools/list","id":7}}),
        ),
    ];
    for (request, expected) in requests {
        let message = Message::request("r1".into(), request);
        assert_eq!(
            serde_json::to_value(&message).unwrap(),
            json!({"type":"control_request","request_id":"r1","request":expected})
        );
        assert_eq!(
            Message::parse(&serde_json::to_string(&message).unwrap()).unwrap(),
            message
        );
    }
    assert_eq!(
        serde_json::to_value(Message::user_text("session".into(), "hello".into())).unwrap(),
        json!({"type":"user","message":{"role":"user","content":"hello"},"session_id":"session","parent_tool_use_id":null})
    );
    let answer = PermissionResponse::Deny {
        message: "Use a safe command".into(),
        extra: Map::new(),
    };
    let response = Message::success("r1".into(), serde_json::to_value(answer).unwrap());
    assert_eq!(
        serde_json::to_value(response).unwrap(),
        json!({"type":"control_response","response":{"subtype":"success","request_id":"r1","response":{"behavior":"deny","message":"Use a safe command"}}})
    );
    let error = json!({"type":"control_response","response":{"subtype":"error","request_id":"r2","error":"unsupported operation","future":9}});
    assert!(matches!(
        decode(error.clone()),
        Message::Known(KnownMessage::ControlResponse {
            response: Extensible::Known(ControlResponse::Error { .. }),
            ..
        })
    ));
    assert_eq!(serde_json::to_value(decode(error.clone())).unwrap(), error);
}

#[test]
fn future_and_changed_shapes_pass_through_losslessly() {
    for value in [
        json!({"type":"future_message","payload":[1,null,"x"]}),
        json!({"type":"system","subtype":"future_system","value":1}),
        json!({"type":"assistant","message":{"role":"assistant","content":[{"type":"image","source":{"type":"future"}}]},"future":null}),
        json!({"type":"control_request","request_id":"r","request":{"subtype":"future_request","data":true}}),
        json!({"type":"control_response","response":{"subtype":"future_response","request_id":"r"}}),
        json!({"type":"stream_event","event":{"type":"future_event","nested":{"x":null}}}),
        json!({"type":"stream_event","event":{"type":"content_block_delta","index":1,"delta":{"type":"future_delta","payload":7}}}),
        json!({"type":"control_request","request_id":"r","request":{"subtype":"set_model","model":{"future":"shape"}}}),
        json!([1, "unknown representation", null]),
    ] {
        assert_eq!(serde_json::to_value(decode(value.clone())).unwrap(), value);
    }
    assert!(matches!(decode(json!({"type":"new"})), Message::Raw(_)));
    let Message::Known(KnownMessage::ControlRequest { request, .. }) =
        decode(json!({"type":"control_request","request_id":"r","request":{"subtype":"new"}}))
    else {
        panic!("expected known envelope")
    };
    assert!(matches!(request, Extensible::Raw(_)));
    assert!(Message::parse("not JSON").is_err());
}

#[test]
fn nullable_fields_distinguish_null_from_missing_and_support_resetting_the_model() {
    let reset = Message::request(
        "reset".into(),
        ControlRequest::SetModel {
            model: Nullable::Null(()),
            extra: Map::new(),
        },
    );
    let value = serde_json::to_value(&reset).unwrap();
    assert_eq!(value["request"]["model"], Value::Null);
    assert_eq!(decode(value), reset);
    let missing =
        json!({"type":"control_request","request_id":"r","request":{"subtype":"set_model"}});
    let Message::Known(KnownMessage::ControlRequest {
        request: Extensible::Raw(_),
        ..
    }) = decode(missing.clone())
    else {
        panic!("missing required nullable field must preserve the request as Raw");
    };
    assert_eq!(
        serde_json::to_value(decode(missing.clone())).unwrap(),
        missing
    );
}

#[test]
fn permission_and_hook_answers_use_their_distinct_response_payloads() {
    let allow = PermissionResponse::Allow {
        updated_input: json!({"command":"git status"}),
        extra: Map::from_iter([("updatedPermissions".into(), json!([]))]),
    };
    let response = Message::success("permission".into(), serde_json::to_value(allow).unwrap());
    assert_eq!(
        serde_json::to_value(response).unwrap()["response"]["response"],
        json!({
            "behavior":"allow", "updatedInput":{"command":"git status"}, "updatedPermissions":[]
        })
    );
    let callback = json!({"hookSpecificOutput":{
        "hookEventName":"PreToolUse", "permissionDecision":"ask", "permissionDecisionReason":"Review required"
    }});
    let response = Message::success("callback".into(), callback.clone());
    assert_eq!(
        serde_json::to_value(response).unwrap()["response"]["response"],
        callback
    );
    assert_eq!(
        serde_json::to_value(Message::success("abstention".into(), json!({}))).unwrap()["response"]
            ["response"],
        json!({})
    );
}

#[test]
fn streaming_usage_can_report_only_output_tokens() {
    let value = json!({"type":"stream_event","event":{"type":"message_delta",
        "delta":{"stop_reason":"tool_use"},"usage":{"output_tokens":7}}});
    let message = decode(value.clone());
    assert!(matches!(&message,Message::Known(KnownMessage::StreamEvent {
        event:Extensible::Known(StreamEvent::MessageDelta { usage,.. }),..
    }) if usage.output_tokens==7));
    assert_eq!(serde_json::to_value(message).unwrap(), value);
}

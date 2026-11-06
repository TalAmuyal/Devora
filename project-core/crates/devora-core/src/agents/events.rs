use devora_agent::session::SessionEvent;
use devora_state::{ChatCommand, ChatItem, ChatUpdate, PermissionQuestion, Store};
use serde_json::{json, Value};

pub(super) fn update(state: &Store, tab: u64, generation: u64, update: ChatUpdate) {
    // Closed tabs and older generations deliberately discard late frames.
    let _ = state.dispatch_chat(ChatCommand::Update {
        tab,
        generation,
        update,
    });
}

pub(super) fn item(state: &Store, tab: u64, generation: u64, kind: &str, value: Value) {
    update(
        state,
        tab,
        generation,
        ChatUpdate::Item {
            item: ChatItem {
                kind: kind.into(),
                payload_json: value.to_string(),
            },
        },
    );
}

pub(super) fn publish(state: &Store, tab: u64, generation: u64, event: SessionEvent) {
    match event {
        SessionEvent::Ready {
            session_id,
            resumed,
            ..
        } => {
            update(
                state,
                tab,
                generation,
                ChatUpdate::Ready {
                    session_id: session_id.clone(),
                },
            );
            item(
                state,
                tab,
                generation,
                "ready",
                json!({"session_id":session_id,"resumed":resumed}),
            );
        }
        SessionEvent::Message(message) => {
            if matches!(
                &message,
                devora_agent::protocol::Message::Known(
                    devora_agent::protocol::KnownMessage::Result(_)
                )
            ) {
                update(state, tab, generation, ChatUpdate::Turn { running: false });
            }
            item(state, tab, generation, "message", json!(message));
        }
        SessionEvent::PermissionRequested {
            request_id,
            question,
        } => {
            item(
                state,
                tab,
                generation,
                "permission_requested",
                json!({"request_id": request_id, "question":question}),
            );
            update(
                state,
                tab,
                generation,
                ChatUpdate::PermissionRequested {
                    question: PermissionQuestion {
                        request_id,
                        payload_json: json!(question).to_string(),
                    },
                },
            );
        }
        SessionEvent::PermissionCancelled { request_id } => {
            item(
                state,
                tab,
                generation,
                "permission_cancelled",
                json!({"request_id":request_id}),
            );
            update(
                state,
                tab,
                generation,
                ChatUpdate::PermissionRemoved { request_id },
            );
        }
        SessionEvent::ToolJudged {
            tool_use_id,
            verdict,
        } => {
            item(
                state,
                tab,
                generation,
                "judgement",
                json!({"tool_use_id":tool_use_id,"verdict":verdict.kind().as_str(),"rule":verdict.rule().map(|r| r.as_str()),"detail":format!("{verdict:?}")}),
            );
        }
        SessionEvent::Usage { usage, .. } => update(
            state,
            tab,
            generation,
            ChatUpdate::Usage {
                payload_json: usage.to_string(),
            },
        ),
        SessionEvent::Compacted(metadata) => {
            item(state, tab, generation, "compaction", json!(metadata))
        }
        SessionEvent::Warning(message) => item(
            state,
            tab,
            generation,
            "warning",
            json!({"message":message}),
        ),
        SessionEvent::Exited { code, restarting } => {
            update(
                state,
                tab,
                generation,
                ChatUpdate::Phase {
                    phase: devora_state::ChatPhase::Opening,
                },
            );
            item(
                state,
                tab,
                generation,
                "exit",
                json!({"code":code,"restarting":restarting}),
            );
        }
        SessionEvent::Stopped(info) => item(
            state,
            tab,
            generation,
            "stopped",
            json!({"session_id":info.session_id,"has_conversation":info.has_conversation,"graceful":info.graceful}),
        ),
        SessionEvent::Failed(message) => item(
            state,
            tab,
            generation,
            "failure",
            json!({"message":message}),
        ),
        SessionEvent::McpRequested {
            request_id,
            server_name,
            message,
        } => {
            item(
                state,
                tab,
                generation,
                "mcp_request",
                json!({"request_id":request_id,"server_name":server_name,"message":message}),
            );
        }
    }
}

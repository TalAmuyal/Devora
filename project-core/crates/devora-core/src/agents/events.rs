use devora_agent::session::SessionEvent;
use devora_agent_space::SpaceEvent;
use devora_state::{ChatCommand, ChatUpdate, PermissionQuestion, Store};

pub(super) fn update(state: &Store, tab: u64, generation: u64, update: ChatUpdate) {
    // Closed tabs and older generations deliberately discard late updates.
    let _ = state.dispatch_chat(ChatCommand::Update {
        tab,
        generation,
        update,
    });
}

pub(super) fn publish(state: &Store, tab: u64, generation: u64, event: SpaceEvent) {
    match event {
        SpaceEvent::Restored(items) => {
            update(state, tab, generation, ChatUpdate::Restore { items })
        }
        SpaceEvent::Transcript(change) => {
            update(state, tab, generation, ChatUpdate::Transcript { change })
        }
        SpaceEvent::Session(event) => session(state, tab, generation, event),
        SpaceEvent::Context(context) => update(
            state,
            tab,
            generation,
            ChatUpdate::Context {
                context: devora_state::ChatContext {
                    used_tokens: context.used_tokens,
                    window_tokens: context.window_tokens,
                    threshold_percent: context.threshold_percent,
                },
            },
        ),
        SpaceEvent::TurnStarted => {
            update(state, tab, generation, ChatUpdate::Turn { running: true })
        }
    }
}

fn session(state: &Store, tab: u64, generation: u64, event: SessionEvent) {
    match event {
        SessionEvent::Ready { session_id, .. } => {
            update(state, tab, generation, ChatUpdate::Ready { session_id })
        }
        SessionEvent::Message(devora_agent::protocol::Message::Known(
            devora_agent::protocol::KnownMessage::Result(_),
        )) => update(state, tab, generation, ChatUpdate::Turn { running: false }),
        SessionEvent::PermissionRequested {
            request_id,
            question,
        } => {
            let decision_reason = question.decision_reason().map(str::to_owned);
            let suggested_rules_json = question
                .permission_suggestions()
                .map(|rules| serde_json::json!(rules).to_string());
            let can_always_allow = question
                .permission_suggestions()
                .is_some_and(|rules| !rules.is_empty());
            update(
                state,
                tab,
                generation,
                ChatUpdate::PermissionRequested {
                    question: PermissionQuestion {
                        request_id,
                        tool_name: question.tool_name,
                        tool_use_id: question.tool_use_id,
                        input_json: question.input.to_string(),
                        decision_reason,
                        can_always_allow,
                        suggested_rules_json,
                    },
                },
            );
        }
        SessionEvent::PermissionCancelled { request_id } => update(
            state,
            tab,
            generation,
            ChatUpdate::PermissionRemoved { request_id },
        ),
        SessionEvent::Usage { usage, .. } => update(
            state,
            tab,
            generation,
            ChatUpdate::Usage {
                payload_json: usage.to_string(),
            },
        ),
        SessionEvent::Exited { .. } => update(
            state,
            tab,
            generation,
            ChatUpdate::Phase {
                phase: devora_state::ChatPhase::Opening,
            },
        ),
        // These already produced typed, committed transcript changes in the owner.
        _ => {}
    }
}

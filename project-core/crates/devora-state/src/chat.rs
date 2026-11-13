//! The Store's typed live chat projection. Token patches update existing items
//! in memory. The space actor owns durable whole items in its own database.
use crate::{StateError, TranscriptChange, TranscriptItem};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChatPhase {
    Opening,
    Running,
    Pausing,
    Paused,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PermissionQuestion {
    pub request_id: String,
    pub tool_name: String,
    pub tool_use_id: String,
    pub input_json: String,
    pub decision_reason: Option<String>,
    pub can_always_allow: bool,
    pub suggested_rules_json: Option<String>,
}

/// How full an agent's context is, for its context bar (ADR-012).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChatContext {
    pub used_tokens: u64,
    /// Zero until the model's context window is known.
    pub window_tokens: u64,
    /// Where Devora saves the agent's memory and compacts.
    pub threshold_percent: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chat {
    pub tab: u64,
    pub generation: u64,
    pub phase: ChatPhase,
    pub turn_running: bool,
    pub pause_requested: bool,
    pub globally_held: bool,
    pub keep_paused: bool,
    pub session_id: Option<String>,
    pub items: Vec<TranscriptItem>,
    pub error: Option<String>,
    pub questions: Vec<PermissionQuestion>,
    pub usage_json: Option<String>,
    pub context: Option<ChatContext>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChatUpdate {
    GlobalHold { held: bool },
    KeepPaused { keep_paused: bool },
    PauseRequested { requested: bool },
    Turn { running: bool },
    Generation { generation: u64 },
    Phase { phase: ChatPhase },
    Ready { session_id: String },
    Restore { items: Vec<TranscriptItem> },
    Transcript { change: TranscriptChange },
    Failure { message: String },
    PermissionRequested { question: PermissionQuestion },
    PermissionRemoved { request_id: String },
    Usage { payload_json: String },
    Context { context: ChatContext },
}

/// Named mutations of the transient projection, serialized by Store.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChatCommand {
    Start {
        tab: u64,
        generation: u64,
    },
    Update {
        tab: u64,
        generation: u64,
        update: ChatUpdate,
    },
    Close {
        tab: u64,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChatEvent {
    Snapshot {
        chats: Vec<Chat>,
    },
    Started {
        chat: Chat,
    },
    Updated {
        tab: u64,
        generation: u64,
        update: ChatUpdate,
    },
    Closed {
        tab: u64,
    },
}

pub(crate) fn apply(chats: &mut Vec<Chat>, command: ChatCommand) -> Result<ChatEvent, StateError> {
    match command {
        ChatCommand::Start { tab, generation } => {
            let chat = Chat {
                tab,
                generation,
                phase: ChatPhase::Opening,
                turn_running: false,
                pause_requested: false,
                globally_held: false,
                keep_paused: false,
                session_id: None,
                items: vec![],
                error: None,
                questions: vec![],
                usage_json: None,
                context: None,
            };
            chats.retain(|c| c.tab != tab);
            chats.push(chat.clone());
            Ok(ChatEvent::Started { chat })
        }
        ChatCommand::Close { tab } => {
            chats.retain(|c| c.tab != tab);
            Ok(ChatEvent::Closed { tab })
        }
        ChatCommand::Update {
            tab,
            generation,
            update,
        } => {
            let chat = chats
                .iter_mut()
                .find(|c| c.tab == tab && c.generation == generation)
                .ok_or(StateError::UnknownTab { tab })?;
            match &update {
                ChatUpdate::GlobalHold { held } => chat.globally_held = *held,
                ChatUpdate::KeepPaused { keep_paused } => chat.keep_paused = *keep_paused,
                ChatUpdate::PauseRequested { requested } => chat.pause_requested = *requested,
                ChatUpdate::Turn { running } => chat.turn_running = *running,
                ChatUpdate::Generation { generation } => {
                    chat.generation = *generation;
                    chat.questions.clear();
                }
                ChatUpdate::Phase { phase } => {
                    chat.phase = phase.clone();
                    if matches!(phase, ChatPhase::Paused | ChatPhase::Failed) {
                        chat.pause_requested = false;
                    }
                    if *phase != ChatPhase::Running {
                        chat.questions.clear();
                        chat.turn_running = false;
                    }
                }
                ChatUpdate::Ready { session_id } => {
                    chat.phase = ChatPhase::Running;
                    chat.error = None;
                    chat.turn_running = false;
                    chat.session_id = Some(session_id.clone());
                    chat.questions.clear();
                }
                ChatUpdate::Restore { items } => {
                    chat.items = items.iter().rev().take(200).cloned().collect();
                    chat.items.reverse();
                }
                ChatUpdate::Failure { message } => {
                    chat.error = Some(message.clone());
                    chat.pause_requested = false;
                    chat.phase = ChatPhase::Failed;
                    chat.turn_running = false;
                    chat.questions.clear();
                }
                ChatUpdate::Transcript { change } => {
                    if matches!(change, TranscriptChange::Upsert { item } if matches!(item.content, crate::TranscriptContent::System { kind:crate::SystemKind::Result, .. }))
                    {
                        chat.turn_running = false;
                    }
                    let sequence = match change {
                        TranscriptChange::Upsert { item } => item.sequence,
                        TranscriptChange::AppendText { sequence, .. }
                        | TranscriptChange::AppendThinking { sequence, .. }
                        | TranscriptChange::AppendSignature { sequence, .. }
                        | TranscriptChange::AppendToolInput { sequence, .. } => *sequence,
                    };
                    match chat
                        .items
                        .binary_search_by_key(&sequence, |item| item.sequence)
                    {
                        Ok(index) => {
                            chat.items[index].apply(change);
                        }
                        Err(index) => {
                            if let TranscriptChange::Upsert { item } = change {
                                chat.items.insert(index, item.as_ref().clone());
                            }
                        }
                    }
                    chat.items.drain(..chat.items.len().saturating_sub(200));
                }
                ChatUpdate::PermissionRequested { question } => {
                    chat.questions
                        .retain(|q| q.request_id != question.request_id);
                    chat.questions.push(question.clone());
                }
                ChatUpdate::PermissionRemoved { request_id } => {
                    chat.questions.retain(|q| q.request_id != *request_id);
                }
                ChatUpdate::Usage { payload_json } => chat.usage_json = Some(payload_json.clone()),
                ChatUpdate::Context { context } => chat.context = Some(*context),
            }
            Ok(ChatEvent::Updated {
                tab,
                generation,
                update,
            })
        }
    }
}

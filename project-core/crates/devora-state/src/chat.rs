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
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chat {
    pub tab: u64,
    pub generation: u64,
    pub phase: ChatPhase,
    pub turn_running: bool,
    pub session_id: Option<String>,
    pub items: Vec<TranscriptItem>,
    pub error: Option<String>,
    pub questions: Vec<PermissionQuestion>,
    pub usage_json: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChatUpdate {
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
                session_id: None,
                items: vec![],
                error: None,
                questions: vec![],
                usage_json: None,
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
                ChatUpdate::Turn { running } => chat.turn_running = *running,
                ChatUpdate::Generation { generation } => {
                    chat.generation = *generation;
                    chat.questions.clear();
                }
                ChatUpdate::Phase { phase } => {
                    chat.phase = phase.clone();
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
                ChatUpdate::Restore { items } => chat.items.clone_from(items),
                ChatUpdate::Failure { message } => {
                    chat.error = Some(message.clone());
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
            }
            Ok(ChatEvent::Updated {
                tab,
                generation,
                update,
            })
        }
    }
}

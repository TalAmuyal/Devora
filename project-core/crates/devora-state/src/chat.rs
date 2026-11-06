//! The Store's live chat projection. Its frames and token deltas live only in
//! memory. Agent Space history belongs to the space database in the next slice.
use crate::StateError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChatPhase {
    Opening,
    Running,
    Pausing,
    Paused,
    Failed,
}

/// Protocol payloads retain their forward-compatible JSON. `kind` identifies
/// message, user, judgement, compaction, warning, exit or interruption records.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatItem {
    pub kind: String,
    pub payload_json: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PermissionQuestion {
    pub request_id: String,
    pub payload_json: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chat {
    pub tab: u64,
    pub generation: u64,
    pub phase: ChatPhase,
    pub turn_running: bool,
    pub session_id: Option<String>,
    pub items: Vec<ChatItem>,
    pub questions: Vec<PermissionQuestion>,
    pub usage_json: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChatUpdate {
    Turn { running: bool },
    Generation { generation: u64 },
    Phase { phase: ChatPhase },
    Ready { session_id: String },
    Item { item: ChatItem },
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
                    chat.turn_running = false;
                    chat.session_id = Some(session_id.clone());
                    chat.questions.clear();
                }
                ChatUpdate::Item { item } => chat.items.push(item.clone()),
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

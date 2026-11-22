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
    /// Where Devora saves the agent's memory and compacts: the agent's own
    /// threshold, when it has one, else its profile's.
    pub threshold_percent: u8,
    /// The profile's threshold, which the agent follows without one of its own.
    pub default_threshold_percent: u8,
    /// The agent's own threshold, as stored, even when it equals the profile's.
    pub own_threshold_percent: Option<u8>,
    pub compaction: CompactionProgress,
}

/// The model an agent's replies come from, for its model widget.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatModel {
    /// The latest reply's model in the agent's own conversation, a full ID; none until one is known.
    pub latest: Option<String>,
    /// How to show it: its name as Claude Code lists it, else its ID as it is.
    pub label: Option<String>,
    /// What the agent is set to: an alias or a full ID.
    pub set_to: String,
    /// The full ID that setting resolves to under the profile's model tiers, when known.
    pub set_to_resolved: Option<String>,
    /// The latest reply came from a model the setting doesn't resolve to (`opusplan` expects its Opus and Sonnet).
    pub differs: bool,
    /// Claude Code's sentence for the fallback the latest model came from.
    pub fallback: Option<String>,
}

/// Where an agent's compaction is, as far as the user can act on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompactionProgress {
    Idle,
    /// Asked for; it starts when the running turn ends.
    Requested,
    /// Its turns are running.
    Underway,
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
    pub model: Option<ChatModel>,
}

/// What an agent with a live runtime is doing, for knowing when everyone has stopped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AgentActivity {
    /// A turn is running: one the user sent, or a compaction step.
    Turn,
    /// A turn waits for the user's answer to a permission question.
    Question,
    /// No turn is running; the runtime is still up.
    Idle,
    /// Its tab was closed and its runtime is stopping.
    Stopping,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunningAgent {
    pub profile_path: String,
    pub name: String,
    /// `None` once its tab was closed.
    pub tab: Option<u64>,
    pub activity: AgentActivity,
}

/// The agents whose runtime is live, out of every agent the registry still holds,
/// open or stopping; a global hold is complete once `running` is empty.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RunningAgents {
    pub total: u32,
    pub running: Vec<RunningAgent>,
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
    Model { model: ChatModel },
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
    /// Published by the agent registry whenever the live runtimes change.
    Running {
        running: RunningAgents,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChatEvent {
    Snapshot {
        chats: Vec<Chat>,
        running: RunningAgents,
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
    Running {
        running: RunningAgents,
    },
}

pub(crate) fn apply(
    chats: &mut Vec<Chat>,
    running: &mut RunningAgents,
    command: ChatCommand,
) -> Result<ChatEvent, StateError> {
    match command {
        ChatCommand::Running { running: next } => {
            *running = next.clone();
            Ok(ChatEvent::Running { running: next })
        }
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
                model: None,
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
                ChatUpdate::Model { model } => chat.model = Some(model.clone()),
            }
            Ok(ChatEvent::Updated {
                tab,
                generation,
                update,
            })
        }
    }
}

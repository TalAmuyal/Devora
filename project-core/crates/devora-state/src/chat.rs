//! The Store's typed live chat projection. Token patches update existing items
//! in memory. The space actor owns durable whole items in its own database.
use crate::{ItemState, StateError, TranscriptChange, TranscriptContent, TranscriptItem};

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
    /// The user set the agent to another model and it hasn't answered on it yet: the label names the chosen model.
    pub awaiting_change: bool,
    /// Why the running session didn't take the model the user set (it stays stored for the next launch).
    pub change_not_taken: Option<String>,
}

/// The effort the agent runs at, for its effort widget.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatEffort {
    /// The level the agent is set to; none is Claude Code's default.
    pub set_to: Option<String>,
    /// The levels its current model takes, lowest first, as Claude Code lists them; empty when it takes none, or when
    /// the list isn't known.
    pub levels: Vec<String>,
    /// Set during a turn: the CLI takes it when the turn ends.
    pub pending: bool,
    /// Why the running session didn't take the level the user set (it stays stored for the next launch).
    pub not_taken: Option<String>,
}

/// What a running turn is doing now, for the line beside the chat's spinner, and since when: the step's start, in
/// milliseconds since the Unix epoch on the core's clock, kept while the step stays the same and taken again when it
/// changes. It lives only in memory: a turn doesn't survive a relaunch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatStep {
    pub activity: StepActivity,
    pub since_ms: u64,
}

/// A turn's current step, from its questions and its latest item; two steps are the same step only for the same tool
/// call or question, so the next Bash starts its own.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StepActivity {
    /// A tool waits for the user's answer to its permission question.
    Waiting { tool: String, request_id: String },
    /// A tool call has no result yet. `subagent` when it's a subagent's, as for the other steps that follow an item.
    Running {
        tool: String,
        tool_use_id: String,
        subagent: bool,
    },
    /// A thought is streaming.
    Thinking { subagent: bool },
    /// A reply is streaming.
    Writing { subagent: bool },
    /// Anything else while the turn runs: the model is on its way to the next step.
    Working,
}

impl StepActivity {
    /// The step a chat is on, or none when no turn runs and no question waits.
    fn of(chat: &Chat) -> Option<Self> {
        if let Some(question) = chat.questions.last() {
            return Some(Self::Waiting {
                tool: question.tool_name.clone(),
                request_id: question.request_id.clone(),
            });
        }
        if !chat.turn_running {
            return None;
        }
        let Some(item) = chat.items.last() else {
            return Some(Self::Working);
        };
        let unfinished = matches!(item.state, ItemState::Pending | ItemState::Streaming);
        let subagent = !item.is_main_thread();
        Some(match &item.content {
            TranscriptContent::Tool {
                name,
                tool_use_id,
                result: None,
                ..
            } if !matches!(item.state, ItemState::Interrupted | ItemState::Failed) => {
                Self::Running {
                    tool: name.clone(),
                    tool_use_id: tool_use_id.clone(),
                    subagent,
                }
            }
            TranscriptContent::Thinking { .. } if unfinished => Self::Thinking { subagent },
            TranscriptContent::Assistant { .. } if unfinished => Self::Writing { subagent },
            _ => Self::Working,
        })
    }
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
    /// A turn is running: one the user sent (or Devora: a compaction step, the restart notice), or one the agent's CLI
    /// started itself (`agent_turn`).
    pub turn_running: bool,
    /// The running turn is one the agent's CLI started itself, as when a background task reports. The user can still
    /// send: a prompt joins that turn, which is then theirs.
    pub agent_turn: bool,
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
    pub effort: Option<ChatEffort>,
    pub step: Option<ChatStep>,
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
    GlobalHold {
        held: bool,
    },
    KeepPaused {
        keep_paused: bool,
    },
    PauseRequested {
        requested: bool,
    },
    /// A turn started or ended: one the agent's CLI started itself when `by_agent`. A start of the agent's own turn
    /// doesn't replace one already shown (a prompt sent just before it began joined it), and an end ends only a turn
    /// of its own kind, so a late end of the agent's turn never ends the user's.
    ///
    /// The known limit: a prompt sent during the agent's turn makes the turn the user's, so if that send fails, its end
    /// leaves no turn shown, and the agent's own turn's end, if it comes, is then ignored. A send fails only when the
    /// CLI can't take it (restarting, or gone), which most likely ended the agent's turn too; showing no turn beats a
    /// spinner left on.
    Turn {
        running: bool,
        by_agent: bool,
    },
    Generation {
        generation: u64,
    },
    Phase {
        phase: ChatPhase,
    },
    Ready {
        session_id: String,
    },
    Restore {
        items: Vec<TranscriptItem>,
    },
    Transcript {
        change: TranscriptChange,
    },
    Failure {
        message: String,
    },
    PermissionRequested {
        question: PermissionQuestion,
    },
    PermissionRemoved {
        request_id: String,
    },
    Usage {
        payload_json: String,
    },
    Context {
        context: ChatContext,
    },
    Effort {
        effort: ChatEffort,
    },
    Model {
        model: ChatModel,
    },
    /// The chat's step changed; the reducer publishes it after the update that changed it.
    Step {
        step: Option<ChatStep>,
    },
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
#[expect(
    clippy::large_enum_variant,
    reason = "devora-ffi mirrors this enum for UniFFI, which can't carry a boxed Chat; one is sent per opened chat"
)]
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

/// Applies `command` at `now_ms` (the core's clock, for a step's start), returning the events it publishes: an update
/// that changes the chat's step is followed by that step.
pub(crate) fn apply(
    chats: &mut Vec<Chat>,
    running: &mut RunningAgents,
    command: ChatCommand,
    now_ms: u64,
) -> Result<Vec<ChatEvent>, StateError> {
    match command {
        ChatCommand::Running { running: next } => {
            *running = next.clone();
            Ok(vec![ChatEvent::Running { running: next }])
        }
        ChatCommand::Start { tab, generation } => {
            let chat = Chat {
                tab,
                generation,
                phase: ChatPhase::Opening,
                turn_running: false,
                agent_turn: false,
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
                effort: None,
                step: None,
            };
            chats.retain(|c| c.tab != tab);
            chats.push(chat.clone());
            Ok(vec![ChatEvent::Started { chat }])
        }
        ChatCommand::Close { tab } => {
            chats.retain(|c| c.tab != tab);
            Ok(vec![ChatEvent::Closed { tab }])
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
                ChatUpdate::Turn {
                    running: true,
                    by_agent: true,
                } => {
                    if !chat.turn_running {
                        chat.turn_running = true;
                        chat.agent_turn = true;
                    }
                }
                ChatUpdate::Turn {
                    running: true,
                    by_agent: false,
                } => {
                    chat.turn_running = true;
                    chat.agent_turn = false;
                }
                ChatUpdate::Turn {
                    running: false,
                    by_agent,
                } => {
                    if chat.agent_turn == *by_agent {
                        chat.turn_running = false;
                        chat.agent_turn = false;
                    }
                }
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
                        chat.agent_turn = false;
                    }
                }
                ChatUpdate::Ready { session_id } => {
                    chat.phase = ChatPhase::Running;
                    chat.error = None;
                    chat.turn_running = false;
                    chat.agent_turn = false;
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
                    chat.agent_turn = false;
                    chat.questions.clear();
                }
                ChatUpdate::Transcript { change } => {
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
                ChatUpdate::Effort { effort } => chat.effort = Some(effort.clone()),
                ChatUpdate::Step { step } => chat.step = step.clone(),
            }
            let mut events = vec![ChatEvent::Updated {
                tab,
                generation,
                update,
            }];
            let step = StepActivity::of(chat).map(|activity| match &chat.step {
                Some(step) if step.activity == activity => step.clone(),
                _ => ChatStep {
                    activity,
                    since_ms: now_ms,
                },
            });
            if step != chat.step {
                chat.step = step.clone();
                events.push(ChatEvent::Updated {
                    tab,
                    generation: chat.generation,
                    update: ChatUpdate::Step { step },
                });
            }
            Ok(events)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SystemKind, ToolResult};

    struct Fixture {
        chats: Vec<Chat>,
        running: RunningAgents,
    }

    impl Fixture {
        fn new() -> Self {
            let mut fixture = Self {
                chats: vec![],
                running: RunningAgents::default(),
            };
            fixture.send(
                ChatCommand::Start {
                    tab: 1,
                    generation: 1,
                },
                0,
            );
            fixture
        }

        fn send(&mut self, command: ChatCommand, now_ms: u64) -> Vec<ChatEvent> {
            apply(&mut self.chats, &mut self.running, command, now_ms).unwrap()
        }

        /// Applies `update` at `now_ms`, returning the step it published, if it changed it.
        fn update(&mut self, update: ChatUpdate, now_ms: u64) -> Option<Option<ChatStep>> {
            let events = self.send(
                ChatCommand::Update {
                    tab: 1,
                    generation: 1,
                    update,
                },
                now_ms,
            );
            match events.as_slice() {
                [_] => None,
                [_, ChatEvent::Updated {
                    update: ChatUpdate::Step { step },
                    ..
                }] => Some(step.clone()),
                other => panic!("unexpected events {other:?}"),
            }
        }

        fn item(
            &mut self,
            sequence: u64,
            state: ItemState,
            content: TranscriptContent,
            now_ms: u64,
        ) -> Option<Option<ChatStep>> {
            self.item_of(None, sequence, state, content, now_ms)
        }

        /// An item of the subagent that tool call `parent` started, or of the agent itself with none.
        fn item_of(
            &mut self,
            parent: Option<&str>,
            sequence: u64,
            state: ItemState,
            content: TranscriptContent,
            now_ms: u64,
        ) -> Option<Option<ChatStep>> {
            let item = TranscriptItem {
                sequence,
                cli_session: "session".into(),
                parent_tool_use_id: parent.map(Into::into),
                state,
                content,
                created_at_ms: None,
            };
            self.update(
                ChatUpdate::Transcript {
                    change: TranscriptChange::upsert(item),
                },
                now_ms,
            )
        }

        fn step(&self) -> Option<ChatStep> {
            self.chats[0].step.clone()
        }
    }

    fn bash(id: &str, result: Option<&str>) -> TranscriptContent {
        TranscriptContent::Tool {
            tool_use_id: id.into(),
            name: "Bash".into(),
            input_json: "{}".into(),
            verdict: None,
            result: result.map(|text| ToolResult {
                text: text.into(),
                content_json: "[]".into(),
                is_error: false,
            }),
        }
    }

    fn step(activity: StepActivity, since_ms: u64) -> Option<ChatStep> {
        Some(ChatStep { activity, since_ms })
    }

    fn running(id: &str) -> StepActivity {
        StepActivity::Running {
            tool: "Bash".into(),
            tool_use_id: id.into(),
            subagent: false,
        }
    }

    #[test]
    fn a_turns_step_follows_its_latest_item_and_keeps_its_start_while_it_stays_the_same() {
        let mut chat = Fixture::new();
        assert_eq!(
            chat.update(
                ChatUpdate::Turn {
                    running: true,
                    by_agent: false
                },
                100
            ),
            Some(step(StepActivity::Working, 100))
        );
        let thought = TranscriptContent::Thinking {
            text: "Hm".into(),
            signature: String::new(),
        };
        assert_eq!(
            chat.item(1, ItemState::Streaming, thought, 200),
            Some(step(StepActivity::Thinking { subagent: false }, 200))
        );
        let more = ChatUpdate::Transcript {
            change: TranscriptChange::AppendThinking {
                sequence: 1,
                text: "m".into(),
            },
        };
        assert_eq!(
            chat.update(more, 300),
            None,
            "the same step publishes nothing"
        );
        assert_eq!(
            chat.step(),
            step(StepActivity::Thinking { subagent: false }, 200)
        );
        assert_eq!(
            chat.item(2, ItemState::Complete, bash("t1", None), 400),
            Some(step(running("t1"), 400))
        );
        assert_eq!(
            chat.item(2, ItemState::Complete, bash("t1", Some("ok")), 500),
            Some(step(StepActivity::Working, 500))
        );
        assert_eq!(
            chat.item(3, ItemState::Complete, bash("t2", None), 600),
            Some(step(running("t2"), 600)),
            "the next Bash is a step of its own"
        );
        let reply = TranscriptContent::Assistant {
            text: "Done".into(),
            model: None,
        };
        assert_eq!(
            chat.item(4, ItemState::Streaming, reply, 700),
            Some(step(StepActivity::Writing { subagent: false }, 700))
        );
        let result = TranscriptContent::System {
            kind: SystemKind::Result,
            text: "Turn completed".into(),
            detail_json: "{}".into(),
        };
        chat.item(5, ItemState::Complete, result, 800);
        assert_eq!(
            chat.update(
                ChatUpdate::Turn {
                    running: false,
                    by_agent: false
                },
                900
            ),
            Some(None),
            "the turn's end ends its step"
        );
    }

    /// Which turn the chat shows running, and whether the user can send, as starts and ends of either kind arrive in
    /// the orders the registry and the space actor can publish them in.
    #[test]
    fn a_turn_ends_only_by_its_own_kind_and_the_agents_never_replaces_the_users() {
        let ours = |running| ChatUpdate::Turn {
            running,
            by_agent: false,
        };
        let agents = |running| ChatUpdate::Turn {
            running,
            by_agent: true,
        };
        let shown = |chat: &Fixture| (chat.chats[0].turn_running, chat.chats[0].agent_turn);

        let mut chat = Fixture::new();
        chat.update(agents(true), 100);
        assert_eq!(shown(&chat), (true, true), "the agent's own turn");
        chat.update(agents(false), 200);
        assert_eq!(shown(&chat), (false, false));

        // A prompt sent during the agent's turn joins it: the turn is the user's, and the agent's end doesn't end it.
        chat.update(agents(true), 300);
        chat.update(ours(true), 400);
        assert_eq!(shown(&chat), (true, false));
        chat.update(agents(false), 500);
        assert_eq!(
            shown(&chat),
            (true, false),
            "a late end of the agent's turn"
        );
        chat.update(ours(false), 600);
        assert_eq!(shown(&chat), (false, false), "one end for the user's turn");

        // The agent's turn starting just after the user sent: the user's stays.
        chat.update(ours(true), 700);
        chat.update(agents(true), 800);
        assert_eq!(shown(&chat), (true, false));
        chat.update(ours(false), 900);
        assert_eq!(shown(&chat), (false, false), "no spinner is left on");

        // The known limit: a send that fails during the agent's turn ends the shown turn, and the agent's own end,
        // if it comes, then changes nothing; nothing is left on.
        chat.update(agents(true), 1000);
        chat.update(ours(true), 1100);
        chat.update(ours(false), 1200);
        assert_eq!(shown(&chat), (false, false), "the failed send's end");
        chat.update(agents(false), 1300);
        assert_eq!(shown(&chat), (false, false));
    }

    #[test]
    fn a_subagents_latest_item_is_the_step_and_says_whose_it_is() {
        let mut chat = Fixture::new();
        chat.update(
            ChatUpdate::Turn {
                running: true,
                by_agent: false,
            },
            100,
        );
        chat.item(1, ItemState::Complete, bash("task", None), 200);
        let thought = TranscriptContent::Thinking {
            text: "Hm".into(),
            signature: String::new(),
        };
        assert_eq!(
            chat.item_of(Some("task"), 2, ItemState::Streaming, thought, 300),
            Some(step(StepActivity::Thinking { subagent: true }, 300))
        );
        let read = StepActivity::Running {
            tool: "Bash".into(),
            tool_use_id: "read".into(),
            subagent: true,
        };
        assert_eq!(
            chat.item_of(
                Some("task"),
                3,
                ItemState::Complete,
                bash("read", None),
                400
            ),
            Some(step(read, 400))
        );
        let reply = TranscriptContent::Assistant {
            text: "Done".into(),
            model: None,
        };
        assert_eq!(
            chat.item(4, ItemState::Streaming, reply, 500),
            Some(step(StepActivity::Writing { subagent: false }, 500)),
            "back to the agent's own step"
        );
    }

    #[test]
    fn a_permission_question_is_the_step_until_it_is_answered() {
        let mut chat = Fixture::new();
        chat.update(
            ChatUpdate::Turn {
                running: true,
                by_agent: false,
            },
            100,
        );
        chat.item(1, ItemState::Complete, bash("t1", None), 200);
        let question = PermissionQuestion {
            request_id: "q1".into(),
            tool_name: "Bash".into(),
            tool_use_id: "t1".into(),
            input_json: "{}".into(),
            decision_reason: None,
            can_always_allow: true,
            suggested_rules_json: None,
        };
        let waiting = StepActivity::Waiting {
            tool: "Bash".into(),
            request_id: "q1".into(),
        };
        assert_eq!(
            chat.update(ChatUpdate::PermissionRequested { question }, 300),
            Some(step(waiting, 300))
        );
        assert_eq!(
            chat.update(
                ChatUpdate::PermissionRemoved {
                    request_id: "q1".into()
                },
                900
            ),
            Some(step(running("t1"), 900)),
            "once answered, the tool's run is timed from the answer"
        );
    }

    #[test]
    fn a_chat_that_stops_has_no_step() {
        let mut chat = Fixture::new();
        chat.update(
            ChatUpdate::Turn {
                running: true,
                by_agent: false,
            },
            100,
        );
        assert_eq!(
            chat.update(
                ChatUpdate::Phase {
                    phase: ChatPhase::Paused
                },
                200
            ),
            Some(None)
        );
        assert_eq!(chat.step(), None);
    }
}

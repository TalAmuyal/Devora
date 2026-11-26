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
    /// The levels its current model takes, lowest first, as Claude Code lists them; empty when it takes none, none when
    /// the list isn't known.
    pub levels: Option<Vec<String>>,
    /// The list is known, but its model isn't in it (an agent can be set to any name): which levels it takes is unknown,
    /// so `levels` is none and nothing is claimed about it.
    pub model_unknown: bool,
    /// What Claude Code runs instead of `set_to` when the model doesn't take it: the nearest level below that it takes
    /// (the pinned-CLI suite asserts the rule). None when it takes the level set, or there's no level below to name.
    pub runs_as: Option<String>,
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
    /// Why the agent is paused or pausing, as the registry holds it.
    pub pause: AgentPause,
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

/// Why an agent is paused or pausing: the registry's one state for it (devora-core's `Agents`), which the chat shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AgentPause {
    /// Not paused for any reason the registry holds.
    None,
    /// The user paused it (after its turn), or it was kept paused through a release; stored with the agent.
    ByUser,
    /// Opened with the pause stored before this run, and not launched since. Unlike a pause made in this run, a hold
    /// that reaches it reads that pause as the user's Keep paused choice.
    PausedAtOpen,
    /// Held by the Global Pause; `keep_paused` is the user's choice to stay paused when it's released, stored.
    /// `released` once the hold is lifted while the agent was still stopping for it: it launches again once stopped,
    /// or, kept paused, stays paused at its boundary.
    Held { keep_paused: bool, released: bool },
}

/// What the pause widget reads, from the agent's pause and its chat's phase (`pause_view`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PauseView {
    /// A pause is on its way: the user's, a hold's until it's released, or Keep paused's. Never once the agent is
    /// Paused or Failed.
    pub pause_requested: bool,
    pub globally_held: bool,
    pub keep_paused: bool,
}

/// The pause widget's view of `pause`: a held agent is asked to pause by the hold itself until it's released, and
/// after that only if kept paused.
pub fn pause_view(pause: &AgentPause, phase: &ChatPhase) -> PauseView {
    let on_its_way = match pause {
        AgentPause::None | AgentPause::PausedAtOpen => false,
        AgentPause::ByUser => true,
        AgentPause::Held {
            keep_paused,
            released,
        } => *keep_paused || !*released,
    };
    PauseView {
        pause_requested: on_its_way && !matches!(phase, ChatPhase::Paused | ChatPhase::Failed),
        globally_held: matches!(pause, AgentPause::Held { .. }),
        keep_paused: matches!(
            pause,
            AgentPause::Held {
                keep_paused: true,
                ..
            }
        ),
    }
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
    Pause {
        pause: AgentPause,
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
                pause: AgentPause::None,
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
                ChatUpdate::Pause { pause } => chat.pause = pause.clone(),
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

    /// A row's later update (a message the agent got at last, or never will) for a row the chat no longer holds (it
    /// keeps the latest 200) doesn't bring the row back: the chat keeps its tail.
    #[test]
    fn an_update_to_a_row_past_the_chats_tail_leaves_the_tail_as_it_is() {
        let mut chat = Fixture::new();
        let user = |delivery| TranscriptContent::User {
            text: "Lost".into(),
            error: None,
            delivery,
        };
        chat.item(
            1,
            ItemState::Complete,
            user(Some(crate::Delivery::Waiting)),
            0,
        );
        for sequence in 2..=201 {
            chat.item(sequence, ItemState::Complete, user(None), 0);
        }
        let tail = |chat: &Fixture| {
            chat.chats[0]
                .items
                .iter()
                .map(|item| item.sequence)
                .collect::<Vec<_>>()
        };
        assert_eq!(tail(&chat), (2..=201).collect::<Vec<_>>());
        chat.item(
            1,
            ItemState::Complete,
            user(Some(crate::Delivery::NotDelivered)),
            0,
        );
        assert_eq!(tail(&chat), (2..=201).collect::<Vec<_>>());
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

    /// The pause widget's inputs from one pause state are what the three chat updates it replaces gave, along every
    /// path the registry takes. Each step is a registry transition as it was published before (the old updates, in
    /// their order, from devora-core's `agents`) and as it is now (one `AgentPause`), or a phase change both share.
    /// Where the two differ, the row says why the widget can't tell.
    #[test]
    fn the_pause_widget_reads_what_the_three_updates_gave() {
        #[derive(Clone, Copy)]
        enum Old {
            Hold(bool),
            Requested(bool),
            Keep(bool),
        }
        #[derive(Clone)]
        enum Step {
            /// A registry transition: what was sent, and what is sent now.
            Pause(&'static str, &'static [Old], AgentPause),
            Phase(ChatPhase),
            Failure,
            /// A new run: its chat starts afresh.
            Start,
        }
        use AgentPause::*;
        use Old::*;
        let held = |keep_paused, released| Held {
            keep_paused,
            released,
        };
        let check = |keep| {
            Step::Pause(
                "the hold reaches it",
                if keep {
                    &[Hold(true), Requested(true), Keep(true)]
                } else {
                    &[Hold(true), Requested(true), Keep(false)]
                },
                held(keep, false),
            )
        };
        let launch = Step::Pause("launch", &[Hold(false)], None);
        let user_pause = Step::Pause(
            "Pause after the turn",
            &[Hold(false), Requested(true)],
            ByUser,
        );
        let running = [
            Step::Start,
            Step::Phase(ChatPhase::Opening),
            launch.clone(),
            Step::Phase(ChatPhase::Running),
        ];
        const STALE_KEEP: &str = "the old chat kept keep_paused after a kept-paused release; the widget reads it only while Devora is paused, and then a Paused agent reads as kept paused either way (keptPaused: !globallyHeld && phase == .paused)";
        let then =
            |head: &[Step], tail: Vec<Step>| head.iter().cloned().chain(tail).collect::<Vec<_>>();
        /// A path's name, its steps, and the steps from which the two are expected to differ, each with why.
        type Path = (&'static str, Vec<Step>, &'static [(usize, &'static str)]);
        let paths: Vec<Path> = vec![
            ("the user pauses", then(&running, vec![user_pause.clone(), Step::Phase(ChatPhase::Paused)]), &[]),
            ("the user's pause fails", then(&running, vec![user_pause.clone(), Step::Pause("the pause failed", &[Requested(false)], None)]), &[]),
            ("held at the boundary, released once stopped, launched again", then(&running, vec![
                check(false), Step::Phase(ChatPhase::Paused),
                Step::Start, Step::Phase(ChatPhase::Opening), launch.clone(), Step::Phase(ChatPhase::Running),
            ]), &[]),
            ("released before the boundary", then(&running, vec![
                check(false), Step::Pause("released, not stopping", &[Requested(false), Hold(false)], None),
            ]), &[]),
            ("released while stopping, then launched again", then(&running, vec![
                check(false), Step::Pause("released while stopping", &[Requested(false), Hold(true)], held(false, true)),
                Step::Pause("held again while stopping", &[], held(false, true)),
                Step::Phase(ChatPhase::Paused),
                Step::Start, Step::Phase(ChatPhase::Opening), launch.clone(), Step::Phase(ChatPhase::Running),
            ]), &[]),
            ("kept paused, stopped before the release", then(&running, vec![
                check(false), Step::Pause("Keep paused ticked", &[Keep(true)], held(true, false)),
                Step::Phase(ChatPhase::Paused),
                Step::Pause("kept paused at the release", &[Hold(false)], ByUser),
            ]), &[(7, STALE_KEEP)]),
            ("kept paused, released mid-turn", then(&running, vec![
                check(false), Step::Pause("Keep paused ticked", &[Keep(true)], held(true, false)),
                Step::Pause("released, kept paused", &[Requested(true), Hold(true)], held(true, true)),
                Step::Phase(ChatPhase::Paused),
                Step::Pause("kept paused at its boundary", &[Hold(false)], ByUser),
            ]), &[(8, STALE_KEEP)]),
            ("Keep paused unticked", then(&running, vec![
                check(false), Step::Pause("Keep paused ticked", &[Keep(true)], held(true, false)),
                Step::Pause("Keep paused unticked", &[Keep(false)], held(false, false)),
            ]), &[]),
            ("opened with a stored pause", vec![Step::Start, Step::Phase(ChatPhase::Paused)], &[]),
            ("opened with a stored pause during a hold, unticked, released", vec![
                Step::Start, check(true), Step::Phase(ChatPhase::Paused),
                Step::Pause("Keep paused unticked", &[Keep(false)], held(false, false)),
                Step::Start, Step::Phase(ChatPhase::Opening), launch.clone(), Step::Phase(ChatPhase::Running),
            ], &[]),
            ("paused by the user while held, before launching", vec![
                Step::Start, check(false), Step::Phase(ChatPhase::Paused),
                Step::Pause("Pause while held, not launched", &[Hold(false)], ByUser),
            ], &[]),
            ("paused by the user while held, running", then(&running, vec![check(false), user_pause.clone(), Step::Phase(ChatPhase::Paused)]), &[]),
            ("fails while held", then(&running, vec![check(false), Step::Failure]), &[]),
            ("the user's pause fails while held with Keep paused, and the hold takes it again", then(&running, vec![
                check(false), Step::Pause("Keep paused ticked", &[Keep(true)], held(true, false)),
                user_pause.clone(),
                Step::Pause("the pause failed", &[Requested(false)], None),
                // The old registry still had Keep paused in memory, so its hold sent it set.
                Step::Pause("the hold takes it again", &[Hold(true), Requested(true), Keep(true)], held(false, false)),
            ]), &[(6, STALE_KEEP), (7, "the one intended difference: the failed pause cleared the stored pause, but the old registry kept Keep paused in memory, so the hold took the agent again with it ticked while nothing said so; now it follows what's stored, unticked")]),
            ("deleted while running", then(&running, vec![
                Step::Pause("the immediate pause", &[Hold(false)], ByUser),
                Step::Phase(ChatPhase::Pausing),
            ]), &[(4, "the immediate pause never sent PauseRequested; the widget shows Pausing either way (isPausing is pauseRequested || phase == .pausing) and nothing reads pauseRequested outside .running, and the tab closes")]),
        ];
        for (path, steps, expected) in paths {
            let (mut old, mut pause, mut phase) = ((false, false, false), None, ChatPhase::Opening);
            for (index, step) in steps.iter().enumerate() {
                match step {
                    Step::Pause(_, updates, next) => {
                        for update in updates.iter() {
                            match update {
                                Hold(held) => old.1 = *held,
                                Requested(requested) => old.0 = *requested,
                                Keep(keep) => old.2 = *keep,
                            }
                        }
                        pause = next.clone();
                    }
                    Step::Phase(next) => {
                        phase = next.clone();
                        if matches!(next, ChatPhase::Paused | ChatPhase::Failed) {
                            old.0 = false;
                        }
                    }
                    Step::Failure => {
                        phase = ChatPhase::Failed;
                        old.0 = false;
                    }
                    Step::Start => {
                        old = (false, false, false);
                        pause = None;
                        phase = ChatPhase::Opening;
                    }
                }
                let view = pause_view(&pause, &phase);
                let same = (view.pause_requested, view.globally_held, view.keep_paused) == old;
                let label = match step {
                    Step::Pause(name, ..) => name.to_string(),
                    other => format!(
                        "{:?}",
                        match other {
                            Step::Phase(phase) => format!("{phase:?}"),
                            Step::Failure => "Failure".into(),
                            _ => "Start".into(),
                        }
                    ),
                };
                if expected.iter().any(|(from, _)| index >= *from) {
                    assert!(!same, "{path}, step {index} ({label}): expected to differ");
                } else {
                    assert!(same, "{path}, step {index} ({label}): old {old:?}, new {view:?} from {pause:?} in {phase:?}");
                }
            }
        }
    }
}

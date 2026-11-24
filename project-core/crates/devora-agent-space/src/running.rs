use crate::compaction::{Compaction, Context};
use crate::restart_notice::{AppRun, RestartNotice, StoppedTask};
use crate::Space;
use devora_agent::protocol::{BackgroundTask, KnownMessage, Message, System};
use devora_agent::session::stop_tree;
use devora_agent::session::{
    saved_model, saved_turn, PermissionAnswer, Resume, Session, SessionEvent, SessionHandle,
    SessionOptions, StopInfo, UsageSource,
};
use devora_agent::transcript::{SystemKind, Transcript, TranscriptChange, TranscriptItem};
use devora_app::log::{Level, Log};
use devora_platform::processes::SystemProcessTable;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

#[derive(Debug, Clone)]
pub enum SpaceEvent {
    Restored(Vec<TranscriptItem>),
    Transcript(TranscriptChange),
    Session(SessionEvent),
    /// How full the context is, whenever that changes.
    Context(Context),
    /// The model the agent's replies come from, at the start and whenever that changes.
    Model(ActiveModel),
    /// The model the user set the agent to in this run, whenever that or its telling changes.
    ChosenModel(ChosenModel),
    /// A turn the space sent itself: compaction's, or the restart notice.
    TurnStarted,
    /// The CLI answered the running turn, or exited during it. Results the CLI
    /// produces by itself (background work finishing, a resume) end nothing.
    TurnEnded,
}

/// The model of the latest reply in the agent's own conversation (a full ID; none before one is known), and Claude
/// Code's sentence for the fallback it came from, while replies still come from that fallback.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActiveModel {
    pub latest: Option<String>,
    pub fallback: Option<String>,
}

/// Tells the CLI the model the user set, off the pump, reporting the outcome back to it.
fn tell_model(
    runtime: SessionHandle,
    model: String,
    told: mpsc::UnboundedSender<(String, Result<Value, String>)>,
) {
    let telling = runtime.set_model(model.clone());
    tokio::spawn(async move {
        let outcome = telling.await;
        let _ = told.send((model, outcome));
    });
}

/// A reply in the agent's own conversation from a model (not one Claude Code made up), which a chosen model awaits.
fn is_reply(event: &SessionEvent) -> bool {
    match event {
        SessionEvent::Message(
            message @ Message::Known(KnownMessage::Assistant { message: turn, .. }),
        ) => {
            message.is_main_thread()
                && turn
                    .extra
                    .get("model")
                    .and_then(Value::as_str)
                    .is_some_and(devora_agent::protocol::is_model)
        }
        _ => false,
    }
}

/// A model the user set the agent to while it runs (`SpaceHandle::set_model`): what it is, whether a reply has come
/// since (until one has, the agent hasn't answered on it yet), and why its CLI didn't take it, if it didn't. A relaunch
/// starts with none: the stored model is then simply what the agent runs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChosenModel {
    pub set_to: Option<String>,
    pub awaiting_reply: bool,
    pub not_taken: Option<String>,
}

impl ActiveModel {
    fn of(transcript: &Transcript) -> Self {
        Self {
            latest: transcript.latest_model().map(str::to_owned),
            fallback: transcript.fallback_note().map(str::to_owned),
        }
    }
}

/// How long a pause waits for the agent's background tasks to stop, and a
/// stopped subagent's notification turn to end, before closing the CLI's input.
const PREPARE_GRACE: std::time::Duration = std::time::Duration::from_secs(5);

/// How long a launch waits for a CLI left running by an earlier Devora to exit after `SIGKILL`.
const LEFT_RUNNING_WAIT: std::time::Duration = std::time::Duration::from_secs(5);

/// Observes the host-owned global hold at turn admission and turn boundaries.
/// The host callback may also record eligibility and publish hold status.
pub type GlobalPauseCheck = Arc<dyn Fn() -> bool + Send + Sync>;

enum Action {
    Turn {
        id: String,
        text: String,
    },
    RestartNotice {
        id: String,
        notice: RestartNotice,
    },
    Interrupt,
    Permission(String, PermissionAnswer),
    PauseAfterTurn,
    GlobalPauseChanged,
    /// The host is pausing the session, which interrupts a running turn: its result is a stop, not a failure.
    ExpectStop,
    /// The agent's own threshold, or none to follow the profile's.
    SetCompactThreshold(Option<u8>),
    CompactNow,
    /// The model the agent runs from now on: stored, then told to the CLI between turns.
    SetModel(String),
}
struct Command {
    action: Action,
    reply: oneshot::Sender<Result<Value, String>>,
}
struct Completion {
    sequence: u64,
    /// A turn that failed to send never runs, so it never ends with a result.
    failed_turn: bool,
    result: Result<Value, String>,
    reply: oneshot::Sender<Result<Value, String>>,
}

/// Commands remain owned by the space actor after a caller abandons its wait.
/// Intentions commit before transport and outcomes commit before replies.
#[derive(Clone)]
pub struct SpaceHandle {
    runtime: SessionHandle,
    commands: mpsc::UnboundedSender<Command>,
}
impl SpaceHandle {
    async fn call(&self, action: Action) -> Result<Value, String> {
        let (reply, received) = oneshot::channel();
        self.commands
            .send(Command { action, reply })
            .map_err(|_| "agent space stopped")?;
        received
            .await
            .map_err(|_| "agent space stopped before recording the outcome")?
    }
    /// Wake the shared boundary and report whether stopping has already begun.
    pub async fn global_pause_changed(&self) -> Result<bool, String> {
        self.call(Action::GlobalPauseChanged)
            .await?
            .as_bool()
            .ok_or_else(|| "invalid pause-boundary response".into())
    }

    pub async fn send_turn(&self, text: String) -> Result<(), String> {
        self.call(Action::Turn {
            id: uuid::Uuid::new_v4().to_string(),
            text,
        })
        .await
        .map(|_| ())
    }
    pub async fn interrupt(&self) -> Result<Value, String> {
        self.call(Action::Interrupt).await
    }
    pub async fn answer_permission(
        &self,
        id: String,
        answer: PermissionAnswer,
    ) -> Result<(), String> {
        self.call(Action::Permission(id, answer)).await.map(|_| ())
    }
    /// Sets the model the agent runs: stored with the agent at once, so its next launch uses it, and told to its CLI
    /// between turns (at once when idle, else when the running turn ends, one Devora sent or one the CLI started itself;
    /// a later change replaces one not yet told), since what the CLI does with one mid-turn hasn't been checked.
    pub async fn set_model(&self, model: String) -> Result<(), String> {
        self.call(Action::SetModel(model)).await.map(|_| ())
    }
    pub async fn set_permission_mode(&self, mode: String) -> Result<Value, String> {
        self.runtime.set_permission_mode(mode).await
    }
    pub async fn answer_mcp(&self, request_id: String, response: Value) -> Result<(), String> {
        self.runtime.answer_mcp(request_id, response).await
    }
    /// Pauses the session, interrupting a running turn, whose result the transcript then shows as stopped. An actor
    /// that has already stopped can't be told, and has no turn running.
    pub async fn pause(&self) -> Result<StopInfo, String> {
        let _ = self.call(Action::ExpectStop).await;
        self.runtime.pause().await
    }
    /// Sets this agent's compaction threshold, or clears it to follow the
    /// profile's; committed before it applies, and checked when a turn ends.
    pub async fn set_compact_threshold(&self, percent: Option<u8>) -> Result<(), String> {
        if percent.is_some_and(|percent| !(1..=100).contains(&percent)) {
            return Err("a compaction threshold is a whole percentage from 1 to 100".into());
        }
        self.call(Action::SetCompactThreshold(percent))
            .await
            .map(|_| ())
    }
    /// Compacts as reaching the threshold would: now with no turn running, else
    /// when the running turn ends.
    pub async fn compact_now(&self) -> Result<(), String> {
        self.call(Action::CompactNow).await.map(|_| ())
    }
    /// Lets the running turn finish, then pauses, with no compaction step after
    /// it; with no turn running, pauses at once. Queues immediately, and the
    /// returned future resolves once the agent has paused.
    pub fn pause_after_turn(
        &self,
    ) -> impl std::future::Future<Output = Result<(), String>> + Send + 'static {
        // Queue before returning so a following permission answer cannot overtake
        // this request. The actor owns it even if the completion wait is dropped.
        let (reply, received) = oneshot::channel();
        let queued = self
            .commands
            .send(Command {
                action: Action::PauseAfterTurn,
                reply,
            })
            .map_err(|_| "agent space stopped".to_owned());
        async move {
            queued?;
            received
                .await
                .map_err(|_| "agent space stopped before recording the outcome")?
                .map(|_| ())
        }
    }
}

/// The actor owns the SQLite connection and OS lock through final persistence.
pub struct RunningSpace {
    handle: SpaceHandle,
    events: mpsc::UnboundedReceiver<SpaceEvent>,
    /// Ends with the stop the actor persisted, when it stopped the session itself.
    worker: Option<JoinHandle<Result<Option<StopInfo>, String>>>,
    settings: Value,
    model: String,
}
impl RunningSpace {
    pub fn handle(&self) -> SpaceHandle {
        self.handle.clone()
    }
    /// The settings the CLI was launched with (`SessionOptions::settings`): the host's, with the profile's model tiers.
    pub fn launch_settings(&self) -> &Value {
        &self.settings
    }
    /// The model the CLI was launched with (`SessionOptions::model`): what the agent is set to, an alias or a full ID.
    pub fn launch_model(&self) -> &str {
        &self.model
    }
    pub async fn next_update(&mut self) -> Option<SpaceEvent> {
        self.events.recv().await
    }
    /// Compatibility for callers interested only in runtime lifecycle events.
    pub async fn next_event(&mut self) -> Option<SessionEvent> {
        while let Some(event) = self.next_update().await {
            if let SpaceEvent::Session(event) = event {
                return Some(event);
            }
        }
        None
    }
    /// A session the actor has already stopped (its own pause after a turn)
    /// can't be asked again, so it answers with the stop the actor recorded,
    /// graceful or forced.
    pub async fn pause(&mut self) -> Result<StopInfo, String> {
        let requested = self.handle.pause().await;
        let recorded = match self.worker.take() {
            Some(worker) => worker
                .await
                .map_err(|e| format!("agent space worker failed: {e}"))??,
            None => None,
        };
        requested.or_else(|error| recorded.ok_or(error))
    }
}

pub(crate) async fn start(
    mut space: Space,
    options: SessionOptions,
    mut compaction: Compaction,
    log: Arc<Log>,
    globally_paused: GlobalPauseCheck,
    run: &AppRun,
) -> Result<RunningSpace, String> {
    let run = run.clone();
    let settings = options.settings.clone();
    let model = options.model.clone();
    let launch = options.clone();
    let launch_log = log.clone();
    let (mut space, mut transcript, history, notice) = tokio::task::spawn_blocking(move || {
        let mut transcript = space.db.fold(&space.resume.session_id)?;
        // A conversation stored before replies kept their model: Claude's saved session says which one answered last.
        // Failing to read it only leaves the model unknown until the next reply.
        if transcript.latest_model().is_none() && space.resume.has_conversation {
            match saved_model(&launch, &space.resume.session_id) {
                Ok(Some(model)) => transcript.continue_from(model),
                Ok(None) => {}
                Err(error) => launch_log.write(
                    Level::Warn,
                    &format!("could not read the model of the agent's latest reply: {error}"),
                ),
            }
        }
        let mut interrupted = transcript.interrupt(!space.resume.has_conversation);
        space.db.save_changes(None, &mut interrupted)?;
        transcript.release_completed();
        let history = space.history()?;
        // A CLI an earlier Devora left running (it died without stopping it) would
        // write the same session as the one resumed now, forking it.
        if let Some(cli) = space.db.cli_process()? {
            match stop_tree(&SystemProcessTable, cli, LEFT_RUNNING_WAIT) {
                Ok(true) => launch_log.write(
                    Level::Warn,
                    &format!(
                        "stopped Claude Code (pid {}) that an earlier Devora left running",
                        cli.pid
                    ),
                ),
                Ok(false) => {}
                Err(error) => {
                    return Err(format!(
                        "Claude Code left running by an earlier Devora couldn't be stopped: {error}"
                    ))
                }
            }
            space.db.record_cli(None)?;
        }
        let (launched, stored) = space.db.launch_state()?;
        let attempt_saved = match stored.as_ref().and_then(|notice| notice.attempt.as_deref()) {
            Some(attempt) => {
                saved_turn(&launch, &space.resume.session_id, attempt).unwrap_or_else(|error| {
                    // Unknown counts as undelivered: a resend may repeat, but a notice is never lost.
                    launch_log.write(
                        Level::Warn,
                        &format!("restart notice delivery unknown: {error}"),
                    );
                    false
                })
            }
            None => false,
        };
        let notice = RestartNotice::at_launch(
            stored,
            attempt_saved,
            launched.as_deref() != Some(run.id()),
            space.resume.has_conversation,
        );
        space.db.record_launch(run.id(), notice.as_ref())?;
        Ok::<_, String>((space, transcript, history, notice))
    })
    .await
    .map_err(|e| e.to_string())??;
    let mut session = match Session::start(options, log.clone()).await {
        Ok(session) => session,
        Err(error) => {
            let change = transcript.system(SystemKind::Failure, &error, json!({}));
            space.db.save_changes(None, &mut [change])?;
            return Err(error);
        }
    };
    let (events, received) = mpsc::unbounded_channel();
    let _ = events.send(SpaceEvent::Restored(history));
    let (commands, received_commands) = mpsc::unbounded_channel();
    // Owed since a restart, the notice is the first turn admitted; a hold
    // refuses it as any turn, and it stays owed for the next launch.
    if let Some(notice) = notice {
        let (reply, _) = oneshot::channel();
        let _ = commands.send(Command {
            action: Action::RestartNotice {
                id: uuid::Uuid::new_v4().to_string(),
                notice,
            },
            reply,
        });
    }
    let handle = SpaceHandle {
        runtime: session.handle(),
        commands,
    };
    let worker = tokio::spawn(async move {
        let result = pump(
            &mut space,
            &mut session,
            &mut transcript,
            &mut compaction,
            &events,
            received_commands,
            globally_paused,
        )
        .await;
        if let Err(error) = &result {
            log.report_error(error);
            transcript.expect_stop();
            if let Ok(info) = session.pause().await {
                let _ = persist(
                    &mut space,
                    &mut transcript,
                    &SessionEvent::Stopped(info),
                    &events,
                );
            }
            // A transient write failure may have cleared during shutdown. Keep
            // the failure in history when storage can accept it again.
            let _ = persist(
                &mut space,
                &mut transcript,
                &SessionEvent::Failed(error.clone()),
                &events,
            );
            let _ = events.send(SpaceEvent::Session(SessionEvent::Failed(error.clone())));
        }
        result
    });
    Ok(RunningSpace {
        handle,
        events: received,
        worker: Some(worker),
        settings,
        model,
    })
}

fn persist(
    space: &mut Space,
    transcript: &mut Transcript,
    event: &SessionEvent,
    events: &mpsc::UnboundedSender<SpaceEvent>,
) -> Result<(), String> {
    match event {
        SessionEvent::Spawned(cli) => space.db.record_cli(Some(cli))?,
        // A stop that had to end the agent's background work owes it the notice
        // to re-establish what it needs, at its next launch.
        SessionEvent::Stopped(info) => {
            space.db.record_cli(None)?;
            if !info.graceful || info.interrupted_turn {
                space.db.owe_notice(RestartNotice::after_stop())?;
            }
        }
        // It ended under Devora's watch, so there's nothing left to stop at the next launch.
        SessionEvent::Exited { .. } | SessionEvent::Failed(_) => space.db.record_cli(None)?,
        _ => {}
    }
    let resume = match event {
        SessionEvent::Ready {
            session_id,
            resumed,
            ..
        } => Some(Resume {
            session_id: session_id.clone(),
            has_conversation: *resumed,
        }),
        SessionEvent::Message(Message::Known(KnownMessage::Result(result))) => Some(Resume {
            session_id: result.session_id.clone(),
            has_conversation: true,
        }),
        SessionEvent::Stopped(info) => Some(Resume {
            session_id: info.session_id.clone(),
            has_conversation: info.has_conversation,
        }),
        SessionEvent::Failed(_)
        | SessionEvent::Exited {
            restarting: false, ..
        } => Some(space.resume.clone()),
        _ => None,
    };
    let mut changes = if matches!(
        event,
        SessionEvent::Exited { .. } | SessionEvent::Stopped(_) | SessionEvent::Failed(_)
    ) {
        transcript.interrupt(!resume.as_ref().unwrap_or(&space.resume).has_conversation)
    } else {
        vec![]
    };
    let model_before = ActiveModel::of(transcript);
    changes.extend(transcript.event(event));
    space.db.save_changes(resume.as_ref(), &mut changes)?;
    transcript.release_completed();
    if let Some(resume) = resume {
        space.resume = resume;
    }
    for change in changes {
        let _ = events.send(SpaceEvent::Transcript(change));
    }
    if ActiveModel::of(transcript) != model_before {
        let _ = events.send(SpaceEvent::Model(ActiveModel::of(transcript)));
    }
    Ok(())
}

fn begin(
    space: &mut Space,
    transcript: &mut Transcript,
    command: Command,
    runtime: SessionHandle,
    completions: mpsc::UnboundedSender<Completion>,
    events: &mpsc::UnboundedSender<SpaceEvent>,
) -> Result<(), String> {
    let mut change = match &command.action {
        Action::Turn { text, .. } => transcript.submit(text.clone()),
        Action::RestartNotice { id, notice } => transcript.intention(
            SystemKind::RestartNotice,
            notice.row(),
            json!({"turn": id, "recovery": notice.recovery}),
        ),
        Action::Interrupt => transcript.intention(
            SystemKind::Interruption,
            "Interrupt requested".into(),
            json!({}),
        ),
        Action::PauseAfterTurn
        | Action::GlobalPauseChanged
        | Action::ExpectStop
        | Action::SetCompactThreshold(_)
        | Action::CompactNow
        | Action::SetModel(_) => return Err("the pump handles this action itself".into()),
        Action::Permission(id, answer) => {
            let (text, decision) = match answer {
                PermissionAnswer::Allow => ("Permission allow requested", json!({"kind":"allow"})),
                PermissionAnswer::Deny { message } => (
                    "Permission denial requested",
                    json!({"kind":"deny","message":message}),
                ),
                PermissionAnswer::AllowWithSuggestedRule => (
                    "Permission allow with suggested rule requested",
                    json!({"kind":"allow_with_suggested_rule"}),
                ),
            };
            transcript.intention(
                SystemKind::PermissionAnswer,
                text.into(),
                json!({"request_id":id,"answer":decision}),
            )
        }
    };
    let TranscriptChange::Upsert { item } = &change else {
        return Err("intention must be a whole item".into());
    };
    let sequence = item.sequence;
    let saved = match &command.action {
        Action::RestartNotice { id, notice } => space.db.save_notice_attempt(
            &mut change,
            &RestartNotice {
                attempt: Some(id.clone()),
                ..notice.clone()
            },
        ),
        _ => space
            .db
            .save_changes(None, std::slice::from_mut(&mut change)),
    };
    if let Err(error) = saved {
        let _ = command.reply.send(Err(error.clone()));
        return Err(error);
    }
    let _ = events.send(SpaceEvent::Transcript(change));
    let turn = matches!(
        command.action,
        Action::Turn { .. } | Action::RestartNotice { .. }
    );
    tokio::spawn(async move {
        let result = match command.action {
            Action::Turn { id, text } => runtime.send_turn(id, text).await.map(|_| Value::Null),
            Action::RestartNotice { id, notice } => runtime
                .send_turn(id, notice.prompt())
                .await
                .map(|_| Value::Null),
            Action::Interrupt => runtime.interrupt().await,
            Action::Permission(id, answer) => runtime
                .answer_permission(id, answer)
                .await
                .map(|_| Value::Null),
            Action::PauseAfterTurn
            | Action::GlobalPauseChanged
            | Action::ExpectStop
            | Action::SetCompactThreshold(_)
            | Action::CompactNow
            | Action::SetModel(_) => unreachable!("handled by the pump"),
        };
        let _ = completions.send(Completion {
            sequence,
            failed_turn: turn && result.is_err(),
            result,
            reply: command.reply,
        });
    });
    Ok(())
}

/// The turn the CLI is running, as the actor knows it: one Devora sent, or one the CLI started itself (when a background
/// task reports, it runs a notification turn of its own). Every "between turns" check (a pause's boundary, telling the
/// CLI a new model, starting a compaction the user asked for) waits for either kind.
///
/// How it moves (`observe`), so that it never sticks:
/// - none, then `init`: the CLI started a turn itself (`Agents`);
/// - `Agents`, then a turn Devora sends: that turn is now the one running (`Ours`), since the CLI folds a turn sent
///   during its own into it and answers it with one result (probed on the pinned CLI);
/// - `Agents` ends on any result (its own, or the interrupt's after Stop), or on the CLI exiting, stopping or failing;
/// - `Ours` ends on the result answering it, or on the CLI exiting (and on a failed send, `Completion::failed_turn`);
///   a result that answers no turn of Devora's passes it by;
/// - another `init` while either runs changes nothing: the CLI sends one per turn, and a background task finishing
///   during Devora's turn is folded into it, with no `init` and no result of its own (probed on the pinned CLI).
#[derive(Clone, Debug, PartialEq, Eq)]
enum ActiveTurn {
    /// A turn Devora sent, by the ID the CLI answers it with.
    Ours(String),
    /// A turn the CLI started itself.
    Agents,
}

impl ActiveTurn {
    /// Folds `event` into the turn running, and returns the turn it ended, if any.
    fn observe(running: &mut Option<ActiveTurn>, event: &SessionEvent) -> Option<ActiveTurn> {
        let ends = match (running.as_ref()?, event) {
            (
                ActiveTurn::Ours(id),
                SessionEvent::Message(Message::Known(KnownMessage::Result(result))),
            ) => result.answers(id),
            (ActiveTurn::Ours(_), SessionEvent::Exited { .. }) => true,
            (
                ActiveTurn::Agents,
                SessionEvent::Message(Message::Known(KnownMessage::Result(_))),
            )
            | (
                ActiveTurn::Agents,
                SessionEvent::Exited { .. } | SessionEvent::Stopped(_) | SessionEvent::Failed(_),
            ) => true,
            _ => false,
        };
        if ends {
            running.take()
        } else {
            None
        }
    }

    /// Whether `event` starts a turn the CLI runs by itself, with none running.
    fn starts(running: &Option<ActiveTurn>, event: &SessionEvent) -> bool {
        running.is_none()
            && matches!(
                event,
                SessionEvent::Message(Message::Known(KnownMessage::System(System::Init { .. })))
            )
    }

    fn is_ours(running: &Option<ActiveTurn>) -> bool {
        matches!(running, Some(ActiveTurn::Ours(_)))
    }
}

/// A pause stopping the agent's background tasks before it closes the CLI's
/// input, so the CLI then exits by itself and the pause is only a pause.
/// A stopped subagent makes the CLI run a notification turn of its own, in
/// which the agent could start the work again, so that turn is interrupted, and
/// the input stays open until it has ended: once the input is closed nothing
/// can interrupt a turn. Past `deadline`, the pause goes ahead, and the worker's
/// grace and forced stop are the fallback.
struct Preparing {
    deadline: tokio::time::Instant,
    /// `stop_task` requests not answered yet.
    stops: usize,
    /// Stopped subagents whose notification turn hasn't ended yet. The CLI
    /// runs one per subagent, even when several are stopped at once and their
    /// turns are interrupted (probed on the pinned CLI); were it ever to
    /// batch them, the pause would only wait out `deadline`.
    agent_turns: usize,
    reply: oneshot::Sender<Result<Value, String>>,
}

impl Preparing {
    fn done(&self, background: &[BackgroundTask]) -> bool {
        self.stops == 0 && self.agent_turns == 0 && background.is_empty()
    }
}

async fn pump(
    space: &mut Space,
    session: &mut Session,
    transcript: &mut Transcript,
    compaction: &mut Compaction,
    events: &mpsc::UnboundedSender<SpaceEvent>,
    mut commands: mpsc::UnboundedReceiver<Command>,
    globally_paused: GlobalPauseCheck,
) -> Result<Option<StopInfo>, String> {
    let (completed, mut completions) = mpsc::unbounded_channel::<Completion>();
    let mut pending = 0usize;
    // The turn running, Devora's or the CLI's own, which a pause waits for.
    let mut turn_running: Option<ActiveTurn> = None;
    // A model the user set this run, and one waiting for the running turn to end before the CLI is told.
    let mut chosen = ChosenModel::default();
    let mut untold: Option<String> = None;
    let (told, mut tellings) = mpsc::unbounded_channel::<(String, Result<Value, String>)>();
    let mut stopping = false;
    let mut pause_after_turn: Option<oneshot::Sender<Result<Value, String>>> = None;
    // The threshold, before any usage arrives.
    let _ = events.send(SpaceEvent::Context(compaction.context()));
    // The model, before any reply: the one the stored conversation continues on, if known.
    let _ = events.send(SpaceEvent::Model(ActiveModel::of(transcript)));
    // The restart notice's turn, until its result delivers it.
    let mut notice_turn: Option<String> = None;
    // The agent's background work, as the CLI last listed it.
    let mut background: Vec<BackgroundTask> = vec![];
    let mut preparing: Option<Preparing> = None;
    let (stop_sent, mut stops) = mpsc::unbounded_channel::<Result<Value, String>>();
    loop {
        if turn_running.is_none() && !stopping && (pause_after_turn.is_some() || globally_paused())
        {
            stopping = true;
            compaction.reset();
            let _ = events.send(SpaceEvent::Context(compaction.context()));
            let reply = pause_after_turn
                .take()
                .unwrap_or_else(|| oneshot::channel().0);
            if background.is_empty() {
                pause(session.handle(), reply);
            } else {
                // Shells first: a stopped subagent starts a turn, which is cut short.
                let mut tasks = background.clone();
                tasks.sort_by_key(|task| task.task_type == "local_agent");
                // Owed before anything stops, so a crash mid-pause still tells the agent.
                space.db.owe_notice(RestartNotice::after_pause(
                    tasks
                        .iter()
                        .map(|task| StoppedTask {
                            task_type: task.task_type.clone(),
                            description: task.description.clone(),
                        })
                        .collect(),
                ))?;
                for task in &tasks {
                    let runtime = session.handle();
                    let sent = stop_sent.clone();
                    let id = task.task_id.clone();
                    tokio::spawn(async move {
                        let _ = sent.send(runtime.stop_task(id).await);
                    });
                }
                preparing = Some(Preparing {
                    deadline: tokio::time::Instant::now() + PREPARE_GRACE,
                    stops: tasks.len(),
                    agent_turns: tasks
                        .iter()
                        .filter(|task| task.task_type == "local_agent")
                        .count(),
                    reply,
                });
            }
        }
        if preparing
            .as_ref()
            .is_some_and(|prepared| prepared.done(&background))
        {
            pause(session.handle(), preparing.take().expect("checked").reply);
        }
        let deadline = preparing.as_ref().map(|prepared| prepared.deadline);
        tokio::select! {
            biased;
            _ = events.closed() => {
                transcript.expect_stop();
                let pause = session.pause().await;
                let mut stopped = None;
                while let Some(event) = session.next_event().await {
                    if let SessionEvent::Stopped(info) = &event { stopped = Some(info.clone()); }
                    persist(space,transcript,&event,events)?;
                }
                while pending > 0 {
                    if let Some(completion) = completions.recv().await {
                        finish(space,transcript,events,completion)?;
                        pending -= 1;
                    }
                }
                return match stopped { Some(info) => Ok(Some(info)), None => pause.map(Some) };
            }
            stopped = stops.recv(), if preparing.is_some() => {
                let prepared = preparing.as_mut().expect("checked");
                prepared.stops -= 1;
                if let Some(Err(error)) = stopped {
                    // A task that won't stop leaves the pause to the worker's grace and forced stop.
                    space.db.owe_notice(RestartNotice::after_stop())?;
                    let _ = events.send(SpaceEvent::Session(SessionEvent::Warning(format!("couldn't stop a background task: {error}"))));
                    pause(session.handle(), preparing.take().expect("checked").reply);
                }
            }
            _ = tokio::time::sleep_until(deadline.unwrap_or_else(tokio::time::Instant::now)), if deadline.is_some() => {
                pause(session.handle(), preparing.take().expect("checked").reply);
            }
            Some((model, outcome)) = tellings.recv() => {
                // Only the latest choice's telling counts; an older one's outcome is moot.
                if chosen.set_to.as_deref() == Some(model.as_str()) {
                    chosen.not_taken = outcome.err();
                    // A refused model never replies: the agent goes on with the one it had.
                    if chosen.not_taken.is_some() {
                        chosen.awaiting_reply = false;
                    }
                    let _ = events.send(SpaceEvent::ChosenModel(chosen.clone()));
                }
            }
            completion = completions.recv(), if pending > 0 => {
                if let Some(completion) = completion {
                    if completion.failed_turn && ActiveTurn::is_ours(&turn_running) {
                        turn_running = None;
                    }
                    finish(space,transcript,events,completion)?;
                    pending -= 1;
                }
            }
            event = session.next_event() => {
                let Some(event) = event else { return Ok(None); };
                persist(space,transcript,&event,events)?;
                if chosen.awaiting_reply && untold.is_none() && is_reply(&event) {
                    chosen.awaiting_reply = false;
                    let _ = events.send(SpaceEvent::ChosenModel(chosen.clone()));
                }
                if let SessionEvent::Message(Message::Known(KnownMessage::System(System::BackgroundTasksChanged { tasks, .. }))) = &event {
                    background = tasks.clone();
                }
                if let Some(prepared) = preparing.as_mut().filter(|prepared| prepared.agent_turns > 0) {
                    match &event {
                        // A stopped subagent's notification turn starting: the agent mustn't act on it.
                        SessionEvent::Message(Message::Known(KnownMessage::System(System::Init { .. }))) => {
                            transcript.expect_stop();
                            let runtime = session.handle();
                            tokio::spawn(async move { let _ = runtime.interrupt().await; });
                        }
                        // Its result names no turn of ours.
                        SessionEvent::Message(Message::Known(KnownMessage::Result(_))) if !ActiveTurn::is_ours(&turn_running) => {
                            prepared.agent_turns -= 1;
                        }
                        _ => {}
                    }
                }
                let terminal = matches!(event,SessionEvent::Stopped(_) | SessionEvent::Failed(_));
                let stop = match &event { SessionEvent::Stopped(info) => Some(info.clone()), _ => None };
                // A crash ends the turn too; recovery resumes without it.
                let finished = ActiveTurn::observe(&mut turn_running, &event);
                if ActiveTurn::starts(&turn_running, &event) {
                    turn_running = Some(ActiveTurn::Agents);
                }
                let ended = matches!(finished, Some(ActiveTurn::Ours(_)));
                let mut step = compact(compaction,&event,finished.as_ref(),events);
                if ended {
                    // Only its own result delivers the notice; a crash leaves the attempt for the next launch to check.
                    if let SessionEvent::Message(Message::Known(KnownMessage::Result(result))) = &event {
                        if let Some(id) = notice_turn.take_if(|id| result.answers(id)) {
                            space.db.notice_delivered(&id)?;
                        }
                    }
                }
                // A spawn is the space's own bookkeeping (the CLI it records), not the host's.
                if !matches!(event, SessionEvent::Spawned(_)) { let _ = events.send(SpaceEvent::Session(event)); }
                if ended { let _ = events.send(SpaceEvent::TurnEnded); }
                if let Some(model) = untold.take_if(|_| finished.is_some()) {
                    tell_model(session.handle(), model, told.clone());
                }
                if finished.is_some() && (pause_after_turn.is_some() || globally_paused()) {
                    // Every pause reason uses the same boundary before compaction.
                    step = None;
                    compaction.reset();
                    let _ = events.send(SpaceEvent::Context(compaction.context()));
                }
                if let Some(step) = step {
                    turn_running = Some(ActiveTurn::Ours(send_step(space,transcript,compaction,&step,session.handle(),completed.clone(),events)?));
                    pending += 1;
                }
                if terminal {
                    if let Some(reply) = pause_after_turn.take() {
                        let _ = reply.send(Ok(Value::Null));
                    }
                    while pending > 0 {
                        if let Some(completion) = completions.recv().await {
                            finish(space,transcript,events,completion)?;
                            pending -= 1;
                        }
                    }
                    return Ok(stop);
                }
            }
            command = commands.recv() => {
                match command {
                    Some(Command { action: Action::PauseAfterTurn, reply }) => {
                        pause_after_turn = Some(reply);
                    }
                    Some(Command { action: Action::GlobalPauseChanged, reply }) => {
                        let _ = reply.send(Ok(Value::Bool(stopping)));
                    }
                    Some(Command { action: Action::ExpectStop, reply }) => {
                        transcript.expect_stop();
                        let _ = reply.send(Ok(Value::Null));
                    }
                    Some(Command { action: Action::SetCompactThreshold(percent), reply }) => {
                        let outcome = space.db.set_compact_threshold(percent).map(|()| {
                            compaction.set_threshold(percent);
                            let _ = events.send(SpaceEvent::Context(compaction.context()));
                            Value::Null
                        });
                        let _ = reply.send(outcome);
                    }
                    Some(Command { action: Action::SetModel(model), reply }) => {
                        let outcome = space.set_model(&model).map(|()| {
                            chosen = ChosenModel { set_to: Some(model.clone()), awaiting_reply: true, not_taken: None };
                            let _ = events.send(SpaceEvent::ChosenModel(chosen.clone()));
                            if turn_running.is_some() {
                                untold = Some(model);
                            } else {
                                untold = None;
                                tell_model(session.handle(), model, told.clone());
                            }
                            Value::Null
                        });
                        let _ = reply.send(outcome);
                    }
                    Some(Command { action: Action::CompactNow, reply }) if globally_paused() || stopping || pause_after_turn.is_some() => {
                        let _ = reply.send(Err("This agent is pausing. It compacts once it resumes and you ask again.".into()));
                    }
                    Some(Command { action: Action::CompactNow, reply }) if turn_running.is_some() => {
                        let outcome = if compaction.request() {
                            let _ = events.send(SpaceEvent::Context(compaction.context()));
                            Ok(Value::Null)
                        } else {
                            Err("A compaction is already under way.".into())
                        };
                        let _ = reply.send(outcome);
                    }
                    Some(Command { action: Action::CompactNow, reply }) => {
                        match compaction.begin() {
                            Some(step) => {
                                let sent = send_step(space,transcript,compaction,&step,session.handle(),completed.clone(),events);
                                let _ = reply.send(sent.as_ref().map(|_| Value::Null).map_err(Clone::clone));
                                turn_running = Some(ActiveTurn::Ours(sent?));
                                pending += 1;
                            }
                            None => { let _ = reply.send(Err("A compaction is already under way.".into())); }
                        }
                    }
                    Some(command) if matches!(command.action, Action::Turn { .. } | Action::RestartNotice { .. }) && (globally_paused() || stopping || pause_after_turn.is_some()) => {
                        let _ = command.reply.send(Err("This agent is pausing. It starts no new turn until it resumes.".into()));
                    }
                    // Its failed send would otherwise end the running turn. One sent during the CLI's own turn joins it.
                    Some(command) if matches!(command.action, Action::Turn { .. } | Action::RestartNotice { .. }) && ActiveTurn::is_ours(&turn_running) => {
                        let _ = command.reply.send(Err("a turn is already running".into()));
                    }
                    Some(command) => {
                        match &command.action {
                            Action::Turn { id, .. } => turn_running = Some(ActiveTurn::Ours(id.clone())),
                            Action::RestartNotice { id, .. } => {
                                turn_running = Some(ActiveTurn::Ours(id.clone()));
                                notice_turn = Some(id.clone());
                            }
                            _ => {}
                        }
                        let notice = matches!(command.action, Action::RestartNotice { .. });
                        begin(space,transcript,command,session.handle(),completed.clone(),events)?;
                        pending += 1;
                        if notice { let _ = events.send(SpaceEvent::TurnStarted); }
                    }
                    None => {}
                }
            }
        }
    }
}

/// Announces a compaction step in the transcript and sends its turn, publishing
/// the context, whose progress it changed; returns the turn's ID.
fn send_step(
    space: &mut Space,
    transcript: &mut Transcript,
    compaction: &Compaction,
    step: &crate::compaction::Step,
    runtime: SessionHandle,
    completions: mpsc::UnboundedSender<Completion>,
    events: &mpsc::UnboundedSender<SpaceEvent>,
) -> Result<String, String> {
    let mut announcement = transcript.system(
        SystemKind::Compaction,
        step.announcement(&compaction.context()),
        json!({}),
    );
    space
        .db
        .save_changes(None, std::slice::from_mut(&mut announcement))?;
    let _ = events.send(SpaceEvent::Transcript(announcement));
    let _ = events.send(SpaceEvent::Context(compaction.context()));
    // Nobody waits for the outcome; it is recorded like any turn's.
    let (reply, _) = oneshot::channel();
    let id = uuid::Uuid::new_v4().to_string();
    begin(
        space,
        transcript,
        Command {
            action: Action::Turn {
                id: id.clone(),
                text: step.prompt(),
            },
            reply,
        },
        runtime,
        completions,
        events,
    )?;
    let _ = events.send(SpaceEvent::TurnStarted);
    Ok(id)
}

/// Pauses without waiting in the pump, which goes on draining the session's
/// events until it stops; the caller hears once it has.
fn pause(runtime: SessionHandle, reply: oneshot::Sender<Result<Value, String>>) {
    tokio::spawn(async move {
        let _ = reply.send(runtime.pause().await.map(|_| Value::Null));
    });
}

/// Feeds compaction the session's usage, the results ending our turns and
/// boundaries, publishing the context as it changes; returns the turn
/// compaction asks for, if any.
/// Moves the compaction on from `event`, which ended `ended`, if it ended a turn; returns the step to send next.
fn compact(
    compaction: &mut Compaction,
    event: &SessionEvent,
    ended: Option<&ActiveTurn>,
    events: &mpsc::UnboundedSender<SpaceEvent>,
) -> Option<crate::compaction::Step> {
    let before = compaction.context();
    let step = match event {
        SessionEvent::Usage {
            source: UsageSource::Assistant | UsageSource::MessageStart,
            usage,
        } => {
            compaction.usage(usage);
            None
        }
        SessionEvent::Message(Message::Known(KnownMessage::Result(result))) => match ended {
            Some(ActiveTurn::Ours(_)) => compaction.result(result),
            // A turn the agent started itself only starts a compaction the user asked for during it. The point where a
            // threshold check for such turns belongs: without one, an agent that works from its background tasks
            // never reaches a point where Devora compacts it.
            Some(ActiveTurn::Agents) => compaction.agents_turn_ended(result),
            None => None,
        },
        SessionEvent::Compacted(metadata) => {
            compaction.compacted(metadata);
            None
        }
        SessionEvent::Exited { .. } | SessionEvent::Stopped(_) | SessionEvent::Failed(_) => {
            compaction.reset();
            None
        }
        _ => None,
    };
    if compaction.context() != before {
        let _ = events.send(SpaceEvent::Context(compaction.context()));
    }
    step
}

fn finish(
    space: &mut Space,
    transcript: &mut Transcript,
    events: &mpsc::UnboundedSender<SpaceEvent>,
    completion: Completion,
) -> Result<(), String> {
    if let Some(mut change) = transcript.settle(
        completion.sequence,
        completion.result.as_ref().err().cloned(),
    ) {
        if let Err(error) = space
            .db
            .save_changes(None, std::slice::from_mut(&mut change))
        {
            let _ = completion.reply.send(Err(error.clone()));
            return Err(error);
        }
        let _ = events.send(SpaceEvent::Transcript(change));
        transcript.release_completed();
    }
    let _ = completion.reply.send(completion.result);
    Ok(())
}

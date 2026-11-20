use crate::compaction::{Compaction, Context};
use crate::restart_notice::{AppRun, RestartNotice, StoppedTask};
use crate::Space;
use devora_agent::protocol::{BackgroundTask, KnownMessage, Message, System};
use devora_agent::session::stop_tree;
use devora_agent::session::{
    saved_turn, PermissionAnswer, Resume, Session, SessionEvent, SessionHandle, SessionOptions,
    StopInfo, UsageSource,
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
    /// A turn the space sent itself: compaction's, or the restart notice.
    TurnStarted,
    /// The CLI answered the running turn, or exited during it. Results the CLI
    /// produces by itself (background work finishing, a resume) end nothing.
    TurnEnded,
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
    /// The agent's own threshold, or none to follow the profile's.
    SetCompactThreshold(Option<u8>),
    CompactNow,
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
    pub async fn set_model(&self, model: String) -> Result<Value, String> {
        self.runtime.set_model(model).await
    }
    pub async fn set_permission_mode(&self, mode: String) -> Result<Value, String> {
        self.runtime.set_permission_mode(mode).await
    }
    pub async fn answer_mcp(&self, request_id: String, response: Value) -> Result<(), String> {
        self.runtime.answer_mcp(request_id, response).await
    }
    pub async fn pause(&self) -> Result<StopInfo, String> {
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
    worker: Option<JoinHandle<Result<(), String>>>,
}
impl RunningSpace {
    pub fn handle(&self) -> SpaceHandle {
        self.handle.clone()
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
    pub async fn pause(&mut self) -> Result<StopInfo, String> {
        let stopped = self.handle.pause().await;
        if let Some(worker) = self.worker.take() {
            worker
                .await
                .map_err(|e| format!("agent space worker failed: {e}"))??;
        }
        stopped
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
    let launch = options.clone();
    let launch_log = log.clone();
    let (mut space, mut transcript, history, notice) = tokio::task::spawn_blocking(move || {
        let mut transcript = space.db.fold(&space.resume.session_id)?;
        let interrupted = transcript.interrupt(!space.resume.has_conversation);
        space.db.save_changes(None, &interrupted)?;
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
            space.db.save_changes(None, &[change])?;
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
    changes.extend(transcript.event(event));
    space.db.save_changes(resume.as_ref(), &changes)?;
    transcript.release_completed();
    if let Some(resume) = resume {
        space.resume = resume;
    }
    for change in changes {
        let _ = events.send(SpaceEvent::Transcript(change));
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
    let change = match &command.action {
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
        | Action::SetCompactThreshold(_)
        | Action::CompactNow => return Err("the pump handles this action itself".into()),
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
            &change,
            &RestartNotice {
                attempt: Some(id.clone()),
                ..notice.clone()
            },
        ),
        _ => space.db.save_changes(None, std::slice::from_ref(&change)),
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
            | Action::SetCompactThreshold(_)
            | Action::CompactNow => unreachable!("handled by the pump"),
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
) -> Result<(), String> {
    let (completed, mut completions) = mpsc::unbounded_channel::<Completion>();
    let mut pending = 0usize;
    // The turn running, by the ID the CLI answers it with, and a pause waiting for it to end.
    let mut turn_running: Option<String> = None;
    let mut stopping = false;
    let mut pause_after_turn: Option<oneshot::Sender<Result<Value, String>>> = None;
    // The threshold, before any usage arrives.
    let _ = events.send(SpaceEvent::Context(compaction.context()));
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
                let pause = session.pause().await;
                let mut stopped = false;
                while let Some(event) = session.next_event().await {
                    stopped |= matches!(event,SessionEvent::Stopped(_));
                    persist(space,transcript,&event,events)?;
                }
                while pending > 0 {
                    if let Some(completion) = completions.recv().await {
                        finish(space,transcript,events,completion)?;
                        pending -= 1;
                    }
                }
                return if stopped { Ok(()) } else { pause.map(|_|()) };
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
            completion = completions.recv(), if pending > 0 => {
                if let Some(completion) = completion {
                    if completion.failed_turn {
                        turn_running = None;
                    }
                    finish(space,transcript,events,completion)?;
                    pending -= 1;
                }
            }
            event = session.next_event() => {
                let Some(event) = event else { return Ok(()); };
                persist(space,transcript,&event,events)?;
                if let SessionEvent::Message(Message::Known(KnownMessage::System(System::BackgroundTasksChanged { tasks, .. }))) = &event {
                    background = tasks.clone();
                }
                if let Some(prepared) = preparing.as_mut().filter(|prepared| prepared.agent_turns > 0) {
                    match &event {
                        // A stopped subagent's notification turn starting: the agent mustn't act on it.
                        SessionEvent::Message(Message::Known(KnownMessage::System(System::Init { .. }))) => {
                            let runtime = session.handle();
                            tokio::spawn(async move { let _ = runtime.interrupt().await; });
                        }
                        // Its result names no turn of ours.
                        SessionEvent::Message(Message::Known(KnownMessage::Result(_))) if turn_running.is_none() => {
                            prepared.agent_turns -= 1;
                        }
                        _ => {}
                    }
                }
                let terminal = matches!(event,SessionEvent::Stopped(_) | SessionEvent::Failed(_));
                // A crash ends the turn too; recovery resumes without it.
                let ended = turn_running.as_deref().is_some_and(|id| match &event {
                    SessionEvent::Message(Message::Known(KnownMessage::Result(result))) => result.answers(id),
                    SessionEvent::Exited { .. } => true,
                    _ => false,
                });
                let mut step = compact(compaction,&event,ended,events);
                if ended {
                    // Only its own result delivers the notice; a crash leaves the attempt for the next launch to check.
                    if let SessionEvent::Message(Message::Known(KnownMessage::Result(result))) = &event {
                        if let Some(id) = notice_turn.take_if(|id| result.answers(id)) {
                            space.db.notice_delivered(&id)?;
                        }
                    }
                    turn_running = None;
                }
                // A spawn is the space's own bookkeeping (the CLI it records), not the host's.
                if !matches!(event, SessionEvent::Spawned(_)) { let _ = events.send(SpaceEvent::Session(event)); }
                if ended { let _ = events.send(SpaceEvent::TurnEnded); }
                if ended && (pause_after_turn.is_some() || globally_paused()) {
                    // Every pause reason uses the same boundary before compaction.
                    step = None;
                    compaction.reset();
                    let _ = events.send(SpaceEvent::Context(compaction.context()));
                }
                if let Some(step) = step {
                    turn_running = Some(send_step(space,transcript,compaction,&step,session.handle(),completed.clone(),events)?);
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
                    return Ok(());
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
                    Some(Command { action: Action::SetCompactThreshold(percent), reply }) => {
                        let outcome = space.db.set_compact_threshold(percent).map(|()| {
                            compaction.set_threshold(percent);
                            let _ = events.send(SpaceEvent::Context(compaction.context()));
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
                                turn_running = Some(sent?);
                                pending += 1;
                            }
                            None => { let _ = reply.send(Err("A compaction is already under way.".into())); }
                        }
                    }
                    Some(command) if matches!(command.action, Action::Turn { .. } | Action::RestartNotice { .. }) && (globally_paused() || stopping || pause_after_turn.is_some()) => {
                        let _ = command.reply.send(Err("This agent is pausing. It starts no new turn until it resumes.".into()));
                    }
                    // Its failed send would otherwise end the running turn.
                    Some(command) if matches!(command.action, Action::Turn { .. } | Action::RestartNotice { .. }) && turn_running.is_some() => {
                        let _ = command.reply.send(Err("a turn is already running".into()));
                    }
                    Some(command) => {
                        match &command.action {
                            Action::Turn { id, .. } => turn_running = Some(id.clone()),
                            Action::RestartNotice { id, .. } => {
                                turn_running = Some(id.clone());
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
    let announcement = transcript.system(
        SystemKind::Compaction,
        step.announcement(&compaction.context()),
        json!({}),
    );
    space
        .db
        .save_changes(None, std::slice::from_ref(&announcement))?;
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
fn compact(
    compaction: &mut Compaction,
    event: &SessionEvent,
    ended_our_turn: bool,
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
        SessionEvent::Message(Message::Known(KnownMessage::Result(result))) if ended_our_turn => {
            compaction.result(result)
        }
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
    if let Some(change) = transcript.settle(
        completion.sequence,
        completion.result.as_ref().err().cloned(),
    ) {
        if let Err(error) = space.db.save_changes(None, std::slice::from_ref(&change)) {
            let _ = completion.reply.send(Err(error.clone()));
            return Err(error);
        }
        let _ = events.send(SpaceEvent::Transcript(change));
        transcript.release_completed();
    }
    let _ = completion.reply.send(completion.result);
    Ok(())
}

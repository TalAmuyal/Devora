use crate::Space;
use devora_agent::protocol::{KnownMessage, Message};
use devora_agent::session::{
    PermissionAnswer, Resume, Session, SessionEvent, SessionHandle, SessionOptions, StopInfo,
};
use devora_agent::transcript::{SystemKind, Transcript, TranscriptChange, TranscriptItem};
use devora_app::log::Log;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

#[derive(Debug, Clone)]
pub enum SpaceEvent {
    Restored(Vec<TranscriptItem>),
    Transcript(TranscriptChange),
    Session(SessionEvent),
}

enum Action {
    Turn(String),
    Interrupt,
    Permission(String, PermissionAnswer),
}
struct Command {
    action: Action,
    reply: oneshot::Sender<Result<Value, String>>,
}
struct Completion {
    sequence: u64,
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
    pub async fn send_turn(&self, text: String) -> Result<(), String> {
        self.call(Action::Turn(text)).await.map(|_| ())
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
    log: Arc<Log>,
) -> Result<RunningSpace, String> {
    let (mut space, mut transcript) = tokio::task::spawn_blocking(move || {
        let mut transcript =
            Transcript::new(space.db.all_history()?, space.resume.session_id.clone());
        let interrupted = transcript.interrupt(!space.resume.has_conversation);
        space.db.save_changes(None, &interrupted)?;
        Ok::<_, String>((space, transcript))
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
    let _ = events.send(SpaceEvent::Restored(transcript.items().to_vec()));
    let (commands, received_commands) = mpsc::unbounded_channel();
    let handle = SpaceHandle {
        runtime: session.handle(),
        commands,
    };
    let worker = tokio::spawn(async move {
        let result = pump(
            &mut space,
            &mut session,
            &mut transcript,
            &events,
            received_commands,
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
        Action::Turn(text) => transcript.submit(text.clone()),
        Action::Interrupt => transcript.intention(
            SystemKind::Interruption,
            "Interrupt requested".into(),
            json!({}),
        ),
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
    if let Err(error) = space.db.save_changes(None, std::slice::from_ref(&change)) {
        let _ = command.reply.send(Err(error.clone()));
        return Err(error);
    }
    let _ = events.send(SpaceEvent::Transcript(change));
    tokio::spawn(async move {
        let result = match command.action {
            Action::Turn(text) => runtime.send_turn(text).await.map(|_| Value::Null),
            Action::Interrupt => runtime.interrupt().await,
            Action::Permission(id, answer) => runtime
                .answer_permission(id, answer)
                .await
                .map(|_| Value::Null),
        };
        let _ = completions.send(Completion {
            sequence,
            result,
            reply: command.reply,
        });
    });
    Ok(())
}

async fn pump(
    space: &mut Space,
    session: &mut Session,
    transcript: &mut Transcript,
    events: &mpsc::UnboundedSender<SpaceEvent>,
    mut commands: mpsc::UnboundedReceiver<Command>,
) -> Result<(), String> {
    let (completed, mut completions) = mpsc::unbounded_channel::<Completion>();
    let mut pending = 0usize;
    loop {
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
            completion = completions.recv(), if pending > 0 => {
                if let Some(completion) = completion { finish(space,transcript,events,completion)?; pending -= 1; }
            }
            event = session.next_event() => {
                let Some(event) = event else { return Ok(()); };
                persist(space,transcript,&event,events)?;
                let terminal = matches!(event,SessionEvent::Stopped(_) | SessionEvent::Failed(_));
                let _ = events.send(SpaceEvent::Session(event));
                if terminal {
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
                if let Some(command) = command {
                    begin(space,transcript,command,session.handle(),completed.clone(),events)?;
                    pending += 1;
                }
            }
        }
    }
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
    }
    let _ = completion.reply.send(completion.result);
    Ok(())
}

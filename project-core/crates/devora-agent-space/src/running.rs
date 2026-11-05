use crate::Space;
use devora_agent::protocol::{KnownMessage, Message};
use devora_agent::session::{
    Resume, Session, SessionEvent, SessionHandle, SessionOptions, StopInfo,
};
use devora_app::log::Log;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

/// A live space owner. Its task holds the OS lock until the CLI is reaped and
/// persists the ID/flag on Ready, Result and Stopped before publishing the event.
/// Dropping the event receiver pauses the session through the same path.
pub struct RunningSpace {
    handle: SessionHandle,
    events: mpsc::UnboundedReceiver<SessionEvent>,
    worker: Option<JoinHandle<Result<(), String>>>,
}

impl RunningSpace {
    pub fn handle(&self) -> SessionHandle {
        self.handle.clone()
    }
    pub async fn next_event(&mut self) -> Option<SessionEvent> {
        self.events.recv().await
    }
    /// Returns only after the space database has recorded StopInfo and its owner
    /// has released the lock. Final events remain drainable.
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
    let mut session = Session::start(options, log.clone()).await?;
    let handle = session.handle();
    let (events, received) = mpsc::unbounded_channel();
    let worker = tokio::spawn(async move {
        let result = pump(&mut space, &mut session, &events).await;
        if let Err(error) = &result {
            log.report_error(error);
            // Stop the process before relinquishing this space's exclusive owner.
            if let Ok(info) = session.pause().await {
                let _ = persist(&mut space, &SessionEvent::Stopped(info));
            }
            let _ = events.send(SessionEvent::Failed(error.clone()));
        }
        result
    });
    Ok(RunningSpace {
        handle,
        events: received,
        worker: Some(worker),
    })
}

fn persist(space: &mut Space, event: &SessionEvent) -> Result<(), String> {
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
    if let Some(resume) = resume {
        space.db.save_session(&resume)?;
        space.resume = resume;
    }
    Ok(())
}

async fn pump(
    space: &mut Space,
    session: &mut Session,
    events: &mpsc::UnboundedSender<SessionEvent>,
) -> Result<(), String> {
    loop {
        tokio::select! {
            biased;
            _ = events.closed() => {
                let pause = session.pause().await;
                let mut stopped = false;
                // A caller may have already sent pause and then dropped its
                // future. Drain its final result/StopInfo even if our pause
                // command reports that shutdown was already in progress.
                while let Some(event) = session.next_event().await {
                    stopped |= matches!(event,SessionEvent::Stopped(_));
                    persist(space,&event)?;
                }
                return if stopped { Ok(()) } else { pause.map(|_|()) };
            }
            event = session.next_event() => {
                let Some(event) = event else { return Ok(()); };
                persist(space,&event)?;
                let terminal = matches!(event,SessionEvent::Stopped(_) | SessionEvent::Failed(_));
                let _ = events.send(event);
                if terminal { return Ok(()); }
            }
        }
    }
}

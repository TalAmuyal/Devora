use super::*;
use devora_app::log::Level;
use devora_state::{feed, Subscription};
use devora_workspace::creation::{CreationHandle, EventSink};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CreationEvent {
    Step { label: String },
    Log { line: String },
    Done { name: String, path: String },
    Failed { message: String },
    Cancelled,
}

pub struct Creation {
    pub id: u64,
    pub events: Subscription<CreationEvent>,
}

pub(super) struct Operation {
    pub cancel: CreationHandle,
    pub done: watch::Sender<bool>,
}

impl Agents {
    pub fn create(
        &self,
        profile: PathBuf,
        form: CreationForm,
        launch_inputs: BTreeMap<String, String>,
        global_config: PathBuf,
    ) -> Result<Creation, String> {
        let (feed, events) = feed();
        let feed = Arc::new(feed);
        let handle = CreationHandle::new();
        let (done, _) = watch::channel(false);
        let id = {
            let mut inner = lock(&self.inner);
            if inner.closing {
                return Err("agents are shutting down".into());
            }
            inner.generation += 1;
            let id = inner.generation;
            inner.operations.insert(
                id,
                Operation {
                    cancel: handle.clone(),
                    done: done.clone(),
                },
            );
            id
        };
        let inner = self.inner.clone();
        let log = self.log.clone();
        self.runtime.spawn_blocking(move || {
            let progress = feed.clone();
            let sink: EventSink = Arc::new(move |event| {
                use devora_workspace::creation::CreationEvent as Progress;
                let event = match event {
                    Progress::Step { label } => CreationEvent::Step { label },
                    Progress::Log { line } => CreationEvent::Log { line },
                    _ => return,
                };
                progress.push(&[event]);
            });
            let mut warnings = vec![];
            let result = Space::create(
                &profile,
                form,
                launch_inputs,
                &global_config,
                &handle,
                &sink,
                &mut warnings,
            );
            for warning in warnings {
                log.write(Level::Warn, &warning);
            }
            let last = match result {
                Ok(space) => {
                    let name = space.identity().name.clone();
                    let path = space.path().to_string_lossy().into_owned();
                    drop(space);
                    CreationEvent::Done { name, path }
                }
                Err(message) => {
                    if handle.is_cancelled() && !message.contains("cleanup failed") {
                        CreationEvent::Cancelled
                    } else {
                        log.report_error(&message);
                        CreationEvent::Failed { message }
                    }
                }
            };
            lock(&inner).operations.remove(&id);
            done.send_replace(true);
            feed.push(&[last]);
        });
        Ok(Creation { id, events })
    }

    pub fn cancel_creation(&self, id: u64) {
        if let Some(operation) = lock(&self.inner).operations.get(&id) {
            operation.cancel.request_cancel();
        }
    }
}

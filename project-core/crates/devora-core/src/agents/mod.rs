//! Agent Space ownership independent of UI subscriptions. Closing a tab removes
//! it immediately and signals pause. Its entry remains reserved until the process
//! is reaped and C's StopInfo persistence completes. Reopen waits for that barrier.
mod events;
mod launch;
mod operations;

use devora_agent::session::SessionEvent;
pub use devora_agent::session::{PermissionAnswer, Plugin};
use devora_agent_space::Space;
pub use devora_agent_space::{CreationForm, Entry as SpaceEntry, Field, LaunchContext, Source};
use devora_agent_space::{SpaceEvent, SpaceHandle};
use devora_app::log::Log;
pub use devora_config::agent::Scope;
use devora_state::{ChatCommand, ChatPhase, ChatUpdate, Command, Event, SessionKind, Store};
pub use operations::{Creation, CreationEvent};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use tokio::runtime::Handle;
use tokio::sync::{oneshot, watch};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Key {
    profile: PathBuf,
    name: String,
}

impl Key {
    fn new(profile: &Path, name: &str) -> Result<Self, String> {
        devora_agent_space::validate_name(name)?;
        Ok(Self {
            profile: std::fs::canonicalize(profile).map_err(|e| e.to_string())?,
            name: name.into(),
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Pause {
    None,
    AfterTurn,
    Immediate,
}

struct Owner {
    run_id: u64,
    tab: Option<u64>,
    generation: u64,
    handle: Option<SpaceHandle>,
    pause: watch::Sender<Pause>,
    done: watch::Sender<bool>,
}

struct Inner {
    owners: BTreeMap<Key, Owner>,
    generation: u64,
    closing: bool,
    operations: BTreeMap<u64, operations::Operation>,
    deletions: BTreeMap<Key, watch::Sender<bool>>,
}

/// Each facade has one event-loop thread. UI async exports need no Tokio context.
/// Explicit shutdown awaits all pauses. Drop also joins the same shutdown path.
pub struct Agents {
    inner: Arc<Mutex<Inner>>,
    state: Arc<Store>,
    log: Arc<Log>,
    runtime: Handle,
    stop: Option<oneshot::Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

fn lock<T>(value: &Mutex<T>) -> MutexGuard<'_, T> {
    value.lock().unwrap_or_else(|e| e.into_inner())
}

impl Agents {
    pub fn new(state: Arc<Store>, log: Arc<Log>) -> Result<Self, String> {
        let inner = Arc::new(Mutex::new(Inner {
            owners: BTreeMap::new(),
            generation: 0,
            closing: false,
            operations: BTreeMap::new(),
            deletions: BTreeMap::new(),
        }));
        let (stop, stopped) = oneshot::channel();
        let (ready, receive) = std::sync::mpsc::sync_channel(1);
        let shutdown_inner = inner.clone();
        let shutdown_state = state.clone();
        let thread = std::thread::Builder::new()
            .name("devora-agents".into())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build();
                match runtime {
                    Ok(runtime) => {
                        if ready.send(Ok(runtime.handle().clone())).is_err() {
                            return;
                        }
                        runtime.block_on(async {
                            let _ = stopped.await;
                            shutdown(&shutdown_inner, &shutdown_state).await;
                        });
                    }
                    Err(error) => {
                        let _ = ready.send(Err(error.to_string()));
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        let runtime = receive.recv().map_err(|e| e.to_string())??;
        Ok(Self {
            inner,
            state,
            log,
            runtime,
            stop: Some(stop),
            thread: Some(thread),
        })
    }

    /// Starts opening immediately. Ready/failure is delivered through the Store.
    /// A request during pause waits for ownership release, then opens again.
    pub async fn open(
        &self,
        profile: &Path,
        name: &str,
        context: LaunchContext,
    ) -> Result<u64, String> {
        let key = Key::new(profile, name)?;
        loop {
            let mut done = {
                let mut inner = lock(&self.inner);
                if inner.closing {
                    return Err("agents are shutting down".into());
                }
                if inner.deletions.contains_key(&key) {
                    return Err("agent space is being deleted".into());
                }
                let waiting = inner.owners.get(&key).filter(|owner| !*owner.done.borrow());
                if let Some(owner) = waiting {
                    if *owner.pause.borrow() == Pause::None {
                        let tab = owner.tab.ok_or("agent has no tab")?;
                        self.state
                            .dispatch(Command::ActivateTab { tab })
                            .map_err(|e| e.to_string())?;
                        return Ok(tab);
                    }
                    owner.done.subscribe()
                } else {
                    let tab = self.open_tab(&inner, &key)?;
                    inner.generation += 1;
                    let generation = inner.generation;
                    let (pause, paused) = watch::channel(Pause::None);
                    let (done, _) = watch::channel(false);
                    inner.owners.insert(
                        key.clone(),
                        Owner {
                            run_id: generation,
                            tab: Some(tab),
                            generation,
                            handle: None,
                            pause,
                            done,
                        },
                    );
                    self.state
                        .dispatch_chat(ChatCommand::Start { tab, generation })
                        .map_err(|e| e.to_string())?;
                    let worker_inner = self.inner.clone();
                    let state = self.state.clone();
                    let log = self.log.clone();
                    self.runtime.spawn(async move {
                        run(worker_inner, state, log, key, generation, paused, context).await;
                    });
                    return Ok(tab);
                }
            };
            done.wait_for(|done| *done)
                .await
                .map_err(|e| e.to_string())?;
        }
    }

    pub async fn restore(&self, context: LaunchContext) -> Result<(), String> {
        let snapshot = self.state.snapshot();
        for tab in &snapshot.window.tabs {
            if let SessionKind::AgentSpace { name } = &tab.session.kind {
                let result = match tab.session.profile_path.as_deref() {
                    Some(profile) => self
                        .open(Path::new(profile), name, context.clone())
                        .await
                        .map(|_| ()),
                    None => Err("agent tab has no owning profile".into()),
                };
                if let Err(error) = result {
                    let mut inner = lock(&self.inner);
                    inner.generation += 1;
                    let generation = inner.generation;
                    self.state
                        .dispatch_chat(ChatCommand::Start {
                            tab: tab.id,
                            generation,
                        })
                        .map_err(|e| e.to_string())?;
                    events::update(
                        &self.state,
                        tab.id,
                        generation,
                        ChatUpdate::Failure {
                            message: error.clone(),
                        },
                    );
                    events::update(
                        &self.state,
                        tab.id,
                        generation,
                        ChatUpdate::Phase {
                            phase: ChatPhase::Failed,
                        },
                    );
                    self.log.report_error(&error);
                }
            }
        }
        if let Some(tab) = snapshot.window.active_tab {
            self.state
                .dispatch(Command::ActivateTab { tab })
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    fn open_tab(&self, inner: &Inner, key: &Key) -> Result<u64, String> {
        let existing = inner.owners.get(key).and_then(|o| o.tab).or_else(|| {
            self.state.snapshot().window.tabs.iter().find(|tab| {
                tab.session.profile_path.as_deref() == key.profile.to_str()
                    && matches!(&tab.session.kind, SessionKind::AgentSpace { name } if *name == key.name)
            }).map(|tab| tab.id)
        });
        if let Some(tab) = existing {
            self.state
                .dispatch(Command::ActivateTab { tab })
                .map_err(|e| e.to_string())?;
            return Ok(tab);
        }
        self.state
            .dispatch(Command::OpenSession {
                title: key.name.clone(),
                kind: SessionKind::AgentSpace {
                    name: key.name.clone(),
                },
                profile_path: Some(
                    key.profile
                        .to_str()
                        .ok_or("profile path is not UTF-8")?
                        .into(),
                ),
            })
            .map_err(|e| e.to_string())?
            .into_iter()
            .find_map(|event| match event {
                Event::SessionOpened { tab, .. } => Some(tab.id),
                _ => None,
            })
            .ok_or_else(|| "agent tab was not opened".into())
    }

    /// Handles generic tab closure as well as the explicit Agent Space export.
    pub fn close_tab(&self, tab: u64) -> Result<(), String> {
        let mut inner = lock(&self.inner);
        self.state
            .dispatch(Command::CloseSession { tab })
            .map_err(|e| e.to_string())?;
        self.state
            .dispatch_chat(ChatCommand::Close { tab })
            .map_err(|e| e.to_string())?;
        if let Some(owner) = inner.owners.values_mut().find(|o| o.tab == Some(tab)) {
            owner.tab = None;
            owner.pause.send_replace(Pause::Immediate);
        }
        Ok(())
    }

    pub async fn pause(&self, tab: u64) -> Result<(), String> {
        let mut done = {
            let inner = lock(&self.inner);
            let owner = inner
                .owners
                .values()
                .find(|o| o.tab == Some(tab))
                .ok_or("agent is not open")?;
            if *owner.done.borrow() {
                return Ok(());
            }
            phase(&self.state, owner, ChatPhase::Pausing);
            owner.pause.send_replace(Pause::Immediate);
            owner.done.subscribe()
        };
        done.wait_for(|done| *done)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Reserve a pause without interrupting the active turn. Permission answers
    /// and Stop remain available, while further sends are rejected immediately.
    pub fn pause_after_turn(&self, tab: u64, generation: u64) -> Result<(), String> {
        let inner = lock(&self.inner);
        let owner = inner
            .owners
            .values()
            .find(|o| o.tab == Some(tab))
            .ok_or("agent is not open")?;
        if inner.closing
            || owner.generation != generation
            || *owner.pause.borrow() == Pause::Immediate
            || *owner.done.borrow()
        {
            return Err("agent generation is stale or pausing".into());
        }
        events::update(&self.state, tab, generation, ChatUpdate::PauseRequested);
        owner.pause.send_replace(Pause::AfterTurn);
        Ok(())
    }

    pub async fn shutdown(&self) {
        shutdown(&self.inner, &self.state).await;
    }

    pub async fn history(
        &self,
        profile: &Path,
        name: &str,
        after: u64,
        limit: u32,
    ) -> Result<Vec<devora_agent::transcript::TranscriptItem>, String> {
        let profile = profile.to_path_buf();
        let name = name.to_owned();
        let (sent, received) = oneshot::channel();
        self.runtime.spawn_blocking(move || {
            let _ = sent.send(Space::read_history(&profile, &name, after, limit));
        });
        received.await.map_err(|e| e.to_string())?
    }

    pub async fn history_before(
        &self,
        profile: &Path,
        name: &str,
        before: Option<u64>,
        limit: u32,
    ) -> Result<Vec<devora_agent::transcript::TranscriptItem>, String> {
        let profile = profile.to_path_buf();
        let name = name.to_owned();
        let (sent, received) = oneshot::channel();
        self.runtime.spawn_blocking(move || {
            let _ = sent.send(Space::history_before(&profile, &name, before, limit));
        });
        received.await.map_err(|e| e.to_string())?
    }

    pub fn list(&self, profile: &Path) -> Result<Vec<SpaceEntry>, String> {
        Space::list(profile)
    }

    /// The launch context from the user's login shell and the app bundle's
    /// `Resources` directory, for hosts that supply nothing of their own.
    pub async fn resolve_launch(
        &self,
        global_config: PathBuf,
        resources: Option<PathBuf>,
    ) -> Result<LaunchContext, String> {
        self.runtime
            .spawn(launch::resolve(global_config, resources))
            .await
            .map_err(|e| e.to_string())?
    }

    /// The template's fields for the creation form, as `form` would render them.
    pub fn creation_fields(
        &self,
        profile: &Path,
        global_config: &Path,
        launch_inputs: BTreeMap<String, String>,
        form: &CreationForm,
    ) -> Result<Vec<Field>, String> {
        devora_agent_space::fields(profile, global_config, launch_inputs, form)
    }

    /// Saves a creation-form value to the global or profile config, so later
    /// agents start from it.
    pub fn save_field(
        &self,
        profile: &Path,
        global_config: &Path,
        scope: Scope,
        key: &str,
        value: &str,
    ) -> Result<(), String> {
        let file = match scope {
            Scope::Global => global_config.to_path_buf(),
            Scope::Profile => profile.join("config.json"),
        };
        devora_config::agent::write(&file, key, value)
    }

    pub async fn delete(&self, profile: &Path, name: &str) -> Result<(), String> {
        let key = Key::new(profile, name)?;
        let (done, _) = watch::channel(false);
        {
            let mut inner = lock(&self.inner);
            if inner.closing {
                return Err("agents are shutting down".into());
            }
            if inner.deletions.contains_key(&key)
                || inner
                    .owners
                    .get(&key)
                    .is_some_and(|o| !*o.done.borrow() || o.tab.is_some())
            {
                return Err("close and pause the agent before deleting its space".into());
            }
            inner.deletions.insert(key.clone(), done.clone());
        }
        let inner = self.inner.clone();
        let state = self.state.clone();
        let (reply, received) = oneshot::channel();
        self.runtime.spawn_blocking(move || {
            let result = Space::delete(&key.profile, &key.name).and_then(|()| {
                state
                    .dispatch(Command::ClearAgentView {
                        profile_path: key.profile.to_string_lossy().into_owned(),
                        name: key.name.clone(),
                    })
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            });
            {
                let mut inner = lock(&inner);
                inner.deletions.remove(&key);
                if result.is_ok() {
                    inner.owners.remove(&key);
                }
            }
            done.send_replace(true);
            let _ = reply.send(result);
        });
        received.await.map_err(|e| e.to_string())?
    }

    fn handle(&self, tab: u64, generation: u64) -> Result<SpaceHandle, String> {
        let inner = lock(&self.inner);
        let owner = inner
            .owners
            .values()
            .find(|o| o.tab == Some(tab))
            .ok_or("agent is not open")?;
        if inner.closing
            || owner.generation != generation
            || *owner.pause.borrow() == Pause::Immediate
        {
            return Err("agent generation is stale or pausing".into());
        }
        owner
            .handle
            .clone()
            .ok_or_else(|| "agent is not ready".into())
    }

    pub async fn send_turn(&self, tab: u64, generation: u64, text: String) -> Result<(), String> {
        let (handle, key) = {
            let inner = lock(&self.inner);
            let (key, owner) = inner
                .owners
                .iter()
                .find(|(_, o)| o.tab == Some(tab))
                .ok_or("agent is not open")?;
            if inner.closing
                || owner.generation != generation
                || *owner.pause.borrow() != Pause::None
            {
                return Err("agent generation is stale or pausing".into());
            }
            let handle = owner.handle.clone().ok_or("agent is not ready")?;
            let chat = self
                .state
                .chat_snapshot()
                .into_iter()
                .find(|c| c.tab == tab)
                .ok_or("agent chat is missing")?;
            if chat.phase != ChatPhase::Running || chat.turn_running {
                return Err("agent is not ready for a new turn".into());
            }
            events::update(
                &self.state,
                tab,
                generation,
                ChatUpdate::Turn { running: true },
            );
            (handle, key.clone())
        };

        let state = self.state.clone();
        let (reply, received) = oneshot::channel();
        self.runtime.spawn(async move {
            let result = handle.send_turn(text.clone()).await;
            if result.is_err() {
                events::update(&state, tab, generation, ChatUpdate::Turn { running: false });
            }

            // Clear only the submitted draft, independently of the caller's
            // lifetime. A newer draft in a reopened tab must remain untouched.
            let result = result.and_then(|()| {
                state
                    .dispatch(Command::ClearAgentDraft {
                        profile_path: key.profile.to_string_lossy().into_owned(),
                        name: key.name,
                        text,
                    })
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            });
            let _ = reply.send(result);
        });
        received.await.map_err(|e| e.to_string())?
    }

    pub async fn interrupt(&self, tab: u64, generation: u64) -> Result<(), String> {
        let handle = self.handle(tab, generation)?;
        let (reply, received) = oneshot::channel();
        self.runtime.spawn(async move {
            let _ = reply.send(handle.interrupt().await.map(|_| ()));
        });
        received.await.map_err(|e| e.to_string())?
    }

    pub async fn answer_permission(
        &self,
        tab: u64,
        generation: u64,
        request_id: String,
        answer: PermissionAnswer,
    ) -> Result<(), String> {
        let handle = self.handle(tab, generation)?;
        let state = self.state.clone();
        let (reply, received) = oneshot::channel();
        self.runtime.spawn(async move {
            let result = handle.answer_permission(request_id.clone(), answer).await;
            if result.is_ok() {
                events::update(
                    &state,
                    tab,
                    generation,
                    ChatUpdate::PermissionRemoved { request_id },
                );
            }
            let _ = reply.send(result);
        });
        received.await.map_err(|e| e.to_string())?
    }
}

impl Drop for Agents {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn phase(state: &Store, owner: &Owner, phase: ChatPhase) {
    if let Some(tab) = owner.tab {
        events::update(state, tab, owner.generation, ChatUpdate::Phase { phase });
    }
}

async fn shutdown(inner: &Mutex<Inner>, state: &Store) {
    let waits: Vec<_> = {
        let mut inner = lock(inner);
        inner.closing = true;
        let mut waits: Vec<_> = inner
            .operations
            .values()
            .map(|operation| {
                operation.cancel.request_cancel();
                operation.done.subscribe()
            })
            .collect();
        waits.extend(inner.deletions.values().map(|done| done.subscribe()));
        waits.extend(inner.owners.values().map(|owner| {
            if !*owner.done.borrow() {
                phase(state, owner, ChatPhase::Pausing);
                owner.pause.send_replace(Pause::Immediate);
            }
            owner.done.subscribe()
        }));
        waits
    };
    for mut done in waits {
        let _ = done.wait_for(|done| *done).await;
    }
}

async fn run(
    inner: Arc<Mutex<Inner>>,
    state: Arc<Store>,
    log: Arc<Log>,
    key: Key,
    generation: u64,
    mut pause: watch::Receiver<Pause>,
    context: LaunchContext,
) {
    let outcome = run_session(&inner, &state, &log, &key, generation, &mut pause, context).await;
    let mut inner = lock(&inner);
    let Some(owner) = inner
        .owners
        .get_mut(&key)
        .filter(|o| o.run_id == generation)
    else {
        return;
    };
    if let Err(error) = outcome {
        log.report_error(&error);
        if let Some(tab) = owner.tab {
            events::update(
                &state,
                tab,
                owner.generation,
                ChatUpdate::Failure {
                    message: error.clone(),
                },
            );
        }
        phase(&state, owner, ChatPhase::Failed);
    } else {
        phase(&state, owner, ChatPhase::Paused);
    }
    owner.handle = None;
    owner.done.send_replace(true);
}

async fn run_session(
    inner: &Mutex<Inner>,
    state: &Store,
    log: &Arc<Log>,
    key: &Key,
    generation: u64,
    pause: &mut watch::Receiver<Pause>,
    context: LaunchContext,
) -> Result<(), String> {
    let path = key.profile.clone();
    let name = key.name.clone();
    let (space, history) = tokio::task::spawn_blocking(move || {
        let space = Space::open(&path, &name)?;
        let history = space.history()?;
        Ok::<_, String>((space, history))
    })
    .await
    .map_err(|e| e.to_string())??;
    if *pause.borrow() == Pause::Immediate {
        return Ok(());
    }
    publish(inner, state, key, generation, SpaceEvent::Restored(history));
    let mut running = match space.start(&context, log.clone()).await {
        Ok(running) => running,
        Err(error) => {
            let profile = key.profile.clone();
            let name = key.name.clone();
            let restored = tokio::task::spawn_blocking(move || {
                Space::history_before(&profile, &name, None, 200)
            })
            .await;
            if let Ok(Ok(items)) = restored {
                publish(inner, state, key, generation, SpaceEvent::Restored(items));
            }
            return Err(error);
        }
    };
    {
        let mut inner = lock(inner);
        let owner = inner.owners.get_mut(key).ok_or("agent owner disappeared")?;
        owner.handle = Some(running.handle());
    }
    loop {
        let should_pause = match *pause.borrow() {
            Pause::None => false,
            Pause::Immediate => true,
            Pause::AfterTurn => {
                let tab = lock(inner).owners.get(key).and_then(|o| o.tab);
                !state
                    .chat_snapshot()
                    .iter()
                    .any(|chat| Some(chat.tab) == tab && chat.turn_running)
            }
        };
        if should_pause {
            let result = running.pause().await;
            while let Some(event) = running.next_update().await {
                publish(inner, state, key, generation, event);
            }
            return result.map(|_| ());
        }
        tokio::select! {
            biased;
            _ = pause.changed() => {},
            event = running.next_update() => {
                let Some(event) = event else { let _ = running.pause().await; return Ok(()); };
                let failed = match &event { SpaceEvent::Session(SessionEvent::Failed(message)) => Some(message.clone()), _ => None };
                if let Some(error) = failed {
                    let _ = running.pause().await;
                    return Err(error);
                }
                publish(inner, state, key, generation, event);
            }
        }
    }
}

fn publish(inner: &Mutex<Inner>, state: &Store, key: &Key, run_id: u64, event: SpaceEvent) {
    let mut inner = lock(inner);
    if matches!(event, SpaceEvent::Session(SessionEvent::Exited { .. })) {
        inner.generation += 1;
        let generation = inner.generation;
        if let Some(owner) = inner.owners.get_mut(key).filter(|o| o.run_id == run_id) {
            if let Some(tab) = owner.tab {
                events::update(
                    state,
                    tab,
                    owner.generation,
                    ChatUpdate::Generation { generation },
                );
            }
            owner.generation = generation;
        }
    }
    if let Some(owner) = inner.owners.get(key).filter(|o| o.run_id == run_id) {
        if let Some(tab) = owner.tab {
            events::publish(state, tab, owner.generation, event);
        }
    }
}

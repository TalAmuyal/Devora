//! Agent Space ownership independent of UI subscriptions. Closing a tab removes
//! it immediately and signals pause. Its entry remains reserved until the process
//! is reaped and C's StopInfo persistence completes. Reopen waits for that barrier.
mod events;
mod global_pause;
mod launch;
mod models;
mod operations;
mod running;

pub use devora_agent::models::{same_model, ModelRow, ModelSections};
use devora_agent::session::SessionEvent;
pub use devora_agent::session::{PermissionAnswer, Plugin};
use devora_agent_space::{
    ActiveModel, ChosenEffort, ChosenModel, RunningSpace, SpaceEvent, SpaceHandle,
};
use devora_agent_space::{AppRun, Space};
pub use devora_agent_space::{
    CreationForm, Entry as SpaceEntry, Field, LaunchContext, ModelChoice, Source,
};
use devora_app::log::{Level, Log};
pub use devora_config::agent::Scope;
use devora_state::{
    AgentPause, ChatCommand, ChatPhase, ChatUpdate, Command, Event, SessionKind, Store,
};
pub use models::{ModelOptions, MODEL_LOOKUP_GRACE};
pub use operations::{Creation, CreationEvent};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use tokio::runtime::Handle;
use tokio::sync::{oneshot, watch, Notify};

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

struct Owner {
    run_id: u64,
    tab: Option<u64>,
    generation: u64,
    handle: Option<SpaceHandle>,
    /// Stops its actor now: a pause, a closed tab, shutting down. Why it is paused is `paused`.
    pause: watch::Sender<bool>,
    paused: AgentPause,
    done: watch::Sender<bool>,
    /// From launching its runtime until the runtime has stopped.
    live: bool,
    /// What its latest launch ran, which keys the models it was offered (`ModelOptions::cached`).
    launched: Option<models::Launch>,
}

struct Inner {
    owners: BTreeMap<Key, Owner>,
    generation: u64,
    closing: bool,
    operations: BTreeMap<u64, operations::Operation>,
    deletions: BTreeMap<Key, watch::Sender<bool>>,
    /// Woken whenever an owner's runtime starts or stops, for `running::track`.
    changed: Arc<Notify>,
    /// What went wrong while shutting down, for `shutdown` to return.
    shutdown_problems: Vec<String>,
    /// The launch of the latest open, for reopening an agent from Keep paused.
    launch: Option<LaunchContext>,
    /// The models each profile's agents are offered, which every agent start fills (`models`).
    models: Arc<ModelOptions>,
    /// This app process, which an agent launched in another one was restarted
    /// from (its restart notice, `devora_agent_space::AppRun`).
    run: AppRun,
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

/// The launch context from the user's login shell and the app bundle's `Resources` directory, for a host without the
/// agent registry (Ember's settings, which list the models Claude Code offers).
pub async fn resolve_launch_context(
    global_config: PathBuf,
    resources: Option<PathBuf>,
) -> Result<LaunchContext, String> {
    launch::resolve(global_config, resources).await
}

/// Whether this is a test run (`DEVORA_TEST_MODE`) that names no fake Claude CLI (`DEVORA_TEST_CLAUDE_PROGRAM`): such a
/// run must look up no models, which would launch the user's own `claude`.
pub fn test_run_without_fake_claude() -> bool {
    test_run() && std::env::var_os("DEVORA_TEST_CLAUDE_PROGRAM").is_none()
}

/// Whether this is a test run that names a fake Claude CLI, which then stands for `claude`
/// (`devora_agent_space::claude_program`).
fn test_run_with_fake_claude() -> bool {
    test_run() && std::env::var_os("DEVORA_TEST_CLAUDE_PROGRAM").is_some()
}

fn test_run() -> bool {
    std::env::var("DEVORA_TEST_MODE").is_ok_and(|mode| mode == "1")
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
            changed: Arc::new(Notify::new()),
            shutdown_problems: Vec::new(),
            models: Arc::new(ModelOptions::new(log.clone())),
            launch: None,
            run: AppRun::new(),
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
        runtime.spawn(running::track(inner.clone(), state.clone()));
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
    /// The user's own open (Resume, Retry, the Agent Hub) clears an individual pause first.
    pub async fn open(
        &self,
        profile: &Path,
        name: &str,
        context: LaunchContext,
    ) -> Result<u64, String> {
        let key = Key::new(profile, name)?;
        // During a hold, opening only goes to the agent's tab; Keep paused is how its pause changes then.
        if !self.state.global_paused() {
            set_paused(&self.state, &key, false)?;
        }
        self.open_key(key, context, false).await
    }

    /// Opens the agent as its stored pause says: a paused one shows Paused, and launches only once the user resumes it.
    /// `reopen` opens a stopped agent anew during a hold, held, so that it launches with the others on release.
    async fn open_key(
        &self,
        key: Key,
        context: LaunchContext,
        reopen: bool,
    ) -> Result<u64, String> {
        lock(&self.inner).launch = Some(context.clone());
        loop {
            let mut done = {
                let mut inner = lock(&self.inner);
                if inner.closing {
                    return Err("agents are shutting down".into());
                }
                if inner.deletions.contains_key(&key) {
                    return Err("agent space is being deleted".into());
                }
                if self.state.global_paused() {
                    if let Some(tab) = inner
                        .owners
                        .get(&key)
                        .filter(|owner| !reopen || !*owner.done.borrow())
                        .and_then(|owner| owner.tab)
                    {
                        self.state
                            .dispatch(Command::ActivateTab { tab })
                            .map_err(|e| e.to_string())?;
                        return Ok(tab);
                    }
                }
                let waiting = inner.owners.get(&key).filter(|owner| !*owner.done.borrow());
                if let Some(owner) = waiting {
                    if !*owner.pause.borrow()
                        && matches!(
                            owner.paused,
                            AgentPause::None
                                | AgentPause::Held {
                                    keep_paused: false,
                                    ..
                                }
                        )
                    {
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
                    let (pause, paused) = watch::channel(false);
                    let (done, _) = watch::channel(false);
                    let view = self.state.snapshot().agent_views.into_iter().find(|view| {
                        view.profile_path == key.profile.to_string_lossy() && view.name == key.name
                    });
                    inner.owners.insert(
                        key.clone(),
                        Owner {
                            run_id: generation,
                            tab: Some(tab),
                            generation,
                            handle: None,
                            pause,
                            // During a hold, a stored pause is the Keep paused choice; without one, it keeps the agent from launching.
                            paused: if view.as_ref().is_some_and(|view| view.paused) {
                                AgentPause::PausedAtOpen
                            } else {
                                AgentPause::None
                            },
                            done,
                            live: false,
                            launched: None,
                        },
                    );
                    inner.changed.notify_one();
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
                    Some(profile) => match Key::new(Path::new(profile), name) {
                        Ok(key) => self.open_key(key, context.clone(), false).await.map(|_| ()),
                        Err(error) => Err(error),
                    },
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
                    // The tab's chat says it, so it gets no error banner too.
                    self.log.write(Level::Error, &format!("{name}: {error}"));
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
            owner.pause.send_replace(true);
        }
        inner.changed.notify_one();
        Ok(())
    }

    /// During a global hold: whether the agent stays paused when Devora resumes (one stored flag with Pause).
    /// A held owner changes only its release choice. An agent paused before the hold has stopped: unticking it
    /// reopens it, held, so that it launches with the others on release.
    pub async fn set_keep_paused(
        &self,
        tab: u64,
        generation: u64,
        keep_paused: bool,
    ) -> Result<(), String> {
        let reopen = {
            let mut inner = lock(&self.inner);
            if inner.closing {
                return Err("agents are shutting down".into());
            }
            if !self.state.global_paused() {
                return Err("Keep paused requires an active global hold".into());
            }
            let launch = inner.launch.clone();
            let (key, owner) = inner
                .owners
                .iter_mut()
                .find(|(_, owner)| owner.tab == Some(tab))
                .ok_or("agent is not open")?;
            if owner.generation != generation || *owner.pause.borrow() {
                return Err("agent generation is stale or pausing".into());
            }
            if *owner.done.borrow() {
                // Paused before the hold, so already stopped and kept paused.
                if keep_paused {
                    return Ok(());
                }
                set_paused(&self.state, key, false)?;
                Some((key.clone(), launch.ok_or("the agent's launch is unknown")?))
            } else {
                let AgentPause::Held {
                    keep_paused: kept,
                    released,
                } = owner.paused
                else {
                    return Err("agent is not globally held".into());
                };
                if kept == keep_paused {
                    return Ok(());
                }
                set_paused(&self.state, key, keep_paused)?;
                set_pause(
                    &self.state,
                    owner,
                    AgentPause::Held {
                        keep_paused,
                        released,
                    },
                );
                None
            }
        };
        if let Some((key, launch)) = reopen {
            self.open_key(key, launch, true).await?;
        }
        Ok(())
    }

    /// Pauses at once, interrupting a running turn, and returns once the agent has stopped: only `delete` does this.
    async fn pause_now(&self, tab: u64) -> Result<(), String> {
        let mut done = {
            let mut inner = lock(&self.inner);
            let (key, owner) = inner
                .owners
                .iter_mut()
                .find(|(_, o)| o.tab == Some(tab))
                .ok_or("agent is not open")?;
            set_paused(&self.state, key, true)?;
            set_pause(&self.state, owner, AgentPause::ByUser);
            if *owner.done.borrow() {
                return Ok(());
            }
            phase(&self.state, owner, ChatPhase::Pausing);
            owner.pause.send_replace(true);
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
        let mut inner = lock(&self.inner);
        if inner.closing {
            return Err("agents are shutting down".into());
        }
        let (key, owner) = inner
            .owners
            .iter_mut()
            .find(|(_, o)| o.tab == Some(tab))
            .ok_or("agent is not open")?;
        if owner.generation != generation || *owner.pause.borrow() || *owner.done.borrow() {
            return Err("agent generation is stale or pausing".into());
        }
        if owner.paused == AgentPause::ByUser {
            return Ok(());
        }
        // Committed when requested, not at the turn's boundary: an app killed mid-turn brings the agent back
        // paused, as the user asked, rather than running.
        set_paused(&self.state, key, true)?;
        let key = key.clone();
        if matches!(owner.paused, AgentPause::Held { .. }) && owner.handle.is_none() {
            set_pause(&self.state, owner, AgentPause::ByUser);
            owner.pause.send_replace(true);
            return Ok(());
        }
        let completion = owner
            .handle
            .as_ref()
            .ok_or("agent is not ready")?
            .pause_after_turn();
        set_pause(&self.state, owner, AgentPause::ByUser);
        let run_id = owner.run_id;
        drop(inner);

        let inner = self.inner.clone();
        let state = self.state.clone();
        let log = self.log.clone();
        self.runtime.spawn(async move {
            if let Err(error) = completion.await {
                let report = {
                    let mut inner = lock(&inner);
                    if let Some(owner) = inner.owners.values_mut().find(|o| {
                        o.tab == Some(tab)
                            && o.run_id == run_id
                            && !*o.done.borrow()
                            && !*o.pause.borrow()
                    }) {
                        let _ = set_paused(&state, &key, false);
                        set_pause(&state, owner, AgentPause::None);
                        true
                    } else {
                        false
                    }
                };
                if report {
                    log.report_error(&format!("pause after turn failed: {error}"));
                }
            }
        });
        Ok(())
    }

    /// Stops every agent, interrupting live turns, and every model lookup; returns what went wrong, if anything did.
    pub async fn shutdown(&self) -> Vec<String> {
        // On the registry's runtime, where the lookups run (a UI calls this from its own executor).
        let models = lock(&self.inner).models.clone();
        let _ = self
            .runtime
            .spawn(async move { models.shutdown(MODEL_LOOKUP_GRACE).await })
            .await;
        shutdown(&self.inner, &self.state).await
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
    /// `Resources` directory, for hosts that supply nothing of their own (`resolve_launch_context`, on the registry's
    /// runtime).
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

    /// The instructions `form` would create, as creation renders them.
    pub fn creation_preview(
        &self,
        profile: &Path,
        global_config: &Path,
        launch_inputs: BTreeMap<String, String>,
        form: &CreationForm,
    ) -> Result<String, String> {
        devora_agent_space::preview(profile, global_config, launch_inputs, form)
    }

    /// The model and effort the New Agent form starts with.
    pub fn model_defaults(
        &self,
        profile: &Path,
        global_config: &Path,
    ) -> Result<ModelChoice, String> {
        devora_agent_space::model_defaults(global_config, profile)
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

    /// Deletes an agent's space. An open agent is paused at once and its tab closed first, as separate steps that each
    /// stay done if a later one fails: a failed delete leaves it paused, its tab closed. One whose tab is already closed
    /// but which is still stopping is refused.
    pub async fn delete(&self, profile: &Path, name: &str) -> Result<(), String> {
        let key = Key::new(profile, name)?;
        let open = lock(&self.inner)
            .owners
            .get(&key)
            .and_then(|owner| owner.tab);
        if let Some(tab) = open {
            self.pause_now(tab).await?;
            self.close_tab(tab)?;
        }
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
                inner.changed.notify_one();
            }
            done.send_replace(true);
            let _ = reply.send(result);
        });
        received.await.map_err(|e| e.to_string())?
    }

    /// Sets the agent on `tab` to `model`, which must be one the agent is offered: a running agent's by the list its own
    /// launch was offered, a paused one's by the list its next launch would be (asking Claude Code when that isn't
    /// cached, so it works after a restart too). A running agent stores it and tells its CLI between turns
    /// (`SpaceHandle::set_model`); a paused one stores it for its next launch.
    pub async fn set_model(&self, tab: u64, generation: u64, model: String) -> Result<(), String> {
        let (key, running, next_launch) = {
            let inner = lock(&self.inner);
            let (key, owner) = inner
                .owners
                .iter()
                .find(|(_, o)| o.tab == Some(tab))
                .ok_or("agent is not open")?;
            if inner.closing || owner.generation != generation {
                return Err("agent generation is stale".into());
            }
            let running = match (owner.live, &owner.handle, &owner.launched) {
                (false, ..) => None,
                (true, Some(handle), Some(launched)) if !*owner.pause.borrow() => {
                    Some((handle.clone(), launched.clone()))
                }
                (true, ..) => {
                    return Err(
                        "This agent is starting or pausing. Set its model once it has.".into(),
                    )
                }
            };
            (key.clone(), running, inner.launch.clone())
        };
        let models = lock(&self.inner).models.clone();
        let offered = match &running {
            Some((_, launched)) => models
                .cached(&key.profile, launched)
                .ok_or("Devora doesn't know the models this agent is offered.")?,
            None => {
                let context =
                    next_launch.ok_or("Devora doesn't know how to launch this agent yet.")?;
                let (profile, name) = (key.profile.clone(), key.name.clone());
                let probe = self
                    .runtime
                    .spawn_blocking(move || {
                        Space::open(&profile, &name)?.own_probe_options(&context)
                    })
                    .await
                    .map_err(|e| e.to_string())??;
                let profile = key.profile.clone();
                self.runtime
                    .spawn(async move { models.offered(&profile, probe).await })
                    .await
                    .map_err(|e| e.to_string())??
            }
        };
        if !offered.offers(&model) {
            return Err(format!(
                "Claude Code doesn't offer the model {model} to this agent."
            ));
        }
        // Asking Claude Code takes seconds, in which a paused agent may have resumed: its running session takes it then.
        let running = match running {
            Some((handle, _)) => Some(handle),
            None => {
                let inner = lock(&self.inner);
                let owner = inner.owners.get(&key).ok_or("agent is not open")?;
                match (owner.live, &owner.handle) {
                    (false, _) => None,
                    (true, Some(handle)) if !*owner.pause.borrow() => Some(handle.clone()),
                    (true, _) => {
                        return Err(
                            "This agent is starting or pausing. Set its model once it has.".into(),
                        )
                    }
                }
            }
        };
        match running {
            Some(handle) => handle.set_model(model).await,
            None => {
                let stored = key.clone();
                self.runtime
                    .spawn_blocking(move || {
                        Space::open(&stored.profile, &stored.name)?.set_model(&model)
                    })
                    .await
                    .map_err(|e| e.to_string())??;
                // The levels the effort offers are the new model's.
                self.present_paused_effort(&key).await;
                Ok(())
            }
        }
    }

    // `set_effort` and `set_model` route a pick the same way (find the owner, running or paused, check what it's
    // offered, look again after the wait, then tell the session or write the space): a third setter shares that routing
    // with them rather than copying it.
    /// Sets the effort of the agent on `tab` (none is Claude Code's default) to a level its model takes, which the
    /// models it's offered say, as `set_model` checks a model: a running agent's model is the one its chat shows it set
    /// to, a paused one's the one its next launch would run. A level the model doesn't take is refused, since the CLI
    /// would take it silently and run another. A running agent stores it and tells its CLI between turns
    /// (`SpaceHandle::set_effort`); a paused one stores it for its next launch.
    pub async fn set_effort(
        &self,
        tab: u64,
        generation: u64,
        effort: Option<String>,
    ) -> Result<(), String> {
        let (key, running, next_launch) = {
            let inner = lock(&self.inner);
            let (key, owner) = inner
                .owners
                .iter()
                .find(|(_, o)| o.tab == Some(tab))
                .ok_or("agent is not open")?;
            if inner.closing || owner.generation != generation {
                return Err("agent generation is stale".into());
            }
            let running = match (owner.live, &owner.handle, &owner.launched) {
                (false, ..) => None,
                (true, Some(handle), Some(launched)) if !*owner.pause.borrow() => {
                    Some((handle.clone(), launched.clone()))
                }
                (true, ..) => {
                    return Err(
                        "This agent is starting or pausing. Set its effort once it has.".into(),
                    )
                }
            };
            (key.clone(), running, inner.launch.clone())
        };
        let models = lock(&self.inner).models.clone();
        let (offered, model) = match &running {
            Some((_, launched)) => {
                let offered = models
                    .cached(&key.profile, launched)
                    .ok_or("Devora doesn't know the models this agent is offered.")?;
                let model = self
                    .state
                    .chat_snapshot()
                    .into_iter()
                    .find(|chat| chat.tab == tab)
                    .and_then(|chat| chat.model)
                    .map(|model| model.set_to)
                    .ok_or("Devora doesn't know this agent's model yet.")?;
                (offered, model)
            }
            None => {
                let context =
                    next_launch.ok_or("Devora doesn't know how to launch this agent yet.")?;
                let (profile, name) = (key.profile.clone(), key.name.clone());
                let (model, probe) = self
                    .runtime
                    .spawn_blocking(move || {
                        let space = Space::open(&profile, &name)?;
                        let model = space.session_options(&context)?.model;
                        Ok::<_, String>((model, space.own_probe_options(&context)?))
                    })
                    .await
                    .map_err(|e| e.to_string())??;
                let profile = key.profile.clone();
                let offered = self
                    .runtime
                    .spawn(async move { models.offered(&profile, probe).await })
                    .await
                    .map_err(|e| e.to_string())??;
                (offered, model)
            }
        };
        if let Some(level) = &effort {
            let levels = offered.effort_levels(&model).ok_or_else(|| {
                format!("Claude Code doesn't list the model {model}, so which effort levels it takes isn't known.")
            })?;
            if !levels.contains(level) {
                return Err(if levels.is_empty() {
                    format!("The model {model} takes no effort level.")
                } else {
                    format!(
                        "The model {model} takes {}, not {level}.",
                        levels.join(", ")
                    )
                });
            }
        }
        // Asking Claude Code takes seconds, in which a paused agent may have resumed: its running session takes it then.
        let running = match running {
            Some((handle, _)) => Some(handle),
            None => {
                let inner = lock(&self.inner);
                let owner = inner.owners.get(&key).ok_or("agent is not open")?;
                match (owner.live, &owner.handle) {
                    (false, _) => None,
                    (true, Some(handle)) if !*owner.pause.borrow() => Some(handle.clone()),
                    (true, _) => {
                        return Err(
                            "This agent is starting or pausing. Set its effort once it has.".into(),
                        )
                    }
                }
            }
        };
        match running {
            Some(handle) => handle.set_effort(effort).await,
            None => {
                let stored = key.clone();
                self.runtime
                    .spawn_blocking(move || {
                        Space::open(&stored.profile, &stored.name)?
                            .set_effort(effort.as_deref(), &model)
                    })
                    .await
                    .map_err(|e| e.to_string())??;
                self.present_paused_effort(&key).await;
                Ok(())
            }
        }
    }

    /// Shows a paused agent's stored effort on its chat again, after a pick changed it or its model.
    async fn present_paused_effort(&self, key: &Key) {
        let (run_id, context) = {
            let inner = lock(&self.inner);
            let Some(owner) = inner.owners.get(key) else {
                return;
            };
            (owner.run_id, inner.launch.clone())
        };
        let Some(context) = context else { return };
        let (profile, name) = (key.profile.clone(), key.name.clone());
        let options = self
            .runtime
            .spawn_blocking(move || {
                let space = Space::open(&profile, &name)?;
                Ok::<_, String>((
                    space.session_options(&context)?,
                    space.own_probe_options(&context)?,
                ))
            })
            .await;
        if let Ok(Ok((options, probe))) = options {
            let _ = self
                .runtime
                .spawn(present_stored_effort(
                    self.inner.clone(),
                    self.state.clone(),
                    key.clone(),
                    run_id,
                    options,
                    probe,
                ))
                .await;
        }
    }

    fn handle(&self, tab: u64, generation: u64) -> Result<SpaceHandle, String> {
        let inner = lock(&self.inner);
        let owner = inner
            .owners
            .values()
            .find(|o| o.tab == Some(tab))
            .ok_or("agent is not open")?;
        if inner.closing || owner.generation != generation || *owner.pause.borrow() {
            return Err("agent generation is stale or pausing".into());
        }
        owner
            .handle
            .clone()
            .ok_or_else(|| "agent is not ready".into())
    }

    pub async fn send_turn(&self, tab: u64, generation: u64, text: String) -> Result<(), String> {
        if self.state.global_paused() {
            return Err("Globally paused — resume Devora to send".into());
        }
        let (handle, key) = {
            let inner = lock(&self.inner);
            let (key, owner) = inner
                .owners
                .iter()
                .find(|(_, o)| o.tab == Some(tab))
                .ok_or("agent is not open")?;
            if inner.closing
                || owner.generation != generation
                || (*owner.pause.borrow() || owner.paused == AgentPause::ByUser)
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
            // A prompt sent during a turn the agent started itself joins it (`ActiveTurn`), which is then the user's.
            if chat.phase != ChatPhase::Running || (chat.turn_running && !chat.agent_turn) {
                return Err("agent is not ready for a new turn".into());
            }
            events::update(
                &self.state,
                tab,
                generation,
                ChatUpdate::Turn {
                    running: true,
                    by_agent: false,
                },
            );
            (handle, key.clone())
        };

        let state = self.state.clone();
        let (reply, received) = oneshot::channel();
        self.runtime.spawn(async move {
            let result = handle.send_turn(text.clone()).await;
            if result.is_err() {
                events::update(
                    &state,
                    tab,
                    generation,
                    ChatUpdate::Turn {
                        running: false,
                        by_agent: false,
                    },
                );
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

    /// Sets the agent's own compaction threshold, or clears it to follow its
    /// profile's. Needs the agent running: its actor owns the space's database.
    pub async fn set_compact_threshold(
        &self,
        tab: u64,
        generation: u64,
        percent: Option<u8>,
    ) -> Result<(), String> {
        let handle = self.handle(tab, generation)?;
        let (reply, received) = oneshot::channel();
        self.runtime.spawn(async move {
            let _ = reply.send(handle.set_compact_threshold(percent).await);
        });
        received.await.map_err(|e| e.to_string())?
    }

    /// Compacts the agent's conversation as reaching its threshold would: now,
    /// or when its running turn ends.
    pub async fn compact_now(&self, tab: u64, generation: u64) -> Result<(), String> {
        let handle = self.handle(tab, generation)?;
        let (reply, received) = oneshot::channel();
        self.runtime.spawn(async move {
            let _ = reply.send(handle.compact_now().await);
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

async fn shutdown(inner: &Mutex<Inner>, state: &Store) -> Vec<String> {
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
                owner.pause.send_replace(true);
            }
            owner.done.subscribe()
        }));
        waits
    };
    for mut done in waits {
        let _ = done.wait_for(|done| *done).await;
    }
    std::mem::take(&mut lock(inner).shutdown_problems)
}

async fn run(
    inner: Arc<Mutex<Inner>>,
    state: Arc<Store>,
    log: Arc<Log>,
    key: Key,
    generation: u64,
    mut pause: watch::Receiver<bool>,
    context: LaunchContext,
) {
    loop {
        let outcome = run_session(
            &inner,
            &state,
            &log,
            &key,
            generation,
            &mut pause,
            context.clone(),
        )
        .await;
        {
            let mut inner = lock(&inner);
            let inner = &mut *inner;
            let Some(owner) = inner
                .owners
                .get_mut(&key)
                .filter(|o| o.run_id == generation)
            else {
                return;
            };
            owner.handle = None;
            owner.live = false;
            inner.changed.notify_one();
            if let Err(error) = outcome {
                // The agent's chat says it, beside Retry, so it gets no error banner too; with no tab to say it, it
                // is reported.
                if owner.tab.is_some() {
                    log.write(Level::Error, &format!("{}: {error}", key.name));
                } else {
                    log.report_error(&format!("{}: {error}", key.name));
                }
                if inner.closing {
                    inner
                        .shutdown_problems
                        .push(format!("{}: {error}", key.name));
                }
                if let Some(tab) = owner.tab {
                    events::update(
                        &state,
                        tab,
                        owner.generation,
                        ChatUpdate::Failure { message: error },
                    );
                }
                phase(&state, owner, ChatPhase::Failed);
                owner.done.send_replace(true);
                return;
            }
            phase(&state, owner, ChatPhase::Paused);
        }
        if !global_pause::wait_for_release(&inner, &state, &key, generation, &mut pause).await {
            let inner = lock(&inner);
            if let Some(owner) = inner.owners.get(&key).filter(|o| o.run_id == generation) {
                owner.done.send_replace(true);
            }
            inner.changed.notify_one();
            return;
        }
        let mut inner = lock(&inner);
        let eligible = inner.owners.get(&key).is_some_and(|o| {
            o.run_id == generation
                && o.tab.is_some()
                && o.paused != AgentPause::ByUser
                && !*o.pause.borrow()
        });
        if !eligible || inner.closing {
            if let Some(owner) = inner.owners.get(&key).filter(|o| o.run_id == generation) {
                owner.done.send_replace(true);
            }
            inner.changed.notify_one();
            return;
        }
        inner.generation += 1;
        let next = inner.generation;
        let owner = inner.owners.get_mut(&key).expect("checked owner");
        owner.generation = next;
        owner.paused = global_pause::relaunched(&owner.paused);
        if let Some(tab) = owner.tab {
            let _ = state.dispatch_chat(ChatCommand::Start {
                tab,
                generation: next,
            });
        }
    }
}

async fn run_session(
    inner: &Arc<Mutex<Inner>>,
    state: &Arc<Store>,
    log: &Arc<Log>,
    key: &Key,
    generation: u64,
    pause: &mut watch::Receiver<bool>,
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
    let changes = state.subscribe_global_pause();
    let globally_paused =
        global_pause::check(inner.clone(), state.clone(), key.clone(), generation);
    publish(inner, state, key, generation, SpaceEvent::Restored(history));
    // What it will launch with, shown before it launches, so a paused or held agent shows its effort too.
    if let (Ok(options), Ok(probe)) = (
        space.session_options(&context),
        space.own_probe_options(&context),
    ) {
        tokio::spawn(present_stored_effort(
            inner.clone(),
            state.clone(),
            key.clone(),
            generation,
            options,
            probe,
        ));
    }
    while globally_paused() && state.global_paused() && !*pause.borrow() {
        if let Some(owner) = lock(inner).owners.get(key) {
            phase(state, owner, ChatPhase::Paused);
        }
        tokio::select! {
            _ = pause.changed() => {},
            _ = changes.next_batch(32) => {},
        }
    }
    if *pause.borrow() {
        return Ok(());
    }
    // Paused until the user resumes it: shown Paused, never launched.
    if lock(inner).owners.get(key).is_some_and(|owner| {
        matches!(
            owner.paused,
            AgentPause::PausedAtOpen
                | AgentPause::Held {
                    keep_paused: true,
                    ..
                }
        )
    }) {
        return Ok(());
    }
    if let Some(owner) = lock(inner).owners.get_mut(key) {
        owner.live = true;
        phase(state, owner, ChatPhase::Opening);
        set_pause(state, owner, AgentPause::None);
    }
    let run = lock(inner).run.clone();
    let mut running = match space
        .start_with_global_pause(&context, log.clone(), globally_paused, &run)
        .await
    {
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
    // What this launch ran, which keys the models it's offered.
    let launched = models::Launch::new(running.launch_program(), running.launch_settings());
    // The model the user set the agent to in this run, as the space last said.
    let mut chosen = ChosenModel::default();
    // The effort the user set the agent to in this run, as the space last said.
    let mut chosen_effort: Option<ChosenEffort> = None;
    // The model the agent's replies come from, as the space last said.
    let mut active = ActiveModel {
        latest: None,
        fallback: None,
    };
    loop {
        if *pause.borrow() {
            let result = running.pause().await;
            while let Some(event) = running.next_update().await {
                publish(inner, state, key, generation, event);
            }
            note_forced_stop(inner, key, &result?);
            return Ok(());
        }
        tokio::select! {
            biased;
            _ = pause.changed() => {},
            _ = changes.next_batch(32) => {
                global_pause::changed(inner.clone(), state.clone(), key.clone(), generation, running.handle());
            },
            event = running.next_update() => {
                let Some(event) = event else { let _ = running.pause().await; return Ok(()); };
                let failed = match &event { SpaceEvent::Session(SessionEvent::Failed(message)) => Some(message.clone()), _ => None };
                if let Some(error) = failed {
                    let _ = running.pause().await;
                    return Err(error);
                }
                let ready = matches!(&event, SpaceEvent::Session(SessionEvent::Ready { .. }));
                if let SpaceEvent::Session(SessionEvent::Ready { capabilities, .. }) = &event {
                    let mut inner = lock(inner);
                    inner.models.clone().learned(&key.profile, &launched, devora_agent::models::decode(capabilities));
                    if let Some(owner) = inner.owners.get_mut(key).filter(|o| o.run_id == generation) {
                        owner.launched = Some(launched.clone());
                    }
                }
                match event {
                    SpaceEvent::Model(model) => {
                        active = model;
                        present_model(inner, state, key, generation, &running, &launched, &active, &chosen, chosen_effort.as_ref());
                    }
                    SpaceEvent::ChosenEffort(effort) => {
                        chosen_effort = Some(effort);
                        present_model(inner, state, key, generation, &running, &launched, &active, &chosen, chosen_effort.as_ref());
                    }
                    SpaceEvent::ChosenModel(model) => {
                        chosen = model;
                        present_model(inner, state, key, generation, &running, &launched, &active, &chosen, chosen_effort.as_ref());
                    }
                    event => publish(inner, state, key, generation, event),
                }
                // The list this launch was just offered names the model, and says whether it's the one set.
                if ready {
                    present_model(inner, state, key, generation, &running, &launched, &active, &chosen, chosen_effort.as_ref());
                }
            }
        }
    }
}

/// Shows `active` on the agent's tab, named and checked against the models its launch was offered, and the effort it's
/// set to, with the levels its model takes.
#[allow(clippy::too_many_arguments)]
fn present_model(
    inner: &Mutex<Inner>,
    state: &Store,
    key: &Key,
    run_id: u64,
    running: &RunningSpace,
    launched: &models::Launch,
    active: &ActiveModel,
    chosen: &ChosenModel,
    chosen_effort: Option<&ChosenEffort>,
) {
    let list = lock(inner).models.cached(&key.profile, launched);
    let model = events::model(active, chosen, running.launch_model(), list.as_ref());
    let effort = events::effort(
        chosen_effort,
        chosen.set_to.as_deref().unwrap_or(running.launch_model()),
        running.launch_effort(),
        list.as_ref(),
    );
    let inner = lock(inner);
    if let Some(owner) = inner.owners.get(key).filter(|o| o.run_id == run_id) {
        if let Some(tab) = owner.tab {
            events::update(state, tab, owner.generation, ChatUpdate::Model { model });
            events::update(state, tab, owner.generation, ChatUpdate::Effort { effort });
        }
    }
}

/// Shows the effort an agent not running yet will launch with (`options`), with the levels its model takes, from the
/// models its launch would be offered (asking Claude Code when that list isn't cached): a paused or held agent's widget
/// has it before any launch, and a running one's is replaced by `present_model` once it runs.
async fn present_stored_effort(
    inner: Arc<Mutex<Inner>>,
    state: Arc<Store>,
    key: Key,
    run_id: u64,
    options: devora_agent::session::SessionOptions,
    probe: devora_agent::session::SessionOptions,
) {
    let models = lock(&inner).models.clone();
    let list = models.offered(&key.profile, probe).await.ok();
    let effort = events::effort(
        None,
        &options.model,
        options.effort.as_deref(),
        list.as_ref(),
    );
    let inner = lock(&inner);
    if let Some(owner) = inner
        .owners
        .get(&key)
        .filter(|o| o.run_id == run_id && !o.live)
    {
        if let Some(tab) = owner.tab {
            events::update(&state, tab, owner.generation, ChatUpdate::Effort { effort });
        }
    }
}

/// Commits whether the user has the agent individually paused, so that a restore keeps it paused.
/// Sets why `owner` is paused, and shows it in its chat.
fn set_pause(state: &Store, owner: &mut Owner, pause: AgentPause) {
    owner.paused = pause.clone();
    if let Some(tab) = owner.tab {
        events::update(state, tab, owner.generation, ChatUpdate::Pause { pause });
    }
}

fn set_paused(state: &Store, key: &Key, paused: bool) -> Result<(), String> {
    state
        .dispatch(Command::SetAgentPaused {
            profile_path: key.profile.to_string_lossy().into_owned(),
            name: key.name.clone(),
            paused,
        })
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// A stop that had to kill Claude during shutdown is one of the shutdown's problems.
fn note_forced_stop(inner: &Mutex<Inner>, key: &Key, stop: &devora_agent::session::StopInfo) {
    let mut inner = lock(inner);
    if inner.closing && !stop.graceful {
        inner.shutdown_problems.push(format!(
            "{}: Claude didn't stop within the grace period and was terminated",
            key.name
        ));
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

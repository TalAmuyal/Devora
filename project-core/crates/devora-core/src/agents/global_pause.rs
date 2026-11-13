//! The global hold reuses the actor's turn boundary and the registry's ownership barrier.
use super::*;

pub(super) fn check(
    inner: Arc<Mutex<Inner>>,
    state: Arc<Store>,
    key: Key,
    run_id: u64,
) -> devora_agent_space::GlobalPauseCheck {
    Arc::new(move || {
        let paused = state.global_paused();
        let mut inner = lock(&inner);
        if let Some(owner) = inner
            .owners
            .get_mut(&key)
            .filter(|owner| owner.run_id == run_id)
        {
            if paused && !owner.pause_requested && !owner.globally_held && !*owner.pause.borrow() {
                owner.globally_held = true;
                if let Some(tab) = owner.tab {
                    events::update(
                        &state,
                        tab,
                        owner.generation,
                        ChatUpdate::GlobalHold { held: true },
                    );
                    events::update(
                        &state,
                        tab,
                        owner.generation,
                        ChatUpdate::PauseRequested { requested: true },
                    );
                    events::update(
                        &state,
                        tab,
                        owner.generation,
                        ChatUpdate::KeepPaused {
                            keep_paused: owner.keep_paused,
                        },
                    );
                }
            }
            // A release cannot cancel the graceful boundary selected by Keep paused.
            return paused || (owner.globally_held && owner.keep_paused);
        }
        paused
    })
}

pub(super) async fn wait_for_release(
    inner: &Mutex<Inner>,
    state: &Store,
    key: &Key,
    run_id: u64,
    pause: &mut watch::Receiver<bool>,
) -> bool {
    let changes = state.subscribe_global_pause();
    loop {
        {
            let mut inner = lock(inner);
            let eligible = inner.owners.get(key).is_some_and(|owner| {
                owner.run_id == run_id
                    && owner.tab.is_some()
                    && owner.globally_held
                    && !owner.pause_requested
                    && !*owner.pause.borrow()
            });
            if !eligible || inner.closing {
                return false;
            }
            if !state.global_paused() {
                let owner = inner.owners.get_mut(key).expect("eligible owner");
                if owner.keep_paused {
                    owner.pause_requested = true;
                    owner.globally_held = false;
                    owner.pause.send_replace(true);
                    if let Some(tab) = owner.tab {
                        events::update(
                            state,
                            tab,
                            owner.generation,
                            ChatUpdate::GlobalHold { held: false },
                        );
                    }
                    return false;
                }
                return true;
            }
        }
        tokio::select! {
            _ = pause.changed() => {},
            _ = changes.next_batch(32) => {},
        }
    }
}

/// The actor acknowledges independently so its event consumer keeps draining
/// transcript/permission updates during a graceful pause.
pub(super) fn changed(
    inner: Arc<Mutex<Inner>>,
    state: Arc<Store>,
    key: Key,
    run_id: u64,
    handle: SpaceHandle,
) {
    // Publish the requested hold even while a turn waits for permission.
    let _ = check(inner.clone(), state.clone(), key.clone(), run_id)();
    tokio::spawn(async move {
        let stopping = handle.global_pause_changed().await.unwrap_or(true);
        let mut inner = lock(&inner);
        if !state.global_paused() {
            if let Some(owner) = inner
                .owners
                .get_mut(&key)
                .filter(|owner| owner.run_id == run_id && owner.handle.is_some())
            {
                if !stopping && !owner.keep_paused {
                    owner.globally_held = false;
                }
                if let Some(tab) = owner.tab {
                    events::update(
                        &state,
                        tab,
                        owner.generation,
                        ChatUpdate::PauseRequested {
                            requested: owner.pause_requested || owner.keep_paused,
                        },
                    );
                    events::update(
                        &state,
                        tab,
                        owner.generation,
                        ChatUpdate::GlobalHold {
                            held: owner.globally_held,
                        },
                    );
                }
            }
        }
    });
}

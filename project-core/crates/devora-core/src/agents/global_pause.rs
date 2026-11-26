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
            if paused && !*owner.pause.borrow() {
                // A pause stored before this run, never launched since, is the Keep paused choice.
                let keep_paused = match owner.paused {
                    AgentPause::None => Some(false),
                    AgentPause::PausedAtOpen => Some(true),
                    AgentPause::ByUser | AgentPause::Held { .. } => None,
                };
                if let Some(keep_paused) = keep_paused {
                    set_pause(
                        &state,
                        owner,
                        AgentPause::Held {
                            keep_paused,
                            released: false,
                        },
                    );
                }
            }
            // A release cannot cancel the graceful boundary selected by Keep paused.
            return paused
                || matches!(
                    owner.paused,
                    AgentPause::Held {
                        keep_paused: true,
                        ..
                    }
                );
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
                    && matches!(owner.paused, AgentPause::Held { .. })
                    && !*owner.pause.borrow()
            });
            if !eligible || inner.closing {
                return false;
            }
            if !state.global_paused() {
                let owner = inner.owners.get_mut(key).expect("eligible owner");
                if matches!(
                    owner.paused,
                    AgentPause::Held {
                        keep_paused: true,
                        ..
                    }
                ) {
                    // Kept paused through the hold: its stored pause, set by Keep paused, stays until the user resumes it.
                    owner.pause.send_replace(true);
                    set_pause(state, owner, AgentPause::ByUser);
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
                // One stopping at the release stays held until it has stopped, and launches again then.
                let pause = match owner.paused {
                    AgentPause::Held {
                        keep_paused: false, ..
                    } if !stopping => AgentPause::None,
                    AgentPause::Held { keep_paused, .. } => AgentPause::Held {
                        keep_paused,
                        released: true,
                    },
                    ref pause => pause.clone(),
                };
                set_pause(&state, owner, pause);
            }
        }
    });
}

/// Why an agent released from a hold is paused as it launches again. A hold that landed after the release decided to
/// relaunch it, and was given Keep paused, is a stored pause it hasn't launched since; the next check holds it with
/// that choice.
pub(super) fn relaunched(pause: &AgentPause) -> AgentPause {
    match pause {
        AgentPause::Held {
            keep_paused: true, ..
        } => AgentPause::PausedAtOpen,
        _ => AgentPause::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_relaunch_keeps_a_keep_paused_choice_made_since_the_release() {
        for released in [false, true] {
            assert_eq!(
                relaunched(&AgentPause::Held {
                    keep_paused: true,
                    released
                }),
                AgentPause::PausedAtOpen
            );
            assert_eq!(
                relaunched(&AgentPause::Held {
                    keep_paused: false,
                    released
                }),
                AgentPause::None
            );
        }
        assert_eq!(relaunched(&AgentPause::None), AgentPause::None);
    }
}

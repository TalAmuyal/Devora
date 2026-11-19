//! Which agents still have a live runtime, published to the chat projection, so a UI can tell
//! when a global hold has stopped everyone, including agents whose tab was closed mid-turn.
use super::*;
use devora_state::{AgentActivity, RunningAgent, RunningAgents};

/// Republishes whenever an owner's runtime starts or stops, or a chat's turn or questions change.
pub(super) async fn track(inner: Arc<Mutex<Inner>>, state: Arc<Store>) {
    let chats = state.subscribe_chats();
    let changed = lock(&inner).changed.clone();
    loop {
        let next = compute(&lock(&inner), &state);
        if next != state.running_agents() {
            let _ = state.dispatch_chat(ChatCommand::Running { running: next });
        }
        tokio::select! {
            _ = chats.next_batch(64) => {},
            _ = changed.notified() => {},
        }
    }
}

fn compute(inner: &Inner, state: &Store) -> RunningAgents {
    let chats = state.chat_activity();
    let total = inner
        .owners
        .values()
        .filter(|owner| !*owner.done.borrow())
        .count() as u32;
    let running = inner
        .owners
        .iter()
        .filter(|(_, owner)| owner.live)
        .map(|(key, owner)| {
            let chat = owner
                .tab
                .and_then(|tab| chats.iter().find(|(chat, ..)| *chat == tab));
            let activity = match chat {
                None => AgentActivity::Stopping,
                Some((_, _, true)) => AgentActivity::Question,
                Some((_, true, _)) => AgentActivity::Turn,
                Some(_) => AgentActivity::Idle,
            };
            RunningAgent {
                profile_path: key.profile.to_string_lossy().into_owned(),
                name: key.name.clone(),
                tab: owner.tab,
                activity,
            }
        })
        .collect();
    RunningAgents { total, running }
}

//! Ember's persisted global hold and owner of the shared repository updater.
use crate::logging::Log;
use devora_core::config;
use devora_core::repo_update_config::{AppKind, Settings};
use devora_core::repo_updater::{PauseSource, RepoUpdater};
use devora_core::state::{feed, Feed, Subscription};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

struct PauseInner {
    paused: bool,
    subscribers: Vec<Feed<bool>>,
}

pub struct PauseState {
    path: PathBuf,
    inner: Mutex<PauseInner>,
}

impl PauseState {
    pub fn open(path: PathBuf) -> Result<Self, String> {
        let paused = match std::fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str::<bool>(&text)
                .map_err(|e| format!("failed to read {}: {e}", path.display()))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
            Err(e) => return Err(format!("failed to read {}: {e}", path.display())),
        };
        Ok(Self {
            path,
            inner: Mutex::new(PauseInner {
                paused,
                subscribers: Vec::new(),
            }),
        })
    }

    pub fn toggle(&self) -> Result<bool, String> {
        let mut inner = self.inner.lock().map_err(|e| e.to_string())?;
        let paused = !inner.paused;
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        config::write_pretty_json(&self.path, &serde_json::json!(paused))?;
        inner.paused = paused;
        inner.subscribers.retain(|feed| feed.push(&[paused]));
        Ok(paused)
    }
}

impl PauseSource for PauseState {
    fn paused(&self) -> bool {
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).paused
    }

    fn subscribe(&self) -> Subscription<bool> {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let (feed, subscription) = feed();
        feed.push(&[inner.paused]);
        inner.subscribers.push(feed);
        subscription
    }
}

pub struct Running {
    pub pause: Arc<PauseState>,
    pub updater: RepoUpdater,
    config_path: PathBuf,
}

impl Running {
    pub fn open(config_path: PathBuf, state_dir: PathBuf, log: Arc<Log>) -> Result<Self, String> {
        let pause = Arc::new(PauseState::open(state_dir.join("global-paused.json"))?);
        let settings = Settings::read(&config_path, AppKind::Ember)?;
        let updater = RepoUpdater::start(config_path.clone(), settings, pause.clone(), log)?;
        Ok(Self {
            pause,
            updater,
            config_path,
        })
    }

    pub fn configure(&self) -> Result<(), String> {
        self.updater
            .configure(Settings::read(&self.config_path, AppKind::Ember)?);
        Ok(())
    }
}

/// Retains startup errors so the normal frontend invoke path surfaces them.
pub struct Background(pub Result<Running, String>);

impl Background {
    pub fn running(&self) -> Result<&Running, String> {
        self.0.as_ref().map_err(Clone::clone)
    }

    pub fn shutdown(&self) {
        if let Ok(running) = &self.0 {
            running.updater.shutdown();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn hold_is_committed_before_publication_and_reopens() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("state/global-paused.json");
        let pause = PauseState::open(path.clone()).unwrap();
        let changes = pause.subscribe();
        assert_eq!(changes.next_batch(8).await, vec![false]);
        assert!(pause.toggle().unwrap());
        assert!(PauseState::open(path.clone()).unwrap().paused());
        assert!(pause.paused());
        assert!(!pause.toggle().unwrap());
        assert_eq!(changes.next_batch(8).await, vec![true, false]);
        assert!(!PauseState::open(path).unwrap().paused());
    }

    #[tokio::test]
    async fn failed_write_preserves_hold_and_publishes_nothing() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("global-paused.json");
        let pause = PauseState::open(path.clone()).unwrap();
        pause.toggle().unwrap();
        let changes = pause.subscribe();
        assert_eq!(changes.next_batch(8).await, vec![true]);
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
        assert!(pause.toggle().is_err());
        assert!(pause.paused());
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(10), changes.next_batch(8))
                .await
                .is_err()
        );
    }

    #[test]
    fn corrupt_state_is_an_error() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("global-paused.json");
        std::fs::write(&path, "garbage").unwrap();
        assert!(PauseState::open(path).is_err());
    }
}

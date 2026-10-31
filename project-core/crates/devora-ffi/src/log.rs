//! The app's log file and its error reporting (`devora_core::log`, ADR-002) across the boundary.
//!
//! An error reported on the Rust side reaches Swift through [`ErrorSubscription`], pulled in batches like the app state's events (ADR-010), rather than through a Swift callback.

use std::path::Path;
use std::sync::Arc;

use devora_core::log::{self, Level, Log};
use devora_core::state::{self as feeds, Subscription};

use crate::CoreError;

#[uniffi::remote(Enum)]
#[uniffi(name = "LogLevel")]
pub enum Level {
    Error,
    Warn,
}

/// The app's log: the one place both sides write to, and the one way to report an error (`report_error`).
#[derive(uniffi::Object)]
pub struct AppLog {
    log: Log,
    path: String,
    errors: Arc<ErrorSubscription>,
}

impl AppLog {
    fn open_at(path: &Path) -> Result<Arc<Self>, CoreError> {
        let log = Log::open(path)
            .map_err(|e| format!("couldn't open the log at {}: {e}", path.display()))?;
        let (feed, subscription) = feeds::feed();
        log.set_error_listener(Box::new(move |message| {
            feed.push(&[message.to_string()]);
        }));
        Ok(Arc::new(AppLog {
            log,
            path: path.to_string_lossy().into_owned(),
            errors: Arc::new(ErrorSubscription {
                inner: subscription,
            }),
        }))
    }

    /// Writes `message` as an error and reports it to the UI.
    pub(crate) fn report(&self, message: &str) {
        self.log.report_error(message);
    }

    pub(crate) fn warn(&self, message: &str) {
        self.log.write(Level::Warn, message);
    }
}

#[uniffi::export]
impl AppLog {
    /// This run's log, `/tmp/devora-sapphire-<timestamp>.log`: prints its path to stderr and writes every panic to it, as Ember does with its own.
    #[uniffi::constructor]
    pub fn open_session() -> Result<Arc<Self>, CoreError> {
        let path = log::session_log_path("devora-sapphire");
        eprintln!("Devora Sapphire log: {}", path.display());
        log::install_panic_hook(path.clone());
        Self::open_at(&path)
    }

    /// The log at `path`, without the panic hook: for tests.
    #[uniffi::constructor]
    pub fn open(path: String) -> Result<Arc<Self>, CoreError> {
        Self::open_at(Path::new(&path))
    }

    pub fn path(&self) -> Result<String, CoreError> {
        Ok(self.path.clone())
    }

    pub fn write(&self, level: Level, message: String) -> Result<(), CoreError> {
        self.log.write(level, &message);
        Ok(())
    }

    /// Writes `message` as an error, then queues it on [`AppLog::errors`] for the UI to show.
    pub fn report_error(&self, message: String) -> Result<(), CoreError> {
        self.report(&message);
        Ok(())
    }

    /// The reported errors, from the log's opening on: one stream per log, so errors reported before the UI pulls wait for it.
    /// Every call returns that same stream, for one consumer: once closed, it delivers nothing more.
    pub fn errors(&self) -> Result<Arc<ErrorSubscription>, CoreError> {
        Ok(self.errors.clone())
    }
}

/// The messages of the errors reported to an [`AppLog`], pulled in batches.
///
/// A pending `next_batch` can't be cancelled from Swift, so the consumer calls `close` to end it, for good.
#[derive(uniffi::Object)]
pub struct ErrorSubscription {
    inner: Subscription<String>,
}

#[uniffi::export]
impl ErrorSubscription {
    /// Waits for at least one error, then returns up to `max` of them; an empty batch means the subscription is closed.
    pub async fn next_batch(&self, max: u32) -> Result<Vec<String>, CoreError> {
        Ok(self.inner.next_batch(max).await)
    }

    /// Ends the subscription and wakes a pending `next_batch`, which then returns an empty batch.
    pub fn close(&self) -> Result<(), CoreError> {
        self.inner.close();
        Ok(())
    }
}

/// A log for a test that doesn't read its file: the file is removed at once and only written to.
#[cfg(test)]
pub(crate) fn test_log() -> Arc<AppLog> {
    let tmp = tempfile::tempdir().unwrap();
    AppLog::open(tmp.path().join("app.log").to_string_lossy().into_owned()).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_lite::future::block_on;

    #[test]
    fn a_reported_error_is_logged_then_pulled() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("app.log");
        let log = AppLog::open(path.to_string_lossy().into_owned()).unwrap();

        log.write(Level::Warn, "careful".into()).unwrap();
        log.report_error("boom".into()).unwrap();

        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("[WARN] careful"), "{contents}");
        assert!(contents.contains("[ERROR] boom"), "{contents}");
        let errors = log.errors().unwrap();
        assert_eq!(block_on(errors.next_batch(10)).unwrap(), ["boom"]);
        errors.close().unwrap();
        assert!(block_on(errors.next_batch(10)).unwrap().is_empty());
    }
}

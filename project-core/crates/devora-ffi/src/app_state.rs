//! The app state (`devora-state`, ADR-008) across the boundary: commands in, and events pulled in batches from a [`StateSubscription`].
//!
//! The `remote` items below redeclare `devora-state`'s types so UniFFI can lift and lower them as they are; a redeclaration that drifts from the original fails to compile.
//! Types whose Rust names collide with SwiftUI's (`Window`, `Tab`) or the Swift store's own (`Command`) are renamed for Swift.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use devora_core::bundle::AppVersion;
use devora_core::state::{self, AppState, Command, Event, Pane, Session, SessionKind, Tab, Window};

use crate::{CoreError, DevoraCore};

#[uniffi::remote(Record)]
pub struct AppState {
    pub window: Window,
    pub active_profile: Option<String>,
}

#[uniffi::remote(Record)]
#[uniffi(name = "AppWindow")]
pub struct Window {
    pub tabs: Vec<Tab>,
    pub active_tab: Option<u64>,
}

#[uniffi::remote(Record)]
#[uniffi(name = "AppTab")]
pub struct Tab {
    pub id: u64,
    pub title: String,
    pub session: Session,
    pub pane: Pane,
}

#[uniffi::remote(Record)]
#[uniffi(name = "AppSession")]
pub struct Session {
    pub kind: SessionKind,
    pub profile_path: Option<String>,
}

#[uniffi::remote(Enum)]
pub enum SessionKind {
    Shell,
    CreatingWorkspace,
    CreationFailed { reason: String },
    Workspace { path: String },
}

#[uniffi::remote(Enum)]
pub enum Pane {
    Terminal { id: u64 },
}

#[uniffi::remote(Enum)]
#[uniffi(name = "StateCommand")]
pub enum Command {
    OpenSession {
        title: String,
        kind: SessionKind,
        profile_path: Option<String>,
    },
    CloseSession {
        tab: u64,
    },
    ActivateTab {
        tab: u64,
    },
    ActivateNext,
    ActivatePrevious,
    MoveTab {
        tab: u64,
        index: u32,
    },
    SetTitle {
        tab: u64,
        title: String,
    },
    WorkspaceCreated {
        tab: u64,
        path: String,
    },
    WorkspaceCreationFailed {
        tab: u64,
        reason: String,
    },
    SetActiveProfile {
        profile_path: Option<String>,
    },
}

#[uniffi::remote(Enum)]
#[uniffi(name = "StateEvent")]
pub enum Event {
    Snapshot { state: AppState },
    SessionOpened { tab: Tab, index: u32 },
    SessionClosed { tab: u64 },
    TabActivated { tab: Option<u64> },
    TabMoved { tab: u64, from: u32, to: u32 },
    TitleChanged { tab: u64, title: String },
    SessionChanged { tab: u64, session: Session },
    ActiveProfileChanged { profile_path: Option<String> },
}

impl From<state::StateError> for CoreError {
    fn from(error: state::StateError) -> Self {
        error.to_string().into()
    }
}

/// Where Sapphire keeps its app state: `state.db` in `~/Library/Application Support/Devora Sapphire`, or in `Devora Sapphire <suffix>` for a bundle with an install suffix, so that side-by-side installs keep their own (Ember never shares it, ADR-008).
///
/// `DEVORA_STATE_DIR` replaces the directory, which is how tests keep their state apart.
/// `resources_dir` is the app bundle's `Resources` directory, where the install suffix is recorded.
#[uniffi::export]
pub fn sapphire_state_path(resources_dir: Option<String>) -> Result<String, CoreError> {
    let install_suffix =
        AppVersion::read(resources_dir.as_deref().map(Path::new), "").install_suffix;
    let path = state_path(
        std::env::var_os("DEVORA_STATE_DIR"),
        std::env::var_os("HOME"),
        install_suffix,
    )?;
    Ok(path.to_string_lossy().into_owned())
}

fn state_path(
    state_dir: Option<OsString>,
    home: Option<OsString>,
    install_suffix: Option<String>,
) -> Result<PathBuf, String> {
    let dir = match state_dir.filter(|dir| !dir.is_empty()) {
        Some(dir) => PathBuf::from(dir),
        None => {
            let home = home
                .filter(|home| !home.is_empty())
                .ok_or("HOME isn't set, so there is nowhere to keep the app state")?;
            let app = match install_suffix.filter(|suffix| !suffix.is_empty()) {
                Some(suffix) => format!("Devora Sapphire {suffix}"),
                None => "Devora Sapphire".to_string(),
            };
            Path::new(&home)
                .join("Library/Application Support")
                .join(app)
        }
    };
    Ok(dir.join("state.db"))
}

/// Moves the app state at `path` aside, keeping it beside `path` under a timestamped name, so that a fresh one can open in its place; returns where it went.
///
/// Only for a state that failed to open with `StateUnreadable`: one in use belongs to another process.
#[uniffi::export]
pub fn set_aside_state(path: String) -> Result<String, CoreError> {
    let aside = state::set_aside(Path::new(&path))?;
    Ok(aside.to_string_lossy().into_owned())
}

/// Opens the app state at `path`, telling a state held by another process from one that can't be read.
pub(crate) fn open(path: String) -> Result<state::Store, CoreError> {
    state::Store::open(&path).map_err(|error| match error {
        state::StateError::InUse { message } => CoreError::StateInUse { path, message },
        other => CoreError::StateUnreadable {
            path,
            message: other.to_string(),
        },
    })
}

/// A stream of the app state's events, pulled in batches; its first event is a `Snapshot`.
///
/// A pending `next_batch` can't be cancelled from Swift, so the consumer calls `close` to end it.
#[derive(uniffi::Object)]
pub struct StateSubscription {
    inner: state::Subscription,
}

#[uniffi::export]
impl StateSubscription {
    /// Waits for at least one event, then returns up to `max` of them; an empty batch means the subscription is closed.
    pub async fn next_batch(&self, max: u32) -> Result<Vec<Event>, CoreError> {
        Ok(self.inner.next_batch(max).await)
    }

    /// Ends the subscription and wakes a pending `next_batch`, which then returns an empty batch.
    pub fn close(&self) -> Result<(), CoreError> {
        self.inner.close();
        Ok(())
    }
}

#[uniffi::export]
impl DevoraCore {
    /// Applies a command to the app state; its events reach every subscription.
    pub fn dispatch(&self, command: Command) -> Result<(), CoreError> {
        self.state.dispatch(command)?;
        Ok(())
    }

    pub fn snapshot(&self) -> Result<AppState, CoreError> {
        Ok(self.state.snapshot())
    }

    pub fn subscribe(&self) -> Result<Arc<StateSubscription>, CoreError> {
        Ok(Arc::new(StateSubscription {
            inner: self.state.subscribe(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_lite::future::block_on;

    fn open_shell() -> Command {
        Command::OpenSession {
            title: "Shell".into(),
            kind: SessionKind::Shell,
            profile_path: None,
        }
    }

    #[test]
    fn a_subscription_delivers_a_snapshot_then_the_events_of_each_command() {
        let core = DevoraCore::new(Some("/nonexistent/config.json".into()), None).unwrap();
        core.dispatch(open_shell()).unwrap();
        let subscription = core.subscribe().unwrap();
        core.dispatch(open_shell()).unwrap();

        let batch = block_on(subscription.next_batch(100)).unwrap();

        let kinds: Vec<_> = batch
            .iter()
            .map(|event| match event {
                Event::Snapshot { state } => {
                    format!("snapshot of {} tabs", state.window.tabs.len())
                }
                Event::SessionOpened { index, .. } => format!("opened at {index}"),
                Event::TabActivated { .. } => "activated".into(),
                other => format!("{other:?}"),
            })
            .collect();
        assert_eq!(kinds, ["snapshot of 1 tabs", "opened at 1", "activated"]);
    }

    #[test]
    fn a_rejected_command_is_an_error() {
        let core = DevoraCore::new(Some("/nonexistent/config.json".into()), None).unwrap();

        let error = core
            .dispatch(Command::CloseSession { tab: 42 })
            .unwrap_err();

        assert_eq!(error.to_string(), "no tab with id 42");
    }

    #[test]
    fn the_state_lives_under_application_support_with_the_install_suffix() {
        let home = Some(OsString::from("/Users/me"));
        assert_eq!(
            state_path(None, home.clone(), None).unwrap(),
            Path::new("/Users/me/Library/Application Support/Devora Sapphire/state.db")
        );
        assert_eq!(
            state_path(None, home.clone(), Some("beta".into())).unwrap(),
            Path::new("/Users/me/Library/Application Support/Devora Sapphire beta/state.db")
        );
        assert_eq!(
            state_path(Some("".into()), home, Some("".into())).unwrap(),
            Path::new("/Users/me/Library/Application Support/Devora Sapphire/state.db"),
            "empty values count as unset"
        );
    }

    #[test]
    fn the_state_dir_override_replaces_the_directory_whatever_the_suffix() {
        assert_eq!(
            state_path(Some("/tmp/scenario".into()), None, Some("beta".into())).unwrap(),
            Path::new("/tmp/scenario/state.db")
        );
    }

    #[test]
    fn without_a_home_there_is_no_state_path() {
        let error = state_path(None, None, None).unwrap_err();
        assert!(error.contains("HOME"), "{error}");
    }

    fn persistent_core(path: &Path) -> Result<Arc<DevoraCore>, CoreError> {
        DevoraCore::new(
            Some("/nonexistent/config.json".into()),
            Some(path.to_string_lossy().into_owned()),
        )
    }

    #[test]
    fn a_persistent_core_restores_its_tabs() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("Devora Sapphire").join("state.db");
        let core = persistent_core(&path).unwrap();
        core.dispatch(open_shell()).unwrap();
        core.dispatch(open_shell()).unwrap();
        let left = core.snapshot().unwrap();
        drop(core);

        assert_eq!(persistent_core(&path).unwrap().snapshot().unwrap(), left);
    }

    #[test]
    fn a_state_held_by_another_core_is_in_use() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("state.db");
        let _first = persistent_core(&path).unwrap();

        let Err(error) = persistent_core(&path) else {
            panic!("a second core opened the same state");
        };

        let CoreError::StateInUse { path: reported, .. } = error else {
            panic!("expected StateInUse, got {error:?}");
        };
        assert_eq!(reported, path.to_string_lossy());
    }

    #[test]
    fn an_unreadable_state_can_be_set_aside_for_a_fresh_one() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("state.db");
        std::fs::write(&path, vec![0x5a; 8192]).unwrap();

        let Err(error) = persistent_core(&path) else {
            panic!("a corrupt state opened");
        };
        let CoreError::StateUnreadable {
            path: reported,
            message,
        } = error
        else {
            panic!("expected StateUnreadable, got {error:?}");
        };
        assert!(message.contains("couldn't open the app state"), "{message}");

        let aside = set_aside_state(reported).unwrap();

        assert_eq!(std::fs::read(&aside).unwrap(), vec![0x5a; 8192]);
        let core = persistent_core(&path).unwrap();
        assert!(core.snapshot().unwrap().window.tabs.is_empty());
    }
}

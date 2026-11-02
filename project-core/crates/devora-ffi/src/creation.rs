//! Task creation (`devora_core::workspace_creation`) across the boundary: a creation runs on a worker thread in a tab of its own, and streams its progress through a [`CreationSubscription`].
//!
//! The core owns the creation's whole course in the app state: it opens the tab as [`SessionKind::CreatingWorkspace`], and when the work ends records the workspace (`WorkspaceCreated`), the failure (`WorkspaceCreationFailed`), or closes the tab after a cancel.
//! The UI only shows the progress and asks for a cancel.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use devora_core::state::{self as feeds, Command, Event, SessionKind, StateError, Subscription};
use devora_core::workspace::{self, CreatedWorkspace, RepoInfo};
use devora_core::workspace_creation::{
    self as creation, CreationEvent, CreationHandle, EventSink, Outcome, WorkspaceCreationManager,
};

use crate::{CoreError, DevoraCore};

#[uniffi::remote(Record)]
pub struct RepoInfo {
    pub name: String,
    pub path: String,
    /// `registered` (listed in the profile's config) or `auto-discovered` (under `<profile>/repos/`).
    pub source: String,
}

#[uniffi::remote(Record)]
pub struct CreatedWorkspace {
    pub path: String,
    /// The workspace's directory name (`ws-N`).
    pub name: String,
}

#[uniffi::remote(Enum)]
pub enum CreationEvent {
    Step { label: String },
    Log { line: String },
    Done { workspace: CreatedWorkspace },
    Failed { message: String },
    Cancelled,
}

/// A creation just started: what cancels it, the tab it runs in, and its progress.
#[derive(uniffi::Record)]
pub struct WorkspaceCreation {
    /// For [`DevoraCore::cancel_workspace_creation`].
    pub id: u32,
    pub tab: u64,
    pub events: Arc<CreationSubscription>,
}

/// A creation's progress, pulled in batches: steps and output lines, then one last event (`Done`, `Failed` or `Cancelled`), sent once the app state has recorded the outcome.
///
/// A pending `next_batch` can't be cancelled from Swift, so the consumer calls `close` to end it.
#[derive(uniffi::Object)]
pub struct CreationSubscription {
    inner: Subscription<CreationEvent>,
}

#[uniffi::export]
impl CreationSubscription {
    /// Waits for at least one event, then returns up to `max` of them; an empty batch means the subscription is closed.
    pub async fn next_batch(&self, max: u32) -> Result<Vec<CreationEvent>, CoreError> {
        Ok(self.inner.next_batch(max).await)
    }

    /// Ends the subscription and wakes a pending `next_batch`, which then returns an empty batch; the creation goes on.
    pub fn close(&self) -> Result<(), CoreError> {
        self.inner.close();
        Ok(())
    }
}

/// The creations in flight, by id, for cancelling.
pub(crate) type Creations = Mutex<WorkspaceCreationManager>;

fn lock(creations: &Creations) -> MutexGuard<'_, WorkspaceCreationManager> {
    creations.lock().unwrap_or_else(PoisonError::into_inner)
}

#[uniffi::export]
impl DevoraCore {
    /// The repos a task in the profile can use: those its config lists, then those under `<profile>/repos/`.
    pub fn get_registered_repos(&self, profile_path: String) -> Result<Vec<RepoInfo>, CoreError> {
        Ok(workspace::get_registered_repos(&profile_path)?)
    }

    /// Opens a tab named `task_name` creating a workspace for it, and starts the creation on a worker thread: it reuses an inactive workspace with exactly these repos, or makes a fresh one.
    ///
    /// With `source_workspace_path` (duplicating a task), the repos the source has are pinned to its commits and its `CLAUDE.md` is copied.
    /// Warnings and a failure are reported to the log, as Ember reports them.
    pub fn create_workspace(
        self: Arc<Self>,
        profile_path: String,
        repo_paths: Vec<String>,
        task_name: String,
        source_workspace_path: Option<String>,
    ) -> Result<WorkspaceCreation, CoreError> {
        let opened = self.state.dispatch(Command::OpenSession {
            title: task_name.clone(),
            kind: SessionKind::CreatingWorkspace,
            profile_path: Some(profile_path.clone()),
        })?;
        let tab = opened
            .iter()
            .find_map(|event| match event {
                Event::SessionOpened { tab, .. } => Some(tab.id),
                _ => None,
            })
            .ok_or_else(|| "opening the task's tab opened no tab".to_string())?;
        let (id, events) = self.start_creation(move |core, handle, sink| {
            let mut warnings = Vec::new();
            let result = creation::create_with_claim(
                handle,
                &profile_path,
                &repo_paths,
                &task_name,
                source_workspace_path.as_deref(),
                sink,
                &mut warnings,
                &|descriptor| {
                    let encoded = descriptor.encode()?;
                    core.state
                        .record_creation(tab, &encoded)
                        .map_err(|e| e.to_string())
                },
            );
            core.finish_creation(tab, result, &warnings)
        });
        Ok(WorkspaceCreation { id, tab, events })
    }

    /// Asks a creation to stop: the process it runs is killed, and its work undone (a fresh workspace removed, a reused one made inactive again, keeping its caches; an added worktree or a clone removed).
    /// It stops the repo operations (`add_repo_to_workspace`, `clone_repo_into_profile`) too, whose ids are a creation's.
    /// A creation that has already ended is left alone.
    pub fn cancel_workspace_creation(&self, id: u32) -> Result<(), CoreError> {
        lock(&self.creations).cancel(id);
        Ok(())
    }
}

impl DevoraCore {
    /// Registers a creation, for cancelling, and runs `work` on a worker thread: its progress streams as `work` sends it, then the last event `work` returns, once the creation is deregistered.
    pub(crate) fn start_creation(
        self: &Arc<Self>,
        work: impl FnOnce(&DevoraCore, &CreationHandle, &EventSink) -> CreationEvent + Send + 'static,
    ) -> (u32, Arc<CreationSubscription>) {
        let (id, handle) = lock(&self.creations).register();
        let (feed, subscription) = feeds::feed();
        let feed = Arc::new(feed);
        let sink: EventSink = {
            let feed = feed.clone();
            Arc::new(move |event| {
                feed.push(&[event]);
            })
        };
        let core = self.clone();
        std::thread::spawn(move || {
            let last = work(&core, &handle, &sink);
            lock(&core.creations).remove(id);
            feed.push(&[last]);
        });
        let events = Arc::new(CreationSubscription {
            inner: subscription,
        });
        (id, events)
    }

    /// Records how a creation ended in the app state, and returns its last event.
    fn finish_creation(
        &self,
        tab: u64,
        result: Result<Outcome, String>,
        warnings: &[String],
    ) -> CreationEvent {
        for warning in warnings {
            self.log.report(warning);
        }
        let (command, last) = match result {
            Ok(Outcome::Done(workspace)) => (
                Command::WorkspaceCreated {
                    tab,
                    path: workspace.path.clone(),
                },
                CreationEvent::Done { workspace },
            ),
            Ok(Outcome::Cancelled) => (Command::CloseSession { tab }, CreationEvent::Cancelled),
            Err(message) => {
                self.log.report(&message);
                (
                    Command::WorkspaceCreationFailed {
                        tab,
                        reason: message.clone(),
                    },
                    CreationEvent::Failed { message },
                )
            }
        };
        match self.state.dispatch(command) {
            // The user closed the tab, which cancels its creation.
            Ok(_) | Err(StateError::UnknownTab { .. }) => {}
            Err(error) => self
                .log
                .report(&format!("couldn't record how task creation ended: {error}")),
        }
        if let Err(error) = self.state.clear_creation(tab) {
            self.log
                .warn(&format!("creation recovery remains pending: {error}"));
        }
        last
    }
}

pub(crate) fn recover_pending(state: &feeds::Store, log: &crate::AppLog) -> Result<(), CoreError> {
    for (tab, encoded) in state.pending_creations()? {
        let descriptor = match creation::RecoveryDescriptor::decode(&encoded) {
            Ok(descriptor) => descriptor,
            Err(error) => {
                log.warn(&format!("invalid creation recovery record: {error}"));
                continue;
            }
        };
        match creation::recover(&descriptor) {
            Ok(warning) => {
                if let Some(warning) = warning {
                    log.warn(&warning);
                }
                state.clear_creation(tab)?;
            }
            Err(error) => log.warn(&format!("creation recovery remains pending: {error}")),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{core, label, profile, pull_all, Profile};
    use devora_core::state::{AppState, Pane, Tab};
    use futures_lite::future::block_on;
    use std::path::Path;

    fn start(core: &Arc<DevoraCore>, profile: &Profile, repos: &[&Path]) -> WorkspaceCreation {
        core.clone()
            .create_workspace(
                profile.path.to_string_lossy().into_owned(),
                repos
                    .iter()
                    .map(|repo| repo.to_string_lossy().into_owned())
                    .collect(),
                "Fix login".into(),
                None,
            )
            .unwrap()
    }

    fn tab(core: &DevoraCore, id: u64) -> Option<Tab> {
        let AppState { window, .. } = core.snapshot().unwrap();
        window.tabs.into_iter().find(|tab| tab.id == id)
    }

    #[test]
    fn a_creation_opens_its_tab_then_streams_its_steps_and_records_the_workspace() {
        let profile = profile(Some("echo prepared"));
        let (core, _log) = core();

        let creation = start(&core, &profile, &[&profile.repo]);
        let opened = tab(&core, creation.tab).unwrap();
        assert_eq!(opened.title, "Fix login");
        assert_eq!(opened.session.kind, SessionKind::CreatingWorkspace);
        let events = pull_all(&creation.events, |_| {});

        // `mise trust` is a step only where mise is installed.
        let steps: Vec<_> = events
            .iter()
            .filter_map(label)
            .filter(|label| !label.starts_with("Trusting mise"))
            .collect();
        assert_eq!(
            steps,
            [
                "Resolving workspace",
                "Fetching repo",
                "Creating worktree repo",
                "Preparing repo"
            ]
        );
        assert!(events.contains(&CreationEvent::Log {
            line: "prepared".into()
        }));
        let workspace = profile.workspace("ws-1");
        assert_eq!(
            events.last(),
            Some(&CreationEvent::Done {
                workspace: CreatedWorkspace {
                    path: workspace.to_string_lossy().into_owned(),
                    name: "ws-1".into()
                }
            })
        );
        // The state records the workspace before the last event goes out.
        assert_eq!(
            tab(&core, creation.tab).unwrap().session.kind,
            SessionKind::Workspace {
                path: workspace.to_string_lossy().into_owned()
            }
        );
        let Pane::Terminal { id } = opened.pane;
        assert_eq!(
            core.terminal_spec(id, None).unwrap().cwd,
            workspace.join("repo").to_string_lossy()
        );
    }

    #[test]
    fn a_cancel_asked_for_at_once_closes_the_tab_and_removes_the_fresh_workspace() {
        let profile = profile(Some("sleep 30"));
        let (core, _log) = core();

        let creation = start(&core, &profile, &[&profile.repo]);
        core.cancel_workspace_creation(creation.id).unwrap();
        let events = pull_all(&creation.events, |_| {});

        assert_eq!(events.last(), Some(&CreationEvent::Cancelled));
        assert_eq!(tab(&core, creation.tab), None);
        assert!(!profile.workspace("ws-1").exists());
    }

    #[test]
    fn a_cancel_while_preparing_kills_the_prepare_command_and_undoes_the_work() {
        let profile = profile(Some("sleep 30"));
        let (core, _log) = core();
        let started = std::time::Instant::now();

        let creation = start(&core, &profile, &[&profile.repo]);
        let events = pull_all(&creation.events, |event| {
            if label(event).is_some_and(|label| label.starts_with("Preparing")) {
                core.cancel_workspace_creation(creation.id).unwrap();
            }
        });

        assert_eq!(events.last(), Some(&CreationEvent::Cancelled));
        assert!(started.elapsed() < std::time::Duration::from_secs(20));
        assert_eq!(tab(&core, creation.tab), None);
        assert!(!profile.workspace("ws-1").exists());
        // Cancelling a creation that has ended is harmless.
        core.cancel_workspace_creation(creation.id).unwrap();
    }

    #[test]
    fn a_failure_is_recorded_on_the_tab_and_reported() {
        let profile = profile(None);
        let (core, log) = core();
        let missing = profile.path.join("repos").join("missing");

        let creation = start(&core, &profile, &[&missing]);
        let events = pull_all(&creation.events, |_| {});

        let Some(CreationEvent::Failed { message }) = events.last() else {
            panic!("expected a failure, got {events:?}");
        };
        assert_eq!(
            tab(&core, creation.tab).unwrap().session.kind,
            SessionKind::CreationFailed {
                reason: message.clone()
            }
        );
        let reported = block_on(log.errors().unwrap().next_batch(10)).unwrap();
        assert_eq!(reported, std::slice::from_ref(message));
    }

    #[test]
    fn a_creation_whose_tab_was_closed_ends_quietly() {
        let profile = profile(Some("sleep 30"));
        let (core, log) = core();

        let creation = start(&core, &profile, &[&profile.repo]);
        core.dispatch(Command::CloseSession { tab: creation.tab })
            .unwrap();
        core.cancel_workspace_creation(creation.id).unwrap();
        let events = pull_all(&creation.events, |_| {});

        assert_eq!(events.last(), Some(&CreationEvent::Cancelled));
        log.report_error("the only error".into()).unwrap();
        let reported = block_on(log.errors().unwrap().next_batch(10)).unwrap();
        assert_eq!(reported, ["the only error"]);
    }

    #[test]
    fn lists_the_profiles_repos() {
        let profile = profile(None);
        let (core, _log) = core();

        let repos = core
            .get_registered_repos(profile.path.to_string_lossy().into_owned())
            .unwrap();

        assert_eq!(
            repos,
            [RepoInfo {
                name: "repo".into(),
                path: profile.repo.to_string_lossy().into_owned(),
                source: "auto-discovered".into(),
            }]
        );
    }
}

#[cfg(test)]
mod recovery_tests {
    use super::*;
    use crate::test_support::{interrupted_creation, profile};
    use std::fs;

    #[test]
    fn restore_cleans_fresh_and_reused_creations_and_keeps_the_failed_tab() {
        for fresh in [true, false] {
            let profile = profile(None);
            let (path, ws) = interrupted_creation(&profile, fresh);
            let log = crate::log::test_log();
            let core = DevoraCore::new(
                Some("/nonexistent/config.json".into()),
                Some(path.to_string_lossy().into()),
                log,
            )
            .unwrap();
            assert!(matches!(
                core.state.snapshot().window.tabs[0].session.kind,
                SessionKind::CreationFailed { .. }
            ));
            assert!(core.state.pending_creations().unwrap().is_empty());
            if fresh {
                assert!(!ws.exists());
            } else {
                assert!(ws.join("repo/file.txt").exists());
                assert!(ws.join("initialized").exists());
                assert!(!ws.join("task.json").exists());
            }
        }
    }

    #[test]
    fn restore_leaves_completed_and_reclaimed_workspaces_untouched() {
        for (fresh, completed) in [(true, true), (false, true), (false, false)] {
            let profile = profile(None);
            let (path, ws) = interrupted_creation(&profile, fresh);
            fs::write(ws.join("initialized"), "").unwrap();
            let claim = if completed {
                r#"{"uid":"finished","title":"complete"}"#
            } else {
                r#"{"creation_token":"someone-else"}"#
            };
            fs::write(ws.join("task.json"), claim).unwrap();
            let core = DevoraCore::new(
                Some("/nonexistent/config.json".into()),
                Some(path.to_string_lossy().into()),
                crate::log::test_log(),
            )
            .unwrap();
            assert!(ws.join("repo/file.txt").exists());
            assert_eq!(fs::read_to_string(ws.join("task.json")).unwrap(), claim);
            assert!(core.state.pending_creations().unwrap().is_empty());
        }
    }

    #[test]
    fn normal_success_clears_the_journal_without_a_warning() {
        let profile = profile(None);
        let path = profile.root.path().join("success.db");
        let log = crate::AppLog::open(profile.root.path().join("app.log").to_string_lossy().into())
            .unwrap();
        let core = DevoraCore::new(
            Some("/nonexistent/config.json".into()),
            Some(path.to_string_lossy().into()),
            log.clone(),
        )
        .unwrap();
        let creation = core
            .clone()
            .create_workspace(
                profile.path_string(),
                vec![profile.repo.to_string_lossy().into()],
                "task".into(),
                None,
            )
            .unwrap();
        let events = crate::test_support::pull_all(&creation.events, |_| {});
        assert!(matches!(events.last(), Some(CreationEvent::Done { .. })));
        assert!(core.state.pending_creations().unwrap().is_empty());
        let contents = std::fs::read_to_string(log.path().unwrap()).unwrap();
        assert!(!contents.contains("[WARN]"), "{contents}");
    }

    #[test]
    fn an_old_creation_with_no_journal_entry_is_a_safe_no_op() {
        let profile = profile(None);
        let (path, ws) = interrupted_creation(&profile, true);
        let store = feeds::Store::open(&path).unwrap();
        for (id, _) in store.pending_creations().unwrap() {
            store.clear_creation(id).unwrap();
        }
        drop(store);
        let core = DevoraCore::new(
            Some("/nonexistent/config.json".into()),
            Some(path.to_string_lossy().into()),
            crate::log::test_log(),
        )
        .unwrap();
        assert!(ws.join("repo/file.txt").exists());
        assert!(core.state.pending_creations().unwrap().is_empty());
        assert!(matches!(
            core.state.snapshot().window.tabs[0].session.kind,
            SessionKind::CreationFailed { .. }
        ));
    }

    #[test]
    fn unreadable_claim_is_retried_on_next_restore() {
        let profile = profile(None);
        let (path, ws) = interrupted_creation(&profile, true);
        fs::write(ws.join("task.json"), "invalid json").unwrap();
        let reopen = || {
            DevoraCore::new(
                Some("/nonexistent/config.json".into()),
                Some(path.to_string_lossy().into()),
                crate::log::test_log(),
            )
            .unwrap()
        };
        let core = reopen();
        assert_eq!(core.state.pending_creations().unwrap().len(), 1);
        drop(core);
        fs::write(ws.join("task.json"), r#"{"creation_token":"test-token"}"#).unwrap();
        let core = reopen();
        assert!(!ws.exists());
        assert!(core.state.pending_creations().unwrap().is_empty());
    }
}

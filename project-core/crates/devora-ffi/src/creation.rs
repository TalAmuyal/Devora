//! Task creation (`devora_core::workspace_creation`) across the boundary: a creation runs on a worker thread in a tab of its own, and streams its progress through a [`CreationSubscription`].
//!
//! The core owns the creation's whole course in the app state: it opens the tab as [`SessionKind::CreatingWorkspace`], and when the work ends records the workspace (`WorkspaceCreated`), the failure (`WorkspaceCreationFailed`), or closes the tab after a cancel.
//! The UI only shows the progress and asks for a cancel.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use devora_core::state::{self as feeds, Command, Event, SessionKind, StateError, Subscription};
use devora_core::workspace::{self, CreatedWorkspace, RepoInfo};
use devora_core::workspace_creation::{
    self as creation, CreationEvent, EventSink, Outcome, WorkspaceCreationManager,
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
            let mut warnings = Vec::new();
            let result = creation::create(
                &handle,
                &profile_path,
                &repo_paths,
                &task_name,
                source_workspace_path.as_deref(),
                &sink,
                &mut warnings,
            );
            let last = core.finish_creation(id, tab, result, &warnings);
            feed.push(&[last]);
        });
        Ok(WorkspaceCreation {
            id,
            tab,
            events: Arc::new(CreationSubscription {
                inner: subscription,
            }),
        })
    }

    /// Asks a creation to stop: the process it runs is killed, and its work undone (a fresh workspace removed, a reused one made inactive again, keeping its caches).
    /// A creation that has already ended is left alone.
    pub fn cancel_workspace_creation(&self, id: u32) -> Result<(), CoreError> {
        lock(&self.creations).cancel(id);
        Ok(())
    }
}

impl DevoraCore {
    /// Records how a creation ended in the app state, and returns its last event.
    fn finish_creation(
        &self,
        id: u32,
        tab: u64,
        result: Result<Outcome, String>,
        warnings: &[String],
    ) -> CreationEvent {
        lock(&self.creations).remove(id);
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
        last
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use devora_core::state::{AppState, Pane, Tab};
    use futures_lite::future::block_on;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command as Process;

    fn git(dir: &Path, args: &[&str]) {
        let status = Process::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@t"])
            .args(args)
            .current_dir(dir)
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    }

    struct Profile {
        _root: tempfile::TempDir,
        path: PathBuf,
        repo: PathBuf,
    }

    impl Profile {
        fn workspace(&self, id: &str) -> PathBuf {
            self.path.join("workspaces").join(id)
        }
    }

    /// A profile whose one repo, `repo`, is a clone of a bare origin with a commit on `main`; `prepare` is its prepare-command.
    fn profile(prepare: Option<&str>) -> Profile {
        let root = tempfile::tempdir().unwrap();
        let origin = root.path().join("repo.git");
        fs::create_dir_all(&origin).unwrap();
        git(&origin, &["init", "-q", "--bare", "-b", "main"]);
        let path = root.path().join("work");
        fs::create_dir_all(path.join("repos")).unwrap();
        fs::create_dir_all(path.join("workspaces")).unwrap();
        let config = match prepare {
            Some(command) => format!(r#"{{"name": "work", "prepare-command": {command:?}}}"#),
            None => r#"{"name": "work"}"#.to_string(),
        };
        fs::write(path.join("config.json"), config).unwrap();
        let repo = path.join("repos").join("repo");
        fs::create_dir_all(&repo).unwrap();
        git(&repo, &["init", "-q", "-b", "main"]);
        fs::write(repo.join("file.txt"), "v1\n").unwrap();
        git(&repo, &["add", "."]);
        git(&repo, &["commit", "-q", "-m", "c1"]);
        git(
            &repo,
            &["remote", "add", "origin", origin.to_str().unwrap()],
        );
        git(&repo, &["push", "-q", "-u", "origin", "main"]);
        Profile {
            _root: root,
            path,
            repo,
        }
    }

    fn core() -> (Arc<DevoraCore>, Arc<crate::AppLog>) {
        let log = crate::log::test_log();
        let core =
            DevoraCore::new(Some("/nonexistent/config.json".into()), None, log.clone()).unwrap();
        (core, log)
    }

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

    fn is_last(event: &CreationEvent) -> bool {
        matches!(
            event,
            CreationEvent::Done { .. } | CreationEvent::Failed { .. } | CreationEvent::Cancelled
        )
    }

    /// Pulls every event up to the last one; `on_event` sees each as it comes.
    fn pull_all(
        creation: &WorkspaceCreation,
        mut on_event: impl FnMut(&CreationEvent),
    ) -> Vec<CreationEvent> {
        let mut events = Vec::new();
        while !events.last().is_some_and(is_last) {
            let batch = block_on(creation.events.next_batch(100)).unwrap();
            assert!(!batch.is_empty(), "the stream ended without a last event");
            for event in batch {
                on_event(&event);
                events.push(event);
            }
        }
        events
    }

    fn tab(core: &DevoraCore, id: u64) -> Option<Tab> {
        let AppState { window, .. } = core.snapshot().unwrap();
        window.tabs.into_iter().find(|tab| tab.id == id)
    }

    fn label(event: &CreationEvent) -> Option<&str> {
        match event {
            CreationEvent::Step { label } => Some(label),
            _ => None,
        }
    }

    #[test]
    fn a_creation_opens_its_tab_then_streams_its_steps_and_records_the_workspace() {
        let profile = profile(Some("echo prepared"));
        let (core, _log) = core();

        let creation = start(&core, &profile, &[&profile.repo]);
        let opened = tab(&core, creation.tab).unwrap();
        assert_eq!(opened.title, "Fix login");
        assert_eq!(opened.session.kind, SessionKind::CreatingWorkspace);
        let events = pull_all(&creation, |_| {});

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
        let events = pull_all(&creation, |_| {});

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
        let events = pull_all(&creation, |event| {
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
        let events = pull_all(&creation, |_| {});

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
        let events = pull_all(&creation, |_| {});

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

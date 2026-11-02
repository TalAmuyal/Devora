//! Fixtures for the tests of the streamed operations (task creation, Add Repo, Clone Repo): profiles over real git repos, and pulling an operation's events.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as Process;
use std::sync::Arc;

use devora_core::workspace_creation::CreationEvent;
use futures_lite::future::block_on;

use crate::creation::CreationSubscription;
use crate::{AppLog, DevoraCore};

pub(crate) fn git(dir: &Path, args: &[&str]) {
    let status = Process::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@t"])
        .args(args)
        .current_dir(dir)
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?}");
}

pub(crate) struct Profile {
    pub(crate) root: tempfile::TempDir,
    pub(crate) path: PathBuf,
    pub(crate) repo: PathBuf,
}

impl Profile {
    pub(crate) fn workspace(&self, id: &str) -> PathBuf {
        self.path.join("workspaces").join(id)
    }

    pub(crate) fn path_string(&self) -> String {
        self.path.to_string_lossy().into_owned()
    }
}

/// A profile whose one repo, `repo`, is a clone of a bare origin, `<root>/repo.git`, with a commit on `main`; `prepare` is its prepare-command.
pub(crate) fn profile(prepare: Option<&str>) -> Profile {
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
    Profile { root, path, repo }
}

pub(crate) fn core() -> (Arc<DevoraCore>, Arc<AppLog>) {
    let log = crate::log::test_log();
    let core = DevoraCore::new(Some("/nonexistent/config.json".into()), None, log.clone()).unwrap();
    (core, log)
}

fn is_last(event: &CreationEvent) -> bool {
    matches!(
        event,
        CreationEvent::Done { .. } | CreationEvent::Failed { .. } | CreationEvent::Cancelled
    )
}

/// Pulls every event up to the last one; `on_event` sees each as it comes.
pub(crate) fn pull_all(
    events: &CreationSubscription,
    mut on_event: impl FnMut(&CreationEvent),
) -> Vec<CreationEvent> {
    let mut pulled = Vec::new();
    while !pulled.last().is_some_and(is_last) {
        let batch = block_on(events.next_batch(100)).unwrap();
        assert!(!batch.is_empty(), "the stream ended without a last event");
        for event in batch {
            on_event(&event);
            pulled.push(event);
        }
    }
    pulled
}

pub(crate) fn label(event: &CreationEvent) -> Option<&str> {
    match event {
        CreationEvent::Step { label } => Some(label),
        _ => None,
    }
}

/// A persisted interrupted creation over a real cached worktree.
pub(crate) fn interrupted_creation(profile: &Profile, fresh: bool) -> (PathBuf, PathBuf) {
    use devora_core::state::{Command, SessionKind, Store};
    let ws = profile.workspace("ws-1");
    fs::create_dir_all(&ws).unwrap();
    git(
        &profile.repo,
        &[
            "worktree",
            "add",
            "--detach",
            ws.join("repo").to_str().unwrap(),
            "HEAD",
        ],
    );
    if !fresh {
        fs::write(ws.join("initialized"), "").unwrap();
    }
    fs::write(
        ws.join("task.json"),
        r#"{"creation_token":"test-token","title":"unfinished"}"#,
    )
    .unwrap();
    let path = profile.root.path().join("state.db");
    let store = Store::open(&path).unwrap();
    store
        .dispatch(Command::OpenSession {
            title: "unfinished".into(),
            kind: SessionKind::CreatingWorkspace,
            profile_path: Some(profile.path_string()),
        })
        .unwrap();
    let tab = store.snapshot().window.tabs[0].id;
    let descriptor = format!(
        r#"{{"path":{:?},"repos":["repo"],"fresh":{},"token":"test-token"}}"#,
        ws.to_str().unwrap(),
        fresh
    );
    store.record_creation(tab, &descriptor).unwrap();
    (path, ws)
}

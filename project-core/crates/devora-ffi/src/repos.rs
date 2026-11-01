//! The repo operations across the boundary: adding a repo to a workspace (`creation::add_repo`) and cloning one into a profile (`creation::clone_repo`).
//!
//! Each runs on a worker thread as a creation does, cancelled through `cancel_workspace_creation`, and streams its progress through a [`CreationSubscription`]; unlike a creation, it opens no tab and leaves the app state alone, so the UI shows its progress in the dialog that started it.

use std::path::Path;
use std::sync::Arc;

use devora_core::config;
use devora_core::repo_clone;
use devora_core::workspace_creation::{self as creation, CreationEvent, Outcome};

use crate::creation::CreationSubscription;
use crate::{CoreError, DevoraCore};

/// A repo operation just started: what cancels it, and its progress.
#[derive(uniffi::Record)]
pub struct RepoOperation {
    /// For [`DevoraCore::cancel_workspace_creation`].
    pub id: u32,
    /// Steps and output lines, then one last event: `Done` (whose workspace is the workspace added to, or the clone), `Failed` (also reported to the log) or `Cancelled` (once the work is undone).
    pub events: Arc<CreationSubscription>,
}

#[uniffi::export]
impl DevoraCore {
    /// Adds a worktree of the repo at `source_repo_path` to the workspace, as `worktree_dir_name` (`<repo>` or `<repo>-<postfix>`), then runs the prepare-command of the workspace's profile and ensures the workspace's `CLAUDE.md`.
    /// A name that exists in the workspace or leaves it fails the operation; a cancel removes the worktree, leaving the workspace as it was.
    pub fn add_repo_to_workspace(
        self: Arc<Self>,
        workspace_path: String,
        source_repo_path: String,
        worktree_dir_name: String,
    ) -> Result<RepoOperation, CoreError> {
        let (id, events) = self.start_creation(move |core, handle, sink| {
            let mut warnings = Vec::new();
            let result = creation::add_repo(
                handle,
                &workspace_path,
                &source_repo_path,
                &worktree_dir_name,
                sink,
                &mut warnings,
            );
            core.finish_repo_operation(result, &warnings)
        });
        Ok(RepoOperation { id, events })
    }

    /// Clones the repo at `url` (a GitHub page, an HTTPS or SSH remote, or any clonable URL) into `<profile>/repos/<name>`, checked out detached, where the profile's repos are discovered.
    /// An existing destination fails the operation, untouched; a cancel removes the partial clone.
    pub fn clone_repo_into_profile(
        self: Arc<Self>,
        profile_path: String,
        url: String,
    ) -> Result<RepoOperation, CoreError> {
        let (id, events) = self.start_creation(move |core, handle, sink| {
            let result = creation::clone_repo(handle, &profile_path, &url, sink);
            core.finish_repo_operation(result, &[])
        });
        Ok(RepoOperation { id, events })
    }

    /// Where `clone_repo_into_profile` would clone `url` into the profile, for a hint while it is typed; `None` when the URL isn't one it can clone.
    pub fn clone_destination(
        &self,
        profile_path: String,
        url: String,
    ) -> Result<Option<String>, CoreError> {
        let Ok(target) = repo_clone::parse_clone_target(&url) else {
            return Ok(None);
        };
        let profile = config::expand_tilde(&profile_path)?;
        let destination = Path::new(&profile).join("repos").join(target.dir_name);
        Ok(Some(destination.to_string_lossy().into_owned()))
    }
}

impl DevoraCore {
    /// Reports the operation's warnings and its failure to the log, as Ember reports them, and returns its last event.
    fn finish_repo_operation(
        &self,
        result: Result<Outcome, String>,
        warnings: &[String],
    ) -> CreationEvent {
        for warning in warnings {
            self.log.report(warning);
        }
        match result {
            Ok(Outcome::Done(workspace)) => CreationEvent::Done { workspace },
            Ok(Outcome::Cancelled) => CreationEvent::Cancelled,
            Err(message) => {
                self.log.report(&message);
                CreationEvent::Failed { message }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{core, git, label, profile, pull_all, Profile};
    use devora_core::workspace::CreatedWorkspace;
    use futures_lite::future::block_on;
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command as Process;

    /// An active workspace `ws-1` with a worktree of the profile's repo, as a task's.
    fn workspace(profile: &Profile) -> PathBuf {
        let ws = profile.workspace("ws-1");
        fs::create_dir_all(&ws).unwrap();
        fs::write(ws.join("task.json"), r#"{"title": "Main work"}"#).unwrap();
        fs::write(ws.join("initialized"), "").unwrap();
        git(
            &profile.repo,
            &[
                "worktree",
                "add",
                "-q",
                "--detach",
                ws.join("repo").to_str().unwrap(),
            ],
        );
        ws
    }

    fn add(core: &Arc<DevoraCore>, ws: &Path, profile: &Profile, name: &str) -> RepoOperation {
        core.clone()
            .add_repo_to_workspace(
                ws.to_string_lossy().into_owned(),
                profile.repo.to_string_lossy().into_owned(),
                name.into(),
            )
            .unwrap()
    }

    fn worktrees(repo: &Path) -> String {
        let output = Process::new("git")
            .args(["worktree", "list"])
            .current_dir(repo)
            .output()
            .unwrap();
        String::from_utf8_lossy(&output.stdout).into_owned()
    }

    #[test]
    fn adding_a_repo_streams_its_steps_then_ends_with_the_workspace_and_its_claude_md() {
        let profile = profile(Some("echo prepared"));
        let ws = workspace(&profile);
        let (core, _log) = core();

        let operation = add(&core, &ws, &profile, "repo-ref");
        let events = pull_all(&operation.events, |_| {});

        // `mise trust` is a step only where mise is installed.
        let steps: Vec<_> = events
            .iter()
            .filter_map(label)
            .filter(|label| !label.starts_with("Trusting mise"))
            .collect();
        assert_eq!(
            steps,
            [
                "Fetching repo-ref",
                "Creating worktree repo-ref",
                "Preparing repo-ref"
            ]
        );
        let preparing = events
            .iter()
            .position(|event| label(event) == Some("Preparing repo-ref"))
            .unwrap();
        let prepared = events
            .iter()
            .position(|event| {
                event
                    == &CreationEvent::Log {
                        line: "prepared".into(),
                    }
            })
            .unwrap();
        assert!(
            preparing < prepared,
            "the output follows its step: {events:?}"
        );
        assert_eq!(
            events.last(),
            Some(&CreationEvent::Done {
                workspace: CreatedWorkspace {
                    path: ws.to_string_lossy().into_owned(),
                    name: "ws-1".into()
                }
            })
        );
        assert!(ws.join("repo-ref").join("file.txt").exists());
        let claude_md = fs::read_to_string(ws.join("CLAUDE.md")).unwrap();
        assert!(claude_md.contains("- `repo-ref/`"), "{claude_md}");
    }

    #[test]
    fn a_cancel_while_preparing_removes_the_added_worktree_and_leaves_the_workspace_as_it_was() {
        let profile = profile(Some("sleep 30"));
        let ws = workspace(&profile);
        let (core, _log) = core();
        let started = std::time::Instant::now();

        let operation = add(&core, &ws, &profile, "repo-ref");
        let events = pull_all(&operation.events, |event| {
            if label(event).is_some_and(|label| label.starts_with("Preparing")) {
                core.cancel_workspace_creation(operation.id).unwrap();
            }
        });

        assert_eq!(events.last(), Some(&CreationEvent::Cancelled));
        assert!(started.elapsed() < std::time::Duration::from_secs(20));
        assert!(!ws.join("repo-ref").exists());
        assert!(!worktrees(&profile.repo).contains("repo-ref"));
        assert!(ws.join("repo").join("file.txt").exists());
        assert!(!ws.join("CLAUDE.md").exists());
    }

    #[test]
    fn adding_an_existing_name_fails_and_reports_it_leaving_the_worktree_alone() {
        let profile = profile(None);
        let ws = workspace(&profile);
        let (core, log) = core();

        let operation = add(&core, &ws, &profile, "repo");
        let events = pull_all(&operation.events, |_| {});

        let message = "Directory 'repo' already exists. Use a different postfix.";
        assert_eq!(
            events,
            [CreationEvent::Failed {
                message: message.into()
            }]
        );
        assert!(ws.join("repo").join("file.txt").exists());
        let reported = block_on(log.errors().unwrap().next_batch(10)).unwrap();
        assert_eq!(reported, [message]);
    }

    #[test]
    fn a_name_that_leaves_the_workspace_fails() {
        let profile = profile(None);
        let ws = workspace(&profile);
        let (core, _log) = core();

        let operation = add(&core, &ws, &profile, "../repo-escape");
        let events = pull_all(&operation.events, |_| {});

        assert!(
            matches!(events.as_slice(), [CreationEvent::Failed { message }] if message.contains("invalid worktree name")),
            "{events:?}"
        );
        assert!(!profile.path.join("workspaces").join("repo-escape").exists());
    }

    fn clone(core: &Arc<DevoraCore>, profile: &Profile, url: String) -> RepoOperation {
        core.clone()
            .clone_repo_into_profile(profile.path_string(), url)
            .unwrap()
    }

    /// The profile's origin, as a URL to clone again under another name.
    fn origin_url(profile: &Profile, name: &str) -> String {
        let renamed = profile.root.path().join(format!("{name}.git"));
        if !renamed.exists() {
            fs::rename(profile.root.path().join("repo.git"), &renamed).unwrap();
        }
        format!("file://{}", renamed.display())
    }

    #[test]
    fn cloning_streams_its_steps_and_ends_with_the_detached_clone() {
        let profile = profile(None);
        let (core, _log) = core();

        let operation = clone(&core, &profile, origin_url(&profile, "other"));
        let events = pull_all(&operation.events, |_| {});

        let steps: Vec<_> = events.iter().filter_map(label).collect();
        assert_eq!(steps, ["Cloning other", "Checking out detached"]);
        let destination = profile.path.join("repos").join("other");
        assert_eq!(
            events.last(),
            Some(&CreationEvent::Done {
                workspace: CreatedWorkspace {
                    path: destination.to_string_lossy().into_owned(),
                    name: "other".into()
                }
            })
        );
        assert!(destination.join("file.txt").exists());
        let head = Process::new("git")
            .args(["symbolic-ref", "-q", "HEAD"])
            .current_dir(&destination)
            .status()
            .unwrap();
        assert!(!head.success(), "the clone's HEAD is detached");
    }

    #[test]
    fn a_cancel_at_once_removes_the_partial_clone() {
        let profile = profile(None);
        let (core, _log) = core();

        let operation = clone(&core, &profile, origin_url(&profile, "other"));
        core.cancel_workspace_creation(operation.id).unwrap();
        let events = pull_all(&operation.events, |_| {});

        assert_eq!(events.last(), Some(&CreationEvent::Cancelled));
        assert!(!profile.path.join("repos").join("other").exists());
    }

    #[test]
    fn cloning_onto_an_existing_repo_fails_and_leaves_it_untouched() {
        let profile = profile(None);
        let (core, log) = core();

        let operation = clone(&core, &profile, origin_url(&profile, "repo"));
        let events = pull_all(&operation.events, |_| {});

        assert!(
            matches!(events.as_slice(), [CreationEvent::Failed { message }] if message.contains("already exists")),
            "{events:?}"
        );
        assert!(profile.repo.join("file.txt").exists());
        assert_eq!(
            block_on(log.errors().unwrap().next_batch(10))
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn the_clone_destination_is_under_the_profiles_repos_or_none_for_what_cant_be_cloned() {
        let (core, _log) = core();
        let destination = |url: &str| core.clone_destination("/p".into(), url.into()).unwrap();

        assert_eq!(
            destination("https://github.com/org/api/pulls"),
            Some("/p/repos/api".into())
        );
        assert_eq!(
            destination("git@github.com:org/web.git"),
            Some("/p/repos/web".into())
        );
        assert_eq!(destination(""), None);
        assert_eq!(destination("not a url"), None);
    }
}

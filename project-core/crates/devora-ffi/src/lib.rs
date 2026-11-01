//! The UniFFI side of the facade (ADR-010): the `DevoraCore` object Sapphire binds, over `devora-core`.
//!
//! Every export returns `Result`, since a panic in an infallible export is an uncatchable crash on the Swift side.
//! The records here are the FFI shapes of the core's data; the crates behind `devora-core` stay UniFFI-free.

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use devora_core::{config, state, theme, workspace};

mod app_state;
mod creation;
mod log;
mod profiles;
mod repos;
mod terminal;
#[cfg(test)]
mod test_support;

pub use log::{AppLog, ErrorSubscription};

uniffi::setup_scaffolding!();

#[derive(Debug, PartialEq, uniffi::Error)]
pub enum CoreError {
    Failed {
        message: String,
    },
    /// Another process holds the app state at `path`, so this one must not write it too.
    StateInUse {
        path: String,
        message: String,
    },
    /// The app state at `path` couldn't be opened (corrupt, from a newer build, unreadable); `set_aside_state` makes way for a fresh one.
    StateUnreadable {
        path: String,
        message: String,
    },
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoreError::Failed { message }
            | CoreError::StateInUse { message, .. }
            | CoreError::StateUnreadable { message, .. } => f.write_str(message),
        }
    }
}

impl std::error::Error for CoreError {}

impl From<String> for CoreError {
    fn from(message: String) -> Self {
        CoreError::Failed { message }
    }
}

#[derive(Debug, PartialEq, uniffi::Record)]
pub struct Profile {
    pub name: String,
    pub path: String,
    pub repo_count: u32,
}

impl From<workspace::ProfileInfo> for Profile {
    fn from(info: workspace::ProfileInfo) -> Self {
        Profile {
            name: info.name,
            path: info.path,
            repo_count: u32::try_from(info.repo_count).unwrap_or(u32::MAX),
        }
    }
}

#[derive(Debug, PartialEq, uniffi::Record)]
pub struct Workspace {
    /// The workspace's directory name (`ws-N`).
    pub id: String,
    pub path: String,
    /// The root of the profile the workspace belongs to: open its session with this profile, not the selected one.
    pub profile_path: String,
    /// The task title; empty for an inactive workspace.
    pub title: String,
    pub repos: Vec<String>,
    pub active: bool,
}

impl From<workspace::WorkspaceInfo> for Workspace {
    fn from(info: workspace::WorkspaceInfo) -> Self {
        Workspace {
            id: info.id,
            path: info.path,
            profile_path: info.profile_path,
            title: info.task_title,
            repos: info.repos,
            active: info.active,
        }
    }
}

#[derive(Debug, PartialEq, uniffi::Record)]
pub struct RepoStatus {
    pub name: String,
    /// The branch, or the short commit hash when the HEAD is detached.
    pub branch: String,
    pub is_detached: bool,
    pub modified: u32,
    pub untracked: u32,
}

impl From<workspace::RepoStatus> for RepoStatus {
    fn from(status: workspace::RepoStatus) -> Self {
        RepoStatus {
            name: status.name,
            branch: status.branch,
            is_detached: status.is_detached,
            modified: u32::try_from(status.modified).unwrap_or(u32::MAX),
            untracked: u32::try_from(status.untracked).unwrap_or(u32::MAX),
        }
    }
}

/// A workspace's repos as git sees them; a workspace whose status couldn't be read is invalid, which doesn't fail the call.
#[derive(Debug, PartialEq, uniffi::Record)]
pub struct WorkspaceStatus {
    /// Sorted by name; empty when `error` is set.
    pub repos: Vec<RepoStatus>,
    pub error: Option<String>,
}

#[derive(uniffi::Object)]
pub struct DevoraCore {
    config_path: PathBuf,
    /// The app state (ADR-008).
    state: state::Store,
    log: Arc<AppLog>,
    creations: creation::Creations,
}

#[uniffi::export]
impl DevoraCore {
    /// `config_path` overrides the global config file; without it, `DEVORA_CONFIG_PATH` or `~/.config/devora/config.json` is used.
    ///
    /// `state_path` is the app state's database (`sapphire_state_path`), which this process then holds alone; without it, the state lives in memory.
    ///
    /// `log` is where the core reports the problems that don't fail a call, as Ember's commands do.
    #[uniffi::constructor]
    pub fn new(
        config_path: Option<String>,
        state_path: Option<String>,
        log: Arc<AppLog>,
    ) -> Result<Arc<Self>, CoreError> {
        let config_path = match config_path {
            Some(path) => PathBuf::from(path),
            None => config::global_config_path()?,
        };
        let state = match state_path {
            Some(path) => app_state::open(path)?,
            None => state::Store::new(),
        };
        Ok(Arc::new(DevoraCore {
            config_path,
            state,
            log,
            creations: Default::default(),
        }))
    }

    pub fn version(&self) -> Result<String, CoreError> {
        Ok(env!("CARGO_PKG_VERSION").to_string())
    }

    pub fn list_profiles(&self) -> Result<Vec<Profile>, CoreError> {
        let profiles = workspace::list_profiles_in(&self.config_path)?;
        Ok(profiles.into_iter().map(Profile::from).collect())
    }

    /// Problems that don't stop the listing, such as a corrupt `task.json`, are reported to the log, as errors.
    pub fn list_workspaces(&self, profile_path: String) -> Result<Vec<Workspace>, CoreError> {
        let mut warnings = Vec::new();
        let workspaces = workspace::list_workspaces(&profile_path, &mut warnings);
        for warning in &warnings {
            self.log.report(warning);
        }
        Ok(workspaces?.into_iter().map(Workspace::from).collect())
    }

    /// Runs `git status` in each of the workspace's repos, at most five at once, as Ember's `get_all_workspace_statuses` does.
    /// It takes one workspace, so that a UI can show each workspace's status as it arrives.
    pub fn workspace_status(
        &self,
        workspace_path: String,
        repo_names: Vec<String>,
    ) -> Result<WorkspaceStatus, CoreError> {
        let mut warnings = Vec::new();
        let input = workspace::WorkspaceStatusInput {
            workspace_path,
            repo_names,
        };
        let result = workspace::get_all_workspace_statuses(vec![input], &mut warnings);
        for warning in &warnings {
            self.log.report(warning);
        }
        let status = result?
            .workspace_statuses
            .pop()
            .ok_or_else(|| "no status for the workspace".to_string())?;
        Ok(WorkspaceStatus {
            repos: status.statuses.into_iter().map(RepoStatus::from).collect(),
            error: status.error,
        })
    }

    /// Ends the workspace's task, keeping its worktrees: in each repo, discards the uncommitted changes and detaches the HEAD, then deletes `task.json`.
    pub fn remove_task(&self, workspace_path: String) -> Result<(), CoreError> {
        Ok(workspace::remove_task(&workspace_path)?)
    }

    /// Removes each repo's worktree (`git worktree remove --force`, in parallel), then the workspace's directory.
    pub fn delete_workspace(&self, workspace_path: String) -> Result<(), CoreError> {
        Ok(workspace::delete_workspace(&workspace_path)?)
    }

    /// Writes `report_json` to `<profile>/diagnostics/hub-profile-<timestamp>.json` and returns the file's path.
    pub fn save_profiling_report(
        &self,
        profile_path: String,
        report_json: String,
    ) -> Result<String, CoreError> {
        Ok(workspace::save_profiling_report(
            &profile_path,
            &report_json,
        )?)
    }

    /// The theme file's colors, keyed by the CSS custom property names the UIs share.
    pub fn load_theme(&self, path: String) -> Result<HashMap<String, String>, CoreError> {
        Ok(theme::load_theme(Path::new(&path))?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_lite::future::block_on;
    use std::fs;

    fn write_profile(root: &Path, name: &str) -> PathBuf {
        let profile = root.join(name);
        fs::create_dir_all(profile.join("workspaces")).unwrap();
        fs::create_dir_all(profile.join("repos")).unwrap();
        fs::write(
            profile.join("config.json"),
            format!(r#"{{"name": "{name}"}}"#),
        )
        .unwrap();
        profile
    }

    fn write_workspace(profile: &Path, id: &str, task_json: Option<&str>, repos: &[&str]) {
        let ws = profile.join("workspaces").join(id);
        fs::create_dir_all(&ws).unwrap();
        fs::write(ws.join("initialized"), "").unwrap();
        if let Some(task) = task_json {
            fs::write(ws.join("task.json"), task).unwrap();
        }
        for repo in repos {
            fs::create_dir(ws.join(repo)).unwrap();
        }
    }

    fn core_with_config(root: &Path, contents: &str) -> Arc<DevoraCore> {
        let config_path = root.join("config.json");
        fs::write(&config_path, contents).unwrap();
        DevoraCore::new(
            Some(config_path.to_string_lossy().into_owned()),
            None,
            crate::log::test_log(),
        )
        .unwrap()
    }

    #[test]
    fn lists_the_registered_profiles_from_the_overridden_config() {
        let tmp = tempfile::tempdir().unwrap();
        let alpha = write_profile(tmp.path(), "alpha");
        let unregistered_dir = tmp.path().join("not-a-profile");
        fs::create_dir(&unregistered_dir).unwrap();
        let core = core_with_config(
            tmp.path(),
            &format!(r#"{{"profiles": [{:?}, {:?}]}}"#, alpha, unregistered_dir),
        );

        let profiles = core.list_profiles().unwrap();

        assert_eq!(
            profiles,
            vec![Profile {
                name: "alpha".to_string(),
                path: alpha.to_string_lossy().into_owned(),
                repo_count: 0,
            }]
        );
    }

    #[test]
    fn a_malformed_config_is_an_error_not_an_empty_list() {
        let tmp = tempfile::tempdir().unwrap();
        let core = core_with_config(tmp.path(), "{not json");

        let message = core.list_profiles().unwrap_err().to_string();

        assert!(message.contains("failed to parse"), "{message}");
    }

    #[test]
    fn lists_workspaces_with_title_and_activity_reporting_a_corrupt_task() {
        let tmp = tempfile::tempdir().unwrap();
        let profile = write_profile(tmp.path(), "alpha");
        write_workspace(
            &profile,
            "ws-1",
            Some(r#"{"title": "Fix the build"}"#),
            &["repo-a"],
        );
        write_workspace(&profile, "ws-2", None, &[]);
        write_workspace(&profile, "ws-3", Some("{corrupt"), &[]);
        let log = log::test_log();
        let core =
            DevoraCore::new(Some("/nonexistent/config.json".into()), None, log.clone()).unwrap();

        let workspaces = core
            .list_workspaces(profile.to_string_lossy().into_owned())
            .unwrap();

        let summary: Vec<_> = workspaces
            .iter()
            .map(|w| (w.id.as_str(), w.title.as_str(), w.active, w.repos.clone()))
            .collect();
        assert_eq!(
            summary,
            vec![
                ("ws-1", "Fix the build", true, vec!["repo-a".to_string()]),
                ("ws-2", "", false, vec![]),
                ("ws-3", "", true, vec![]),
            ]
        );
        let reported = block_on(log.errors().unwrap().next_batch(10)).unwrap();
        assert_eq!(reported.len(), 1, "{reported:?}");
        assert!(reported[0].contains("failed to parse"), "{reported:?}");
    }

    fn git(dir: &Path, args: &[&str]) {
        let status = std::process::Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@t"])
            .args(args)
            .current_dir(dir)
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    }

    #[test]
    fn reads_each_repos_branch_and_changes_sorted_by_name() {
        let tmp = tempfile::tempdir().unwrap();
        let ws = tmp.path().join("ws-1");
        for repo in ["beta", "alpha"] {
            let dir = ws.join(repo);
            fs::create_dir_all(&dir).unwrap();
            git(&dir, &["init", "-q", "-b", "main"]);
            git(&dir, &["commit", "-q", "--allow-empty", "-m", "init"]);
        }
        git(&ws.join("beta"), &["checkout", "-q", "--detach"]);
        fs::write(ws.join("alpha").join("new.txt"), "").unwrap();
        let core = core_with_config(tmp.path(), "{}");

        let status = core
            .workspace_status(
                ws.to_string_lossy().into_owned(),
                vec!["beta".into(), "alpha".into()],
            )
            .unwrap();

        assert_eq!(status.error, None);
        let summary: Vec<_> = status
            .repos
            .iter()
            .map(|r| (r.name.as_str(), r.is_detached, r.modified, r.untracked))
            .collect();
        assert_eq!(summary, vec![("alpha", false, 0, 1), ("beta", true, 0, 0)]);
        assert_eq!(status.repos[0].branch, "main");
        assert!(!status.repos[1].branch.is_empty());
    }

    #[test]
    fn a_repo_that_is_gone_makes_the_workspace_invalid_rather_than_failing_the_call() {
        let tmp = tempfile::tempdir().unwrap();
        let ws = tmp.path().join("ws-1");
        fs::create_dir_all(&ws).unwrap();
        let core = core_with_config(tmp.path(), "{}");

        let status = core
            .workspace_status(ws.to_string_lossy().into_owned(), vec!["gone".into()])
            .unwrap();

        assert_eq!(status.repos, vec![]);
        assert!(status.error.is_some_and(|e| e.contains("gone")));
    }

    fn git_output(dir: &Path, args: &[&str]) -> String {
        let output = std::process::Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .unwrap();
        assert!(output.status.success(), "git {args:?}");
        String::from_utf8(output.stdout).unwrap()
    }

    /// A profile with a source repo and an active workspace `ws-1` whose `repo` is a detached worktree of it.
    fn workspace_with_a_worktree(root: &Path) -> (PathBuf, PathBuf) {
        let profile = write_profile(root, "alpha");
        let source = profile.join("repos").join("repo");
        fs::create_dir_all(&source).unwrap();
        git(&source, &["init", "-q", "-b", "main"]);
        fs::write(source.join("tracked.txt"), "original").unwrap();
        git(&source, &["add", "."]);
        git(&source, &["commit", "-q", "-m", "init"]);
        let ws = profile.join("workspaces").join("ws-1");
        fs::create_dir_all(&ws).unwrap();
        fs::write(ws.join("task.json"), r#"{"title": "Task"}"#).unwrap();
        let worktree = ws.join("repo");
        git(
            &source,
            &[
                "worktree",
                "add",
                "-q",
                "--detach",
                worktree.to_str().unwrap(),
            ],
        );
        (source, ws)
    }

    #[test]
    fn removing_a_task_discards_the_changes_and_detaches_but_keeps_the_worktree() {
        let tmp = tempfile::tempdir().unwrap();
        let (source, ws) = workspace_with_a_worktree(tmp.path());
        let repo = ws.join("repo");
        git(&repo, &["checkout", "-q", "-b", "feature"]);
        fs::write(repo.join("tracked.txt"), "changed").unwrap();
        fs::write(repo.join("untracked.txt"), "").unwrap();
        let core = core_with_config(tmp.path(), "{}");

        core.remove_task(ws.to_string_lossy().into_owned()).unwrap();

        assert!(!ws.join("task.json").exists());
        assert_eq!(git_output(&repo, &["status", "--porcelain"]), "");
        assert_eq!(
            git_output(&repo, &["rev-parse", "--abbrev-ref", "HEAD"]).trim(),
            "HEAD"
        );
        assert!(git_output(&source, &["worktree", "list"]).contains("ws-1/repo"));
    }

    #[test]
    fn deleting_a_workspace_removes_its_worktrees_and_its_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let (source, ws) = workspace_with_a_worktree(tmp.path());
        fs::remove_file(ws.join("task.json")).unwrap();
        fs::write(ws.join("repo").join("untracked.txt"), "").unwrap();
        let core = core_with_config(tmp.path(), "{}");

        core.delete_workspace(ws.to_string_lossy().into_owned())
            .unwrap();

        assert!(!ws.exists());
        assert!(!git_output(&source, &["worktree", "list"]).contains("ws-1"));
        assert!(source.exists());
    }

    #[test]
    fn loads_a_theme_and_reports_a_missing_one() {
        let tmp = tempfile::tempdir().unwrap();
        let theme_path = tmp.path().join("theme.conf");
        fs::write(&theme_path, "background #24273A\n").unwrap();
        let core = core_with_config(tmp.path(), "{}");

        let tokens = core
            .load_theme(theme_path.to_string_lossy().into_owned())
            .unwrap();
        let missing = core.load_theme(tmp.path().join("nope.conf").to_string_lossy().into_owned());

        assert_eq!(
            tokens.get("--color-terminal-bg").map(String::as_str),
            Some("#24273A")
        );
        assert!(missing.is_err());
    }
}

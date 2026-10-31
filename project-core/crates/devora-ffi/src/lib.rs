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
mod log;
mod terminal;

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
            title: info.task_title,
            repos: info.repos,
            active: info.active,
        }
    }
}

#[derive(uniffi::Object)]
pub struct DevoraCore {
    config_path: PathBuf,
    /// The app state (ADR-008).
    state: state::Store,
    log: Arc<AppLog>,
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

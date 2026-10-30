//! What a terminal session runs, built once for every PTY owner: Ember's `devora-pty`, and Sapphire's libghostty, which owns its own PTY.
//!
//! [`terminal_spec`] resolves a [`TerminalRequest`] (what the user opened) on a [`TerminalHost`] (the app around it) into a [`TerminalSpec`]: the program, its arguments, the cwd, and the environment to set and to remove.
//! The spec is plain owned data, so a UI can spawn it without calling back into the core.

use devora_config::config;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Inherited variables a terminal must not see.
/// An app launched from a Neovim terminal inherits `$NVIM`, and Neovim started inside our terminal then talks to that outer instance instead of running.
pub const REMOVED_ENV: [&str; 2] = ["NVIM", "NVIM_LISTEN_ADDRESS"];

/// Everything needed to start a terminal session's process.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: String,
    /// Variables to set, on top of the inherited environment.
    pub env: HashMap<String, String>,
    /// Inherited variables to remove.
    pub env_remove: Vec<String>,
}

/// What a terminal is opened on.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum TerminalTarget {
    /// A login shell in `cwd`, or in `$HOME` when it has none.
    Shell { cwd: Option<String> },
    /// A workspace: the shell starts in the worktree of its only repo (or the workspace root when it has several) and runs the profile's `terminal.default-app`.
    Workspace { path: String, repos: Vec<String> },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalRequest {
    pub target: TerminalTarget,
    /// The session's profile root; it selects the profile's settings and is exposed to `debi` as `DEBI_PROFILE_PATH`.
    pub profile_path: Option<String>,
}

/// The app that runs the terminal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostApp {
    Ember,
    Sapphire,
}

impl HostApp {
    /// The variable that tells tools inside the terminal which app they run in.
    pub fn marker_env(self) -> &'static str {
        match self {
            Self::Ember => "DEVORA_EMBER",
            Self::Sapphire => "DEVORA_SAPPHIRE",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalHost {
    pub app: HostApp,
    /// The app bundle's `Resources` directory, whose `git-shortcuts`, `bundled-apps` and `cc-plugins/*/bin` go first on `PATH`; `None` outside a bundle.
    pub resources_dir: Option<String>,
    /// The port of the app's IPC server, exposed as `DEVORA_IPC_PORT` (`debi preview` posts to it).
    pub ipc_port: u16,
}

/// The settings a terminal reads from the profile and user config.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TerminalSettings {
    pub git_shortcuts: bool,
    /// `terminal.default-app`; `None` (or `shell`) runs a bare login shell.
    pub default_app: Option<String>,
    /// The resolved Claude launch variables (`devora_config::config::claude_launch_env`).
    pub claude_env: HashMap<String, String>,
}

impl TerminalSettings {
    /// Reads the settings for a profile (or the user level when `None`).
    /// An unreadable `default-app` falls back to a bare shell, with a warning pushed to `warnings`.
    pub fn resolve(profile_path: Option<&str>, warnings: &mut Vec<String>) -> Self {
        let default_app = profile_path.and_then(|path| {
            config::get_default_app(path).unwrap_or_else(|e| {
                warnings.push(format!(
                    "terminal.default-app is unreadable, so the terminal runs a bare shell: {e}"
                ));
                None
            })
        });
        Self {
            git_shortcuts: config::get_git_shortcuts_enabled(profile_path),
            default_app,
            claude_env: config::claude_launch_env(profile_path),
        }
    }
}

/// Builds the spec from the config files and the app's own environment.
///
/// `terminal_id` is the owner's id for the session, exposed as `DEVORA_PTY_ID` so that IPC requests from inside the terminal (`debi preview`) can name their tab.
/// Settings that can't be read fall back to their defaults, with a warning pushed to `warnings` for the UI to report (ADR-002).
pub fn terminal_spec(
    request: &TerminalRequest,
    host: &TerminalHost,
    terminal_id: u32,
    warnings: &mut Vec<String>,
) -> TerminalSpec {
    let settings = TerminalSettings::resolve(request.profile_path.as_deref(), warnings);
    build(request, host, &settings, terminal_id, &|name| {
        std::env::var(name).ok()
    })
}

/// The `Resources` directory of the app bundle the current executable runs from (`<app>/Contents/MacOS/<exe>` → `<app>/Contents/Resources`).
pub fn bundle_resources_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    Some(exe.parent()?.parent()?.join("Resources"))
}

/// The pure part of [`terminal_spec`]: `inherited` looks up the app's own environment.
pub fn build(
    request: &TerminalRequest,
    host: &TerminalHost,
    settings: &TerminalSettings,
    terminal_id: u32,
    inherited: &dyn Fn(&str) -> Option<String>,
) -> TerminalSpec {
    let (cwd, app_command) = match &request.target {
        TerminalTarget::Shell { cwd } => (cwd.clone(), None),
        TerminalTarget::Workspace { path, repos } => {
            let cwd = match repos.as_slice() {
                [only] => format!("{path}/{only}"),
                _ => path.clone(),
            };
            let app = settings
                .default_app
                .clone()
                .filter(|app| !app.is_empty() && app != "shell");
            (Some(cwd), app)
        }
    };

    let mut args = vec!["-l".to_string(), "-i".to_string()];
    if let Some(app) = app_command {
        args.extend(["-c".to_string(), app]);
    }

    let mut env = HashMap::new();
    let mut set = |key: &str, value: String| {
        env.insert(key.to_string(), value);
    };
    set("TERM", "xterm-256color".into());
    set("COLORTERM", "truecolor".into());
    set(host.app.marker_env(), "1".into());
    set("DEVORA_IPC_PORT", host.ipc_port.to_string());
    set("DEVORA_PTY_ID", terminal_id.to_string());
    if let Some(resources) = &host.resources_dir {
        let extra = bundle_paths(Path::new(resources), settings.git_shortcuts);
        if !extra.is_empty() {
            set(
                "PATH",
                format!(
                    "{}:{}",
                    extra.join(":"),
                    inherited("PATH").unwrap_or_default()
                ),
            );
        }
    }
    for (key, value) in &settings.claude_env {
        set(key, value.clone());
    }
    if let Some(profile) = &request.profile_path {
        set("DEBI_PROFILE_PATH", profile.clone());
    }

    TerminalSpec {
        program: inherited("SHELL").unwrap_or_else(|| "/bin/zsh".to_string()),
        args,
        cwd: cwd
            .or_else(|| inherited("HOME"))
            .unwrap_or_else(|| "/".to_string()),
        env,
        env_remove: REMOVED_ENV.iter().map(|name| name.to_string()).collect(),
    }
}

/// The bundle directories that go first on `PATH`, in order, skipping any that don't exist.
fn bundle_paths(resources: &Path, git_shortcuts: bool) -> Vec<String> {
    let mut paths = Vec::new();
    let mut push = |dir: PathBuf| paths.push(dir.to_string_lossy().into_owned());

    // Debi's git-shortcut shims (`gcl` → `debi git checkout-latest`, …) come first, so a bare alias resolves to the shim in any shell that doesn't already define the name.
    let git_shortcuts_dir = resources.join("git-shortcuts");
    if git_shortcuts && git_shortcuts_dir.is_dir() {
        push(git_shortcuts_dir);
    }
    let bundled_apps = resources.join("bundled-apps");
    if bundled_apps.exists() {
        push(bundled_apps);
    }
    if let Ok(entries) = std::fs::read_dir(resources.join("cc-plugins")) {
        for entry in entries.flatten() {
            let bin = entry.path().join("bin");
            if bin.is_dir() {
                push(bin);
            }
        }
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host(resources_dir: Option<&Path>) -> TerminalHost {
        TerminalHost {
            app: HostApp::Ember,
            resources_dir: resources_dir.map(|dir| dir.to_string_lossy().into_owned()),
            ipc_port: 4242,
        }
    }

    fn inherited(name: &str) -> Option<String> {
        match name {
            "PATH" => Some("/usr/bin:/bin".into()),
            "SHELL" => Some("/bin/fish".into()),
            "HOME" => Some("/home/me".into()),
            _ => None,
        }
    }

    fn spec(target: TerminalTarget, settings: &TerminalSettings) -> TerminalSpec {
        let request = TerminalRequest {
            target,
            profile_path: None,
        };
        build(&request, &host(None), settings, 7, &inherited)
    }

    fn workspace(repos: &[&str]) -> TerminalTarget {
        TerminalTarget::Workspace {
            path: "/ws/1".into(),
            repos: repos.iter().map(|r| r.to_string()).collect(),
        }
    }

    fn nvim() -> TerminalSettings {
        TerminalSettings {
            default_app: Some("nvim".into()),
            ..Default::default()
        }
    }

    #[test]
    fn a_shell_is_a_login_shell_in_its_cwd_or_home() {
        let s = spec(
            TerminalTarget::Shell {
                cwd: Some("/tmp".into()),
            },
            &nvim(),
        );
        assert_eq!(s.program, "/bin/fish");
        assert_eq!(
            s.args,
            ["-l", "-i"],
            "a plain shell never runs the default app"
        );
        assert_eq!(s.cwd, "/tmp");
        assert_eq!(
            spec(TerminalTarget::Shell { cwd: None }, &nvim()).cwd,
            "/home/me"
        );
    }

    #[test]
    fn without_shell_or_home_it_falls_back_to_zsh_in_the_root() {
        let request = TerminalRequest {
            target: TerminalTarget::Shell { cwd: None },
            profile_path: None,
        };
        let s = build(
            &request,
            &host(None),
            &TerminalSettings::default(),
            1,
            &|_| None,
        );
        assert_eq!(s.program, "/bin/zsh");
        assert_eq!(s.cwd, "/");
    }

    #[test]
    fn a_workspace_with_one_repo_starts_in_its_worktree() {
        assert_eq!(spec(workspace(&["api"]), &nvim()).cwd, "/ws/1/api");
    }

    #[test]
    fn a_workspace_with_several_or_no_repos_starts_in_its_root() {
        assert_eq!(spec(workspace(&["api", "web"]), &nvim()).cwd, "/ws/1");
        assert_eq!(spec(workspace(&[]), &nvim()).cwd, "/ws/1");
    }

    #[test]
    fn a_workspace_runs_the_default_app_unless_it_is_shell_or_unset() {
        assert_eq!(
            spec(workspace(&["api"]), &nvim()).args,
            ["-l", "-i", "-c", "nvim"]
        );
        for default_app in [None, Some("shell"), Some("")] {
            let settings = TerminalSettings {
                default_app: default_app.map(String::from),
                ..Default::default()
            };
            assert_eq!(
                spec(workspace(&["api"]), &settings).args,
                ["-l", "-i"],
                "{default_app:?}"
            );
        }
    }

    #[test]
    fn the_environment_names_the_terminal_and_its_host() {
        let s = spec(
            TerminalTarget::Shell { cwd: None },
            &TerminalSettings::default(),
        );
        assert_eq!(
            s.env.get("TERM").map(String::as_str),
            Some("xterm-256color")
        );
        assert_eq!(
            s.env.get("COLORTERM").map(String::as_str),
            Some("truecolor")
        );
        assert_eq!(s.env.get("DEVORA_EMBER").map(String::as_str), Some("1"));
        assert_eq!(
            s.env.get("DEVORA_IPC_PORT").map(String::as_str),
            Some("4242")
        );
        assert_eq!(s.env.get("DEVORA_PTY_ID").map(String::as_str), Some("7"));
        assert!(
            !s.env.contains_key("PATH"),
            "PATH is inherited untouched outside a bundle"
        );
        assert!(!s.env.contains_key("DEBI_PROFILE_PATH"));
        assert_eq!(s.env_remove, ["NVIM", "NVIM_LISTEN_ADDRESS"]);
    }

    #[test]
    fn sapphire_marks_its_terminals_as_its_own() {
        let request = TerminalRequest {
            target: TerminalTarget::Shell { cwd: None },
            profile_path: None,
        };
        let host = TerminalHost {
            app: HostApp::Sapphire,
            ..host(None)
        };
        let s = build(&request, &host, &TerminalSettings::default(), 1, &inherited);
        assert_eq!(s.env.get("DEVORA_SAPPHIRE").map(String::as_str), Some("1"));
        assert!(!s.env.contains_key("DEVORA_EMBER"));
    }

    #[test]
    fn the_profile_and_claude_settings_reach_the_environment() {
        let settings = TerminalSettings {
            claude_env: HashMap::from([("DEVORA_CCC_EFFORT".to_string(), "high".to_string())]),
            ..Default::default()
        };
        let request = TerminalRequest {
            target: TerminalTarget::Shell { cwd: None },
            profile_path: Some("/profiles/work".into()),
        };
        let s = build(&request, &host(None), &settings, 1, &inherited);
        assert_eq!(
            s.env.get("DEVORA_CCC_EFFORT").map(String::as_str),
            Some("high")
        );
        assert_eq!(
            s.env.get("DEBI_PROFILE_PATH").map(String::as_str),
            Some("/profiles/work")
        );
    }

    fn bundle() -> tempfile::TempDir {
        let resources = tempfile::tempdir().unwrap();
        for dir in [
            "git-shortcuts",
            "bundled-apps",
            "cc-plugins/judgess/bin",
            "cc-plugins/no-bin",
        ] {
            std::fs::create_dir_all(resources.path().join(dir)).unwrap();
        }
        std::fs::write(resources.path().join("cc-plugins/stray.txt"), "").unwrap();
        resources
    }

    #[test]
    fn bundle_directories_go_first_on_path() {
        let resources = bundle();
        let root = resources.path().to_string_lossy().into_owned();
        let request = TerminalRequest {
            target: TerminalTarget::Shell { cwd: None },
            profile_path: None,
        };
        let settings = TerminalSettings {
            git_shortcuts: true,
            ..Default::default()
        };
        let s = build(
            &request,
            &host(Some(resources.path())),
            &settings,
            1,
            &inherited,
        );
        assert_eq!(
            s.env["PATH"],
            format!("{root}/git-shortcuts:{root}/bundled-apps:{root}/cc-plugins/judgess/bin:/usr/bin:/bin")
        );
    }

    #[test]
    fn git_shortcuts_stay_off_path_when_disabled() {
        let resources = bundle();
        let root = resources.path().to_string_lossy().into_owned();
        let request = TerminalRequest {
            target: TerminalTarget::Shell { cwd: None },
            profile_path: None,
        };
        let s = build(
            &request,
            &host(Some(resources.path())),
            &TerminalSettings::default(),
            1,
            &inherited,
        );
        assert_eq!(
            s.env["PATH"],
            format!("{root}/bundled-apps:{root}/cc-plugins/judgess/bin:/usr/bin:/bin")
        );
    }

    #[test]
    fn an_empty_bundle_leaves_path_alone() {
        let resources = tempfile::tempdir().unwrap();
        let request = TerminalRequest {
            target: TerminalTarget::Shell { cwd: None },
            profile_path: None,
        };
        let s = build(
            &request,
            &host(Some(resources.path())),
            &TerminalSettings::default(),
            1,
            &inherited,
        );
        assert!(!s.env.contains_key("PATH"));
    }

    /// Serializes the tests that point `DEVORA_CONFIG_PATH` at their own config.
    static ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Resolves the settings of a profile whose `config.json` holds `profile_config`, with no user config.
    fn resolve_with_profile_config(profile_config: &str) -> (TerminalSettings, Vec<String>) {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::tempdir().unwrap();
        let profile = tmp.path().join("profile");
        std::fs::create_dir(&profile).unwrap();
        std::fs::write(profile.join("config.json"), profile_config).unwrap();
        let original = std::env::var("DEVORA_CONFIG_PATH").ok();
        std::env::set_var("DEVORA_CONFIG_PATH", tmp.path().join("no-user-config.json"));

        let mut warnings = Vec::new();
        let settings = TerminalSettings::resolve(profile.to_str(), &mut warnings);

        match original {
            Some(v) => std::env::set_var("DEVORA_CONFIG_PATH", v),
            None => std::env::remove_var("DEVORA_CONFIG_PATH"),
        }
        (settings, warnings)
    }

    #[test]
    fn the_profile_default_app_is_read_without_warnings() {
        let (settings, warnings) =
            resolve_with_profile_config(r#"{"terminal": {"default-app": "nvim"}}"#);
        assert_eq!(settings.default_app.as_deref(), Some("nvim"));
        assert!(warnings.is_empty(), "{warnings:?}");
    }

    #[test]
    fn an_unreadable_profile_config_falls_back_to_a_shell_with_a_warning() {
        let (settings, warnings) = resolve_with_profile_config("{not json");
        assert_eq!(settings.default_app, None);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("failed to parse"), "{warnings:?}");
    }

    #[test]
    fn targets_deserialize_from_the_ui_shape() {
        let shell: TerminalTarget = serde_json::from_str(r#"{"kind":"shell","cwd":null}"#).unwrap();
        assert_eq!(shell, TerminalTarget::Shell { cwd: None });
        let ws: TerminalTarget =
            serde_json::from_str(r#"{"kind":"workspace","path":"/ws/1","repos":["api"]}"#).unwrap();
        assert_eq!(ws, workspace(&["api"]));
    }
}

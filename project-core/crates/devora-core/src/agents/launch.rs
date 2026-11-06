//! What an Agent Space session is launched with when the app supplies nothing
//! (ADR-009, ADR-012): Claude as the user's login shell finds it, that shell's
//! environment, the bundled plugins, and Devora's defaults as inline settings.
//!
//! A Finder-launched app has a minimal environment, so the login shell is asked,
//! as the Health check does. Its environment reaches Claude, and so the agent's
//! tools, as it reaches `ccc` in a terminal, less [`REMOVED`] and with the bundle's
//! directories first on `PATH`.
use super::{LaunchContext, Plugin};
use devora_config::config;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

/// Bounds a login shell that hangs in its rc files.
const CAPTURE_TIMEOUT: Duration = Duration::from_secs(30);

/// Inherited variables that belong to another Claude session, terminal or app.
const REMOVED: [&str; 11] = [
    "CLAUDECODE",
    "CLAUDE_CODE_ENTRYPOINT",
    "CLAUDE_CODE_SESSION_ID",
    "CLAUDE_CODE_PARENT_SESSION_ID",
    "CLAUDE_CODE_SSE_PORT",
    "NVIM",
    "NVIM_LISTEN_ADDRESS",
    "DEVORA_IPC_PORT",
    "DEVORA_PTY_ID",
    "DEVORA_EMBER",
    "DEVORA_SAPPHIRE",
];
/// Prefixes of inherited variables a terminal's `ccc` session sets for itself.
const REMOVED_PREFIXES: [&str; 1] = ["DEVORA_CCC_"];

pub(super) async fn resolve(
    global_config: PathBuf,
    resources: Option<PathBuf>,
) -> Result<LaunchContext, String> {
    // The acceptance harness runs a fake CLI with the app's own environment.
    let test_program = std::env::var("DEVORA_TEST_MODE")
        .is_ok_and(|mode| mode == "1")
        .then(|| std::env::var_os("DEVORA_TEST_CLAUDE_PROGRAM"))
        .flatten();
    let (program, inherited) = match test_program {
        Some(program) => (PathBuf::from(program), std::env::vars().collect()),
        None => capture_login_shell().await?,
    };
    Ok(context(
        global_config,
        resources.as_deref(),
        program,
        inherited,
    ))
}

/// The pure part of [`resolve`], given what the login shell reported.
fn context(
    global_config: PathBuf,
    resources: Option<&Path>,
    program: PathBuf,
    inherited: BTreeMap<String, String>,
) -> LaunchContext {
    let mut environment: BTreeMap<String, String> = inherited
        .into_iter()
        .filter(|(key, _)| {
            !REMOVED.contains(&key.as_str())
                && !REMOVED_PREFIXES
                    .iter()
                    .any(|prefix| key.starts_with(prefix))
        })
        .collect();
    let mut plugins = vec![];
    if let Some(resources) = resources {
        let mut path =
            devora_terminal::bundle_paths(resources, config::get_git_shortcuts_enabled(None));
        if let Some(login) = environment.get("PATH") {
            path.push(login.clone());
        }
        environment.insert("PATH".into(), path.join(":"));
        plugins = devora_agent::plugin_dirs(Some(&resources.join("cc-plugins")))
            .into_iter()
            .map(PathBuf::from)
            .map(|directory| Plugin {
                name: directory
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                directory,
            })
            .collect();
    }
    let env: serde_json::Map<_, _> = devora_agent::ENV_DEFAULTS
        .iter()
        .map(|(key, value)| ((*key).to_string(), serde_json::json!(value)))
        .collect();
    LaunchContext {
        global_config,
        program,
        plugins,
        settings: serde_json::json!({ "env": env }),
        environment,
    }
}

/// Runs the user's login shell once to find `claude` and read its environment.
/// The shell's own stdout (rc-file noise) is discarded; both results go to files.
async fn capture_login_shell() -> Result<(PathBuf, BTreeMap<String, String>), String> {
    let directory = tempfile::tempdir().map_err(|e| e.to_string())?;
    let program_file = directory.path().join("program");
    let env_file = directory.path().join("env");
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
    let mut command = tokio::process::Command::new(shell);
    command
        .args([
            "-l",
            "-i",
            "-c",
            "command -v claude > \"$1\"; env -0 > \"$2\"",
            "devora-agent",
        ])
        .arg(&program_file)
        .arg(&env_file)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let status = tokio::time::timeout(CAPTURE_TIMEOUT, command.status())
        .await
        .map_err(|_| "your login shell didn't finish within 30 seconds".to_string())?
        .map_err(|e| format!("couldn't run your login shell: {e}"))?;
    if !status.success() {
        return Err(format!("your login shell exited with {status}"));
    }
    let program = std::fs::read_to_string(&program_file).unwrap_or_default();
    let program = PathBuf::from(program.trim());
    if !program.is_absolute() {
        return Err("Claude Code (`claude`) isn't on your login shell's PATH".into());
    }
    let env = std::fs::read(&env_file).map_err(|e| e.to_string())?;
    let inherited = env
        .split(|byte| *byte == 0)
        .filter_map(|entry| {
            let entry = String::from_utf8_lossy(entry);
            let (key, value) = entry.split_once('=')?;
            Some((key.to_string(), value.to_string()))
        })
        .collect();
    Ok((program, inherited))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_login_environment_less_other_sessions_with_the_bundle_first_on_path() {
        let resources = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(resources.path().join("bundled-apps")).unwrap();
        std::fs::create_dir_all(resources.path().join("cc-plugins/judgess")).unwrap();
        std::fs::create_dir_all(resources.path().join("cc-plugins/code-cleanup")).unwrap();
        let inherited = BTreeMap::from(
            [
                ("PATH", "/opt/homebrew/bin:/usr/bin"),
                ("SSH_AUTH_SOCK", "/tmp/agent.sock"),
                ("GH_TOKEN", "token"),
                ("CLAUDECODE", "1"),
                ("CLAUDE_CODE_SESSION_ID", "parent"),
                ("DEVORA_CCC_EFFORT", "high"),
                ("NVIM", "/tmp/nvim.sock"),
            ]
            .map(|(k, v)| (k.to_string(), v.to_string())),
        );
        let context = context(
            "/config.json".into(),
            Some(resources.path()),
            "/opt/homebrew/bin/claude".into(),
            inherited,
        );
        assert_eq!(
            context.environment.keys().collect::<Vec<_>>(),
            ["GH_TOKEN", "PATH", "SSH_AUTH_SOCK"]
        );
        assert_eq!(
            context.environment["PATH"],
            format!(
                "{}:/opt/homebrew/bin:/usr/bin",
                resources.path().join("bundled-apps").display()
            )
        );
        assert_eq!(
            context
                .plugins
                .iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>(),
            ["code-cleanup", "judgess"]
        );
        assert_eq!(context.settings["env"]["CLAUDE_CODE_ENABLE_TELEMETRY"], "0");
    }
}

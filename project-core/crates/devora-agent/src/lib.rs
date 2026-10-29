//! How Devora launches Claude Code (ADR-007, ADR-009).
//!
//! [`ccc`] turns a `ccc` command line into what to run: Claude Code with Devora's plugins, the effort and permission mode Devora resolved for the session, and Devora's telemetry defaults.
//! Today the session's settings arrive as `DEVORA_CCC_*` environment variables, injected into Ember's terminals from the user and profile config.

use std::ffi::OsString;
use std::path::Path;

/// The program a launch runs.
pub const CLAUDE: &str = "claude";

/// Environment defaults applied when the variable is unset or empty.
const ENV_DEFAULTS: [(&str, &str); 3] = [
    ("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1"),
    ("CLAUDE_CODE_ENABLE_TELEMETRY", "0"),
    ("CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS", "1"),
];

const EFFORT_ENV: &str = "DEVORA_CCC_EFFORT";
const PERMISSION_MODE_ENV: &str = "DEVORA_CCC_PERMISSION_MODE";

/// What a `ccc` command line asks for.
#[derive(Debug, PartialEq, Eq)]
pub enum Ccc {
    /// `-u`, `--update` or `update` anywhere on the command line: run `claude --update` and nothing else.
    Update,
    /// Launch Claude Code.
    Launch(LaunchSpec),
}

/// A Claude Code launch: arguments plus the environment variables to set on top of the inherited environment.
#[derive(Debug, PartialEq, Eq)]
pub struct LaunchSpec {
    pub args: Vec<OsString>,
    pub env: Vec<(String, String)>,
}

/// Resolves a `ccc` command line.
///
/// - The user's arguments come first, unchanged.
/// - Then one `--plugin-dir` per directory in `plugins_dir`, in name order.
/// - Then `--effort` and `--permission-mode` from `DEVORA_CCC_EFFORT` / `DEVORA_CCC_PERMISSION_MODE`, unless the user passed their own (`--flag value` or `--flag=value`).
///
/// `env_var` reads the launching environment; it is a parameter so the resolution is testable.
pub fn ccc(user_args: &[OsString], plugins_dir: Option<&Path>, env_var: impl Fn(&str) -> Option<String>) -> Ccc {
    let mut user_set_effort = false;
    let mut user_set_permission_mode = false;
    for arg in user_args {
        match arg.to_str() {
            Some("-u" | "--update" | "update") => return Ccc::Update,
            Some(flag) if is_flag(flag, "--effort") => user_set_effort = true,
            Some(flag) if is_flag(flag, "--permission-mode") => user_set_permission_mode = true,
            _ => {}
        }
    }

    let mut args = user_args.to_vec();
    for plugin in plugin_dirs(plugins_dir) {
        args.push("--plugin-dir".into());
        args.push(plugin);
    }
    let non_empty = |name: &str| env_var(name).filter(|v| !v.is_empty());
    if let Some(effort) = non_empty(EFFORT_ENV).filter(|_| !user_set_effort) {
        args.extend(["--effort".into(), effort.into()]);
    }
    if let Some(mode) = non_empty(PERMISSION_MODE_ENV).filter(|_| !user_set_permission_mode) {
        args.extend(["--permission-mode".into(), mode.into()]);
    }

    let env = ENV_DEFAULTS
        .iter()
        .filter(|(name, _)| non_empty(name).is_none())
        .map(|(name, value)| (name.to_string(), value.to_string()))
        .collect();
    Ccc::Launch(LaunchSpec { args, env })
}

fn is_flag(arg: &str, flag: &str) -> bool {
    arg == flag || arg.strip_prefix(flag).is_some_and(|rest| rest.starts_with('='))
}

/// The plugin directories under `plugins_dir`, sorted by name; non-directories are skipped and a missing directory yields none.
fn plugin_dirs(plugins_dir: Option<&Path>) -> Vec<OsString> {
    let Some(entries) = plugins_dir.and_then(|dir| std::fs::read_dir(dir).ok()) else {
        return Vec::new();
    };
    let mut dirs: Vec<_> = entries.flatten().map(|entry| entry.path()).filter(|path| path.is_dir()).collect();
    dirs.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
    dirs.into_iter().map(|path| path.into_os_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn args(list: &[&str]) -> Vec<OsString> {
        list.iter().map(OsString::from).collect()
    }

    fn launch(user_args: &[&str], env: &[(&str, &str)]) -> LaunchSpec {
        let env: HashMap<String, String> = env.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        match ccc(&args(user_args), None, |name| env.get(name).cloned()) {
            Ccc::Launch(spec) => spec,
            Ccc::Update => panic!("expected a launch"),
        }
    }

    #[test]
    fn update_wins_anywhere() {
        for line in [&["-u"][..], &["--update"], &["--resume", "update", "x"]] {
            assert_eq!(ccc(&args(line), None, |_| None), Ccc::Update);
        }
    }

    #[test]
    fn env_effort_and_mode_follow_user_args() {
        let spec = launch(&["-p", "hi"], &[(EFFORT_ENV, "high"), (PERMISSION_MODE_ENV, "plan")]);
        assert_eq!(spec.args, args(&["-p", "hi", "--effort", "high", "--permission-mode", "plan"]));
    }

    #[test]
    fn user_flags_win_in_both_forms() {
        let env = [(EFFORT_ENV, "high"), (PERMISSION_MODE_ENV, "plan")];
        assert_eq!(launch(&["--effort", "low"], &env).args, args(&["--effort", "low", "--permission-mode", "plan"]));
        assert_eq!(launch(&["--permission-mode=auto"], &env).args, args(&["--permission-mode=auto", "--effort", "high"]));
        // A flag that merely starts with the same letters is not the user's effort.
        assert_eq!(launch(&["--effortless"], &env).args, args(&["--effortless", "--effort", "high", "--permission-mode", "plan"]));
    }

    #[test]
    fn telemetry_defaults_fill_only_unset_or_empty() {
        let spec = launch(&[], &[("CLAUDE_CODE_ENABLE_TELEMETRY", "1"), ("CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS", "")]);
        assert_eq!(
            spec.env,
            vec![
                ("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC".to_string(), "1".to_string()),
                ("CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS".to_string(), "1".to_string()),
            ]
        );
    }

    #[test]
    fn plugin_dirs_are_sorted_directories_only() {
        let tmp = tempfile::tempdir().unwrap();
        for name in ["judgess", "code-cleanup"] {
            std::fs::create_dir(tmp.path().join(name)).unwrap();
        }
        std::fs::write(tmp.path().join("notes.txt"), "").unwrap();
        let Ccc::Launch(spec) = ccc(&[], Some(tmp.path()), |_| None) else { panic!("expected a launch") };
        let expected: Vec<OsString> = ["code-cleanup", "judgess"]
            .iter()
            .flat_map(|name| ["--plugin-dir".into(), tmp.path().join(name).into_os_string()])
            .collect();
        assert_eq!(spec.args, expected);
    }
}

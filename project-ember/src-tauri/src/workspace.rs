use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileInfo {
    pub name: String,
    pub path: String,
    pub repo_count: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceInfo {
    pub id: String,
    pub path: String,
    pub task_title: String,
    pub repos: Vec<String>,
    pub active: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoStatus {
    pub name: String,
    pub branch: String,
    pub is_detached: bool,
    pub modified: usize,
    pub untracked: usize,
    pub timing: Option<RepoStatusTiming>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoStatusTiming {
    pub total_ms: f64,
    pub git_status_ms: f64,
    pub git_rev_parse_ms: Option<f64>,
    pub spawn_overhead_ms: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceStatusResult {
    pub statuses: Vec<RepoStatus>,
    pub thread_spawn_ms: f64,
    pub thread_join_ms: f64,
    pub handler_total_ms: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoInfo {
    pub name: String,
    pub path: String,
    pub source: String, // "registered" | "auto-discovered"
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CreatedWorkspace {
    pub path: String,
    pub name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepurposeContext {
    pub current_title: String,
}

fn home_dir() -> Result<PathBuf, String> {
    std::env::var("HOME")
        .map(PathBuf::from)
        .map_err(|_| "HOME environment variable not set".to_string())
}

pub(crate) fn expand_tilde(path: &str) -> Result<String, String> {
    if path == "~" {
        return Ok(home_dir()?.to_string_lossy().into_owned());
    }
    if let Some(rest) = path.strip_prefix("~/") {
        let home = home_dir()?;
        Ok(home.join(rest).to_string_lossy().into_owned())
    } else {
        Ok(path.to_string())
    }
}

pub(crate) fn global_config_path() -> Result<PathBuf, String> {
    if let Ok(override_path) = std::env::var("DEVORA_CONFIG_PATH") {
        return Ok(PathBuf::from(override_path));
    }
    let home = home_dir()?;
    Ok(home.join(".config/devora/config.json"))
}

pub(crate) fn read_json_file(path: &Path) -> Result<Value, String> {
    let content =
        fs::read_to_string(path).map_err(|e| format!("failed to read {}: {e}", path.display()))?;
    serde_json::from_str(&content)
        .map_err(|e| format!("failed to parse {}: {e}", path.display()))
}

pub(crate) fn write_pretty_json(path: &Path, value: &Value) -> Result<(), String> {
    let mut buf = Vec::new();
    let formatter = serde_json::ser::PrettyFormatter::with_indent(b"    ");
    let mut serializer = serde_json::Serializer::with_formatter(&mut buf, formatter);
    value
        .serialize(&mut serializer)
        .map_err(|e| format!("failed to serialize {}: {e}", path.display()))?;
    fs::write(path, buf).map_err(|e| format!("failed to write {}: {e}", path.display()))
}

fn get_value_by_dot_path<'a>(root: &'a Value, dot_path: &str) -> Option<&'a Value> {
    let mut current = root;
    for key in dot_path.split('.') {
        current = current.get(key)?;
    }
    Some(current)
}

pub(crate) fn list_repo_subdirs(dir: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut repos: Vec<String> = entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                return None;
            }
            if entry.file_type().ok()?.is_dir() {
                Some(name)
            } else {
                None
            }
        })
        .collect();

    repos.sort();
    repos
}

fn discover_repos_from_repos_dir(profile_path: &Path) -> Vec<RepoInfo> {
    let repos_dir = profile_path.join("repos");
    let Ok(entries) = fs::read_dir(&repos_dir) else {
        return Vec::new();
    };

    let mut repos: Vec<RepoInfo> = entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            if !entry.file_type().ok()?.is_dir() {
                return None;
            }
            let subdir_path = entry.path();
            if subdir_path.join(".git").exists() {
                let name = entry.file_name().to_string_lossy().into_owned();
                Some(RepoInfo {
                    name,
                    path: subdir_path.to_string_lossy().into_owned(),
                    source: "auto-discovered".to_string(),
                })
            } else {
                None
            }
        })
        .collect();

    repos.sort_by(|a, b| a.name.cmp(&b.name));
    repos
}

pub fn list_profiles() -> Result<Vec<ProfileInfo>, String> {
    let config_path = global_config_path()?;
    if !config_path.exists() {
        return Ok(Vec::new());
    }

    let config = read_json_file(&config_path)?;

    let profiles_val = match config.get("profiles") {
        Some(v) => v,
        None => return Ok(Vec::new()),
    };

    let profile_paths = profiles_val
        .as_array()
        .ok_or("\"profiles\" is not an array in global config")?;

    let mut result = Vec::new();
    for path_val in profile_paths {
        let path_str = match path_val.as_str() {
            Some(s) => expand_tilde(s)?,
            None => continue,
        };

        let profile_dir = Path::new(&path_str);
        let profile_config_path = profile_dir.join("config.json");
        if !profile_config_path.exists() {
            continue;
        }

        let profile_config = match read_json_file(&profile_config_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let name = match profile_config.get("name").and_then(|v| v.as_str()) {
            Some(n) => n.to_string(),
            None => continue, // skip profiles without a name
        };

        let registered = get_registered_repos(&path_str)?;

        result.push(ProfileInfo {
            name,
            path: path_str,
            repo_count: registered.len(),
        });
    }

    Ok(result)
}

pub fn list_workspaces(
    profile_path: &str,
    warnings: &mut Vec<String>,
) -> Result<Vec<WorkspaceInfo>, String> {
    let workspaces_dir = Path::new(profile_path).join("workspaces");
    if !workspaces_dir.exists() {
        return Ok(Vec::new());
    }

    let entries = fs::read_dir(&workspaces_dir)
        .map_err(|e| format!("failed to read workspaces dir: {e}"))?;

    let mut workspaces = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("failed to read dir entry: {e}"))?;
        let dir_name = entry.file_name().to_string_lossy().into_owned();

        if !dir_name.starts_with("ws-") {
            continue;
        }
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }

        let ws_path = entry.path();
        let initialized = ws_path.join("initialized").exists();
        if !initialized {
            continue;
        }

        let task_path = ws_path.join("task.json");
        let active = task_path.exists();

        let task_title = if active {
            match read_task_title(&task_path) {
                Ok(title) => title,
                Err(e) => {
                    // The workspace is still listed (with an empty title), but the corrupt task.json must not go unnoticed
                    warnings.push(e);
                    String::new()
                }
            }
        } else {
            String::new()
        };

        let repos = list_repo_subdirs(&ws_path);

        workspaces.push(WorkspaceInfo {
            id: dir_name,
            path: ws_path.to_string_lossy().into_owned(),
            task_title,
            repos,
            active,
        });
    }

    workspaces.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(workspaces)
}

pub fn get_workspace_status(
    workspace_path: &str,
    repo_names: Vec<String>,
) -> Result<WorkspaceStatusResult, String> {
    use std::time::Instant;

    let handler_start = Instant::now();

    let ws_dir = Path::new(workspace_path);
    if !ws_dir.exists() {
        return Err(format!("workspace path does not exist: {workspace_path}"));
    }

    let spawn_start = Instant::now();
    let handles: Vec<_> = repo_names
        .into_iter()
        .map(|name| {
            let repo_path = ws_dir.join(&name);
            thread::spawn(move || get_single_repo_status(&name, &repo_path))
        })
        .collect();
    let thread_spawn_ms = spawn_start.elapsed().as_secs_f64() * 1000.0;

    let join_start = Instant::now();
    let mut statuses = Vec::new();
    for handle in handles {
        match handle.join() {
            Ok(Ok(status)) => statuses.push(status),
            Ok(Err(e)) => return Err(e),
            Err(_) => return Err("thread panicked while getting repo status".to_string()),
        }
    }
    let thread_join_ms = join_start.elapsed().as_secs_f64() * 1000.0;

    statuses.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(WorkspaceStatusResult {
        statuses,
        thread_spawn_ms,
        thread_join_ms,
        handler_total_ms: handler_start.elapsed().as_secs_f64() * 1000.0,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceStatusInput {
    pub workspace_path: String,
    pub repo_names: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchWorkspaceStatusResult {
    pub workspace_statuses: Vec<SingleWorkspaceStatus>,
    pub handler_total_ms: f64,
    pub thread_spawn_ms: f64,
    pub thread_join_ms: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SingleWorkspaceStatus {
    pub workspace_path: String,
    pub statuses: Vec<RepoStatus>,
    pub error: Option<String>,
}

const MAX_CONCURRENT_GIT: usize = 5;

pub fn get_all_workspace_statuses(
    workspaces: Vec<WorkspaceStatusInput>,
    warnings: &mut Vec<String>,
) -> Result<BatchWorkspaceStatusResult, String> {
    use std::sync::{Arc, Condvar, Mutex};
    use std::time::Instant;

    let handler_start = Instant::now();

    let semaphore = Arc::new((Mutex::new(MAX_CONCURRENT_GIT), Condvar::new()));

    let spawn_start = Instant::now();
    let handles: Vec<_> = workspaces
        .iter()
        .flat_map(|ws| {
            let ws_path = ws.workspace_path.clone();
            let semaphore = semaphore.clone();
            ws.repo_names.iter().map(move |name| {
                let ws_dir = PathBuf::from(&ws_path);
                let repo_path = ws_dir.join(name);
                let name = name.clone();
                let ws_path = ws_path.clone();
                let sem = semaphore.clone();
                thread::spawn(move || {
                    let (lock, cvar) = &*sem;
                    {
                        let mut slots = cvar
                            .wait_while(lock.lock().unwrap(), |s| *s == 0)
                            .unwrap();
                        *slots -= 1;
                    }
                    let result = get_single_repo_status(&name, &repo_path);
                    {
                        let mut slots = lock.lock().unwrap();
                        *slots += 1;
                        cvar.notify_one();
                    }
                    (ws_path, result)
                })
            })
        })
        .collect();
    let thread_spawn_ms = spawn_start.elapsed().as_secs_f64() * 1000.0;

    let join_start = Instant::now();
    let mut results_by_ws: std::collections::HashMap<String, Result<Vec<RepoStatus>, String>> =
        std::collections::HashMap::new();
    for handle in handles {
        match handle.join() {
            Ok((ws_path, repo_result)) => {
                let entry = results_by_ws
                    .entry(ws_path)
                    .or_insert_with(|| Ok(Vec::new()));
                match repo_result {
                    Ok(status) => {
                        if let Ok(statuses) = entry {
                            statuses.push(status);
                        }
                    }
                    Err(e) => {
                        *entry = Err(e);
                    }
                }
            }
            Err(_) => {
                warnings.push("thread panicked while getting repo status".to_string());
            }
        }
    }
    let thread_join_ms = join_start.elapsed().as_secs_f64() * 1000.0;

    let mut workspace_statuses: Vec<SingleWorkspaceStatus> = Vec::new();
    for ws in &workspaces {
        match results_by_ws.remove(&ws.workspace_path) {
            Some(Ok(mut statuses)) => {
                statuses.sort_by(|a, b| a.name.cmp(&b.name));
                workspace_statuses.push(SingleWorkspaceStatus {
                    workspace_path: ws.workspace_path.clone(),
                    statuses,
                    error: None,
                });
            }
            Some(Err(e)) => {
                workspace_statuses.push(SingleWorkspaceStatus {
                    workspace_path: ws.workspace_path.clone(),
                    statuses: Vec::new(),
                    error: Some(e),
                });
            }
            None => {
                workspace_statuses.push(SingleWorkspaceStatus {
                    workspace_path: ws.workspace_path.clone(),
                    statuses: Vec::new(),
                    error: None,
                });
            }
        }
    }

    Ok(BatchWorkspaceStatusResult {
        workspace_statuses,
        handler_total_ms: handler_start.elapsed().as_secs_f64() * 1000.0,
        thread_spawn_ms,
        thread_join_ms,
    })
}

fn get_single_repo_status(name: &str, repo_path: &Path) -> Result<RepoStatus, String> {
    use std::time::Instant;

    let total_start = Instant::now();

    let spawn_start = Instant::now();
    let status_output = Command::new("git")
        .args(["--no-optional-locks", "status", "--branch", "--porcelain"])
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("failed to run git status in {name}: {e}"))?;
    let git_status_ms = spawn_start.elapsed().as_secs_f64() * 1000.0;

    let status_text = String::from_utf8_lossy(&status_output.stdout);
    let parsed = parse_git_status_branch_porcelain(&status_text);

    let mut git_rev_parse_ms = None;
    let branch = if parsed.is_detached {
        let rev_start = Instant::now();
        let hash_output = Command::new("git")
            .args(["--no-optional-locks", "rev-parse", "--short", "HEAD"])
            .current_dir(repo_path)
            .output()
            .map_err(|e| format!("failed to get commit hash in {name}: {e}"))?;
        git_rev_parse_ms = Some(rev_start.elapsed().as_secs_f64() * 1000.0);
        String::from_utf8_lossy(&hash_output.stdout)
            .trim()
            .to_string()
    } else {
        parsed.branch
    };

    let total_ms = total_start.elapsed().as_secs_f64() * 1000.0;
    let spawn_overhead_ms = total_ms - git_status_ms - git_rev_parse_ms.unwrap_or(0.0);

    Ok(RepoStatus {
        name: name.to_string(),
        branch,
        is_detached: parsed.is_detached,
        modified: parsed.modified,
        untracked: parsed.untracked,
        timing: Some(RepoStatusTiming {
            total_ms,
            git_status_ms,
            git_rev_parse_ms,
            spawn_overhead_ms,
        }),
    })
}

pub fn get_registered_repos(profile_path: &str) -> Result<Vec<RepoInfo>, String> {
    let profile_dir = Path::new(profile_path);

    // Explicit repos from config.json
    let mut repos = Vec::new();
    let config_path = profile_dir.join("config.json");
    if config_path.exists() {
        let config = read_json_file(&config_path)?;
        if let Some(repo_paths) = config.get("repos").and_then(|v| v.as_array()) {
            for path_val in repo_paths {
                if let Some(path_str) = path_val.as_str() {
                    let expanded = expand_tilde(path_str)?;
                    let repo_path = Path::new(&expanded);
                    let name = repo_path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    repos.push(RepoInfo {
                        name,
                        path: expanded,
                        source: "registered".to_string(),
                    });
                }
            }
        }
    }

    // Auto-discovered repos from <profile>/repos/
    let discovered = discover_repos_from_repos_dir(profile_dir);
    for repo in discovered {
        if !repos.iter().any(|r| r.path == repo.path) {
            repos.push(repo);
        }
    }

    repos.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(repos)
}

pub fn get_config_value(key: &str) -> Result<Option<Value>, String> {
    let config_path = global_config_path()?;
    if !config_path.exists() {
        return Ok(None);
    }

    let config = read_json_file(&config_path)?;
    // The config uses kebab-case keys, so convert dots to navigate
    Ok(get_value_by_dot_path(&config, key).cloned())
}

/// Resolves the session terminal app command (`terminal.default-app`, profile → user).
/// Returns `None` when unset at both scopes, which the caller treats as a bare login shell.
/// Routes through the same resolver as the Settings Hub so the displayed and the launched value cannot disagree.
pub fn get_default_app(profile_path: &str) -> Result<Option<String>, String> {
    let user_cfg = read_config_opt(&global_config_path()?)?;
    let profile_cfg = read_config_opt(&profile_config_path(profile_path))?;
    let spec = config_field("terminal.default-app")
        .expect("terminal.default-app is a registered config field");
    Ok(resolve_config(profile_cfg.as_ref(), user_cfg.as_ref(), spec)
        .and_then(|v| v.as_str().map(str::to_string)))
}

/// Resolves the configured prepare-command (`prepare-command`, profile → user).
/// Returns `None` when unset at both scopes.
/// A malformed/unreadable config degrades to `None` (prepare is skipped) rather than failing workspace creation.
pub fn get_prepare_command(profile_path: Option<&str>) -> Option<String> {
    let user_cfg = global_config_path().ok().and_then(|p| read_config_lenient(&p));
    let profile_cfg = profile_path.and_then(|p| read_config_lenient(&profile_config_path(p)));
    let spec = config_field("prepare-command")?;
    resolve_config(profile_cfg.as_ref(), user_cfg.as_ref(), spec)
        .and_then(|v| v.as_str().map(str::to_string))
}

/// Devora's built-in default for `terminal.git-shortcuts`.
/// Shared with the Settings Hub registry so the displayed default and the PTY consumer below cannot drift.
pub const DEFAULT_GIT_SHORTCUTS: bool = true;

/// Resolves whether Debi git-shortcut command shims should be placed on the PATH of session shells.
/// Profile-level `terminal.git-shortcuts` wins, then global config, then defaults to `true` (feature enabled).
/// A malformed or unreadable config degrades to the default rather than failing PTY creation.
pub fn get_git_shortcuts_enabled(profile_path: Option<&str>) -> bool {
    if let Some(profile_path) = profile_path {
        let profile_config_path = Path::new(profile_path).join("config.json");
        if profile_config_path.exists() {
            if let Ok(profile_config) = read_json_file(&profile_config_path) {
                if let Some(val) = get_value_by_dot_path(&profile_config, "terminal.git-shortcuts") {
                    if let Some(b) = val.as_bool() {
                        return b;
                    }
                }
            }
        }
    }

    match get_config_value("terminal.git-shortcuts") {
        Ok(Some(val)) => val.as_bool().unwrap_or(DEFAULT_GIT_SHORTCUTS),
        _ => DEFAULT_GIT_SHORTCUTS,
    }
}

// ── Claude Code launch configuration (model tiers + effort) ──
//
// `ccc` launches Claude Code with a per-tier model mapping and an effort level.
// Each is resolved per key with the precedence profile → user → Devora default, where a level may hold an explicit string (a value), JSON `null` (= "None": impose nothing, so Claude Code falls back to its own default), or omit the key (fall through to the next level).
// The resolved values are injected as environment variables by the PTY layer (see commands.rs).

/// One configurable Claude launch setting: its kebab-case config key (under the top-level `claude` object), the environment variable the PTY exports, and the Devora default value.
struct ClaudeSettingSpec {
    key: &'static str,
    env_var: &'static str,
    default: &'static str,
}

const CLAUDE_SETTINGS: [ClaudeSettingSpec; 4] = [
    ClaudeSettingSpec {
        key: "opus-model",
        env_var: "ANTHROPIC_DEFAULT_OPUS_MODEL",
        default: "claude-opus-4-8",
    },
    ClaudeSettingSpec {
        key: "sonnet-model",
        env_var: "ANTHROPIC_DEFAULT_SONNET_MODEL",
        default: "claude-opus-4-8",
    },
    ClaudeSettingSpec {
        key: "haiku-model",
        env_var: "ANTHROPIC_DEFAULT_HAIKU_MODEL",
        default: "claude-sonnet-4-6",
    },
    ClaudeSettingSpec {
        key: "effort",
        env_var: "DEVORA_CCC_EFFORT",
        default: "xhigh",
    },
];

/// Valid Claude Code effort levels (low → max).
/// Mirrored in the Ember UI dropdown (`src/ui/components/ClaudeConfigCard.ts`) — keep the two lists in sync.
pub const CLAUDE_EFFORT_LEVELS: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];

/// One resolved setting: either a concrete value to apply, or None (impose nothing).
enum ResolvedSetting {
    Value(String),
    None,
}

/// Result of `read_claude_settings`: the raw values stored at one scope plus the effective values after full precedence resolution.
/// Both maps are keyed by the kebab config key.
#[derive(Serialize)]
pub struct ClaudeSettingsResponse {
    /// Per key: the raw stored value at this scope — a string, JSON `null`, or the key is
    /// omitted entirely (meaning "Default" / not set at this scope).
    stored: serde_json::Map<String, Value>,
    /// Per key (all four always present): the effective resolved value — a string, or JSON
    /// `null` meaning None (no override; Claude Code uses its own default).
    resolved: serde_json::Map<String, Value>,
}

/// Reads a config file, returning `Ok(None)` when it does not exist.
fn read_config_opt(path: &Path) -> Result<Option<Value>, String> {
    if path.exists() {
        Ok(Some(read_json_file(path)?))
    } else {
        Ok(None)
    }
}

/// Best-effort read used on the PTY hot path: a missing or unreadable config degrades to `None` (treated as absent) rather than failing session creation.
fn read_config_lenient(path: &Path) -> Option<Value> {
    if path.exists() {
        read_json_file(path).ok()
    } else {
        None
    }
}

fn profile_config_path(profile_path: &str) -> PathBuf {
    Path::new(profile_path).join("config.json")
}

/// Resolves one setting across profile → user → Devora default.
/// A present non-empty string wins as a value; a present JSON `null` wins as None and stops the chain; an absent key, an empty/whitespace string, or any other malformed type falls through to the next level.
fn resolve_one(
    profile_cfg: Option<&Value>,
    user_cfg: Option<&Value>,
    spec: &ClaudeSettingSpec,
) -> ResolvedSetting {
    let dot_path = format!("claude.{}", spec.key);
    for cfg in [profile_cfg, user_cfg].into_iter().flatten() {
        match get_value_by_dot_path(cfg, &dot_path) {
            Some(Value::String(s)) if !s.trim().is_empty() => {
                return ResolvedSetting::Value(s.clone());
            }
            Some(Value::Null) => return ResolvedSetting::None,
            _ => {} // absent / empty / malformed → fall through
        }
    }
    ResolvedSetting::Value(spec.default.to_string())
}

/// Resolves the Claude launch settings for a session and returns the environment variables the PTY should export.
/// Keys resolved to None are omitted (Claude Code uses its default).
pub fn claude_launch_env(profile_path: Option<&str>) -> HashMap<String, String> {
    let user_cfg = global_config_path()
        .ok()
        .and_then(|p| read_config_lenient(&p));
    let profile_cfg = profile_path.and_then(|p| read_config_lenient(&profile_config_path(p)));

    let mut env = HashMap::new();
    for spec in &CLAUDE_SETTINGS {
        if let ResolvedSetting::Value(v) = resolve_one(profile_cfg.as_ref(), user_cfg.as_ref(), spec)
        {
            env.insert(spec.env_var.to_string(), v);
        }
    }
    env
}

/// Reads the Claude settings for the Settings Hub UI at the given scope (a profile path, or `None` for the user-level/global scope): the raw stored values at that scope plus the effective values after full resolution.
/// A corrupt config surfaces as an error.
pub fn read_claude_settings(profile_path: Option<&str>) -> Result<ClaudeSettingsResponse, String> {
    let user_cfg = read_config_opt(&global_config_path()?)?;
    let profile_cfg = match profile_path {
        Some(p) => read_config_opt(&profile_config_path(p))?,
        None => None,
    };

    // The scope's own config holds the raw "stored" values.
    let own_cfg = if profile_path.is_some() {
        profile_cfg.as_ref()
    } else {
        user_cfg.as_ref()
    };

    let mut stored = serde_json::Map::new();
    if let Some(claude) = own_cfg.and_then(|c| c.get("claude")).and_then(|v| v.as_object()) {
        for spec in &CLAUDE_SETTINGS {
            // Preserve string and null verbatim; ignore malformed types.
            if let Some(val) = claude.get(spec.key).filter(|v| v.is_string() || v.is_null()) {
                stored.insert(spec.key.to_string(), val.clone());
            }
        }
    }

    let mut resolved = serde_json::Map::new();
    for spec in &CLAUDE_SETTINGS {
        let value = match resolve_one(profile_cfg.as_ref(), user_cfg.as_ref(), spec) {
            ResolvedSetting::Value(s) => Value::String(s),
            ResolvedSetting::None => Value::Null,
        };
        resolved.insert(spec.key.to_string(), value);
    }

    Ok(ClaudeSettingsResponse { stored, resolved })
}

/// Writes one Claude setting at the given scope, preserving sibling keys.
/// `state`: `"value"` (store `value` as a string), `"none"` (store JSON null), `"default"` (remove the key so it falls through).
/// Empty/whitespace values are rejected; effort values are validated against `CLAUDE_EFFORT_LEVELS`.
pub fn write_claude_setting(
    profile_path: Option<&str>,
    key: &str,
    state: &str,
    value: Option<&str>,
) -> Result<(), String> {
    let spec = CLAUDE_SETTINGS
        .iter()
        .find(|s| s.key == key)
        .ok_or_else(|| format!("unknown claude setting key: {key}"))?;

    let config_path = match profile_path {
        Some(p) => profile_config_path(p),
        None => global_config_path()?,
    };

    let mut config = read_config_opt(&config_path)?
        .unwrap_or_else(|| Value::Object(serde_json::Map::new()));
    let obj = config
        .as_object_mut()
        .ok_or("config is not a JSON object")?;

    let claude_empty = {
        let claude = obj
            .entry("claude")
            .or_insert_with(|| Value::Object(serde_json::Map::new()));
        let claude_obj = claude
            .as_object_mut()
            .ok_or("\"claude\" is not an object in config")?;

        match state {
            "value" => {
                let v = value
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| format!("a non-empty value is required to set {key}"))?;
                if spec.key == "effort" && !CLAUDE_EFFORT_LEVELS.contains(&v) {
                    return Err(format!(
                        "invalid effort level: {v} (expected one of {})",
                        CLAUDE_EFFORT_LEVELS.join(", ")
                    ));
                }
                claude_obj.insert(key.to_string(), Value::String(v.to_string()));
            }
            "none" => {
                claude_obj.insert(key.to_string(), Value::Null);
            }
            "default" => {
                claude_obj.remove(key);
            }
            other => {
                return Err(format!(
                    "unknown state: {other} (expected value, none, or default)"
                ))
            }
        }

        claude_obj.is_empty()
    };

    // Keep configs tidy: drop the `claude` object once it holds no overrides.
    if claude_empty {
        obj.remove("claude");
    }

    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create {}: {e}", parent.display()))?;
    }
    write_pretty_json(&config_path, &config)
}

// ── Generic config settings (Settings Hub) ──
//
// The config keys (besides Claude) that the Settings Hub edits, resolved per key with the precedence profile → user → Devora built-in.
// Unlike Claude settings, these have no `null`/"None" state: a key is either stored at a scope or absent (inherit).
// Debi (Go) reads the same keys from the same files, so each value must keep its JSON type (a bool stays a bool, not the string "true").

/// A configurable field's value type, which drives validation and JSON encoding.
#[derive(Clone, Copy)]
enum ConfigFieldKind {
    Text,
    Bool,
    Enum(&'static [&'static str]),
}

/// Devora's built-in default for a field, shown as the resolved "Default" hint when nothing is stored at any scope.
/// `None` for keys whose real default lives in Debi (Go) - advertising a value here would duplicate Go's default and risk drift, so the UI shows a generic label instead.
enum BuiltinDefault {
    None,
    Bool(bool),
}

struct ConfigFieldSpec {
    /// Dot-path under the config root (e.g. `terminal.default-app`, `task-tracker.asana.project-id`).
    key: &'static str,
    kind: ConfigFieldKind,
    builtin_default: BuiltinDefault,
}

const CONFIG_FIELDS: &[ConfigFieldSpec] = &[
    ConfigFieldSpec {
        key: "terminal.default-app",
        kind: ConfigFieldKind::Text,
        builtin_default: BuiltinDefault::None, // unset = bare login shell
    },
    ConfigFieldSpec {
        key: "terminal.git-shortcuts",
        kind: ConfigFieldKind::Bool,
        builtin_default: BuiltinDefault::Bool(DEFAULT_GIT_SHORTCUTS),
    },
    ConfigFieldSpec {
        key: "prepare-command",
        kind: ConfigFieldKind::Text,
        builtin_default: BuiltinDefault::None,
    },
    ConfigFieldSpec {
        key: "pr.auto-merge",
        kind: ConfigFieldKind::Bool,
        builtin_default: BuiltinDefault::None, // Debi owns the default (and a per-repo tier exists)
    },
    ConfigFieldSpec {
        key: "feature.branch-prefix",
        kind: ConfigFieldKind::Text,
        builtin_default: BuiltinDefault::None,
    },
    ConfigFieldSpec {
        key: "task-tracker.provider",
        kind: ConfigFieldKind::Enum(&["asana"]),
        builtin_default: BuiltinDefault::None,
    },
    ConfigFieldSpec {
        key: "task-tracker.asana.workspace-id",
        kind: ConfigFieldKind::Text,
        builtin_default: BuiltinDefault::None,
    },
    ConfigFieldSpec {
        key: "task-tracker.asana.project-id",
        kind: ConfigFieldKind::Text,
        builtin_default: BuiltinDefault::None,
    },
    ConfigFieldSpec {
        key: "task-tracker.asana.cli-tag",
        kind: ConfigFieldKind::Text,
        builtin_default: BuiltinDefault::None,
    },
    ConfigFieldSpec {
        key: "task-tracker.asana.section-id",
        kind: ConfigFieldKind::Text,
        builtin_default: BuiltinDefault::None,
    },
];

fn config_field(key: &str) -> Option<&'static ConfigFieldSpec> {
    CONFIG_FIELDS.iter().find(|s| s.key == key)
}

fn builtin_default_value(d: &BuiltinDefault) -> Option<Value> {
    match d {
        BuiltinDefault::None => None,
        BuiltinDefault::Bool(b) => Some(Value::Bool(*b)),
    }
}

/// The raw value of `spec.key` stored in one scope's config, coerced to the field's expected JSON type.
/// Returns `None` when the key is absent or stored with a mismatched type.
fn stored_config_value(cfg: Option<&Value>, spec: &ConfigFieldSpec) -> Option<Value> {
    let val = get_value_by_dot_path(cfg?, spec.key)?;
    match spec.kind {
        ConfigFieldKind::Bool => val.as_bool().map(Value::Bool),
        ConfigFieldKind::Text | ConfigFieldKind::Enum(_) => {
            val.as_str().map(|s| Value::String(s.to_string()))
        }
    }
}

/// Resolves `spec.key` across profile → user → Devora built-in.
/// Returns `None` only when the key is unset everywhere and there is no built-in default (a Debi-owned key).
fn resolve_config(
    profile_cfg: Option<&Value>,
    user_cfg: Option<&Value>,
    spec: &ConfigFieldSpec,
) -> Option<Value> {
    stored_config_value(profile_cfg, spec)
        .or_else(|| stored_config_value(user_cfg, spec))
        .or_else(|| builtin_default_value(&spec.builtin_default))
}

/// Result of `read_config_settings`: the raw values stored at one scope plus the effective values after full precedence resolution.
/// Both maps are keyed by the dot-path config key.
#[derive(Serialize)]
pub struct ConfigSettingsResponse {
    /// Per key present at this scope: the raw stored value (a string or bool). Absent keys are omitted, which the UI reads as "Default" (inherit) for that scope.
    stored: serde_json::Map<String, Value>,
    /// Per key (all present): the effective value after profile → user → built-in, or JSON `null` when nothing is set and Devora has no built-in to advertise (a Debi-owned default).
    resolved: serde_json::Map<String, Value>,
}

/// Reads the generic config settings for the Settings Hub at the given scope (a profile path, or `None` for the user-level/global scope): the raw stored values at that scope plus the effective values after full resolution. A corrupt config surfaces as an error.
pub fn read_config_settings(profile_path: Option<&str>) -> Result<ConfigSettingsResponse, String> {
    let user_cfg = read_config_opt(&global_config_path()?)?;
    let profile_cfg = match profile_path {
        Some(p) => read_config_opt(&profile_config_path(p))?,
        None => None,
    };
    let own_cfg = if profile_path.is_some() {
        profile_cfg.as_ref()
    } else {
        user_cfg.as_ref()
    };

    let mut stored = serde_json::Map::new();
    let mut resolved = serde_json::Map::new();
    for spec in CONFIG_FIELDS {
        if let Some(v) = stored_config_value(own_cfg, spec) {
            stored.insert(spec.key.to_string(), v);
        }
        let effective =
            resolve_config(profile_cfg.as_ref(), user_cfg.as_ref(), spec).unwrap_or(Value::Null);
        resolved.insert(spec.key.to_string(), effective);
    }

    Ok(ConfigSettingsResponse { stored, resolved })
}

/// Coerces a UI-supplied string into the field's JSON value, validating the field kind.
/// An empty value is rejected for `Text` (clear via the `default` state instead) but accepted for `Enum` as the explicit "none" choice (e.g. disabling a tracker inherited from a higher scope).
fn coerce_config_value(spec: &ConfigFieldSpec, value: Option<&str>) -> Result<Value, String> {
    let raw = value.unwrap_or("");
    match spec.kind {
        ConfigFieldKind::Bool => match raw {
            "true" => Ok(Value::Bool(true)),
            "false" => Ok(Value::Bool(false)),
            other => Err(format!("invalid boolean for {}: {other}", spec.key)),
        },
        ConfigFieldKind::Text => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return Err(format!("a non-empty value is required to set {}", spec.key));
            }
            Ok(Value::String(trimmed.to_string()))
        }
        ConfigFieldKind::Enum(allowed) => {
            if raw.is_empty() {
                return Ok(Value::String(String::new()));
            }
            if !allowed.contains(&raw) {
                return Err(format!(
                    "invalid value for {}: {raw} (expected one of {})",
                    spec.key,
                    allowed.join(", ")
                ));
            }
            Ok(Value::String(raw.to_string()))
        }
    }
}

/// Writes one generic config setting at the given scope, preserving sibling keys.
/// `state`: `"value"` (store the coerced `value`), `"default"` (remove the key so it falls through).
pub fn write_config_setting(
    profile_path: Option<&str>,
    key: &str,
    state: &str,
    value: Option<&str>,
) -> Result<(), String> {
    let spec = config_field(key).ok_or_else(|| format!("unknown config key: {key}"))?;

    let config_path = match profile_path {
        Some(p) => profile_config_path(p),
        None => global_config_path()?,
    };
    let mut config =
        read_config_opt(&config_path)?.unwrap_or_else(|| Value::Object(serde_json::Map::new()));
    if !config.is_object() {
        return Err("config is not a JSON object".to_string());
    }

    let new_value = match state {
        "value" => Some(coerce_config_value(spec, value)?),
        "default" => None,
        other => {
            return Err(format!(
                "unknown state: {other} (expected value or default)"
            ))
        }
    };
    write_dot_path(&mut config, key, new_value)?;

    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create {}: {e}", parent.display()))?;
    }
    write_pretty_json(&config_path, &config)
}

/// Sets (`Some`) or removes (`None`) a dot-path key in a JSON object, creating intermediate objects on set and pruning intermediate objects left empty after a remove.
fn write_dot_path(root: &mut Value, dot_path: &str, value: Option<Value>) -> Result<(), String> {
    let keys: Vec<&str> = dot_path.split('.').collect();
    set_or_remove_path(root, &keys, value)
}

fn set_or_remove_path(node: &mut Value, keys: &[&str], value: Option<Value>) -> Result<(), String> {
    let obj = node.as_object_mut().ok_or("config is not a JSON object")?;
    let (first, rest) = keys
        .split_first()
        .expect("a dot-path always has at least one segment");

    if rest.is_empty() {
        match value {
            Some(v) => {
                obj.insert(first.to_string(), v);
            }
            None => {
                obj.remove(*first);
            }
        }
        return Ok(());
    }

    match value {
        Some(v) => {
            let child = obj
                .entry(first.to_string())
                .or_insert_with(|| Value::Object(serde_json::Map::new()));
            if !child.is_object() {
                return Err(format!("\"{first}\" is not an object in config"));
            }
            set_or_remove_path(child, rest, Some(v))
        }
        None => {
            // Remove: descend only if the intermediate exists and is an object, then prune it if empty.
            if let Some(child) = obj.get_mut(*first) {
                if child.is_object() {
                    set_or_remove_path(child, rest, None)?;
                    if child.as_object().is_some_and(serde_json::Map::is_empty) {
                        obj.remove(*first);
                    }
                }
            }
            Ok(())
        }
    }
}

pub(crate) fn find_next_workspace_id(workspaces_dir: &Path) -> Result<String, String> {
    if !workspaces_dir.exists() {
        return Ok("ws-1".to_string());
    }

    let entries =
        fs::read_dir(workspaces_dir).map_err(|e| format!("failed to read workspaces dir: {e}"))?;

    let mut existing_numbers: Vec<usize> = entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let name = entry.file_name().to_string_lossy().into_owned();
            name.strip_prefix("ws-")?.parse::<usize>().ok()
        })
        .collect();

    existing_numbers.sort();

    // Fill gaps: find first missing number starting from 1
    let mut next = 1;
    for n in &existing_numbers {
        if *n == next {
            next += 1;
        } else {
            break;
        }
    }

    Ok(format!("ws-{next}"))
}

pub(crate) fn mise_available() -> bool {
    Command::new("which")
        .arg("mise")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn remove_task(workspace_path: &str) -> Result<(), String> {
    let ws_dir = Path::new(workspace_path);
    let task_path = ws_dir.join("task.json");

    if !task_path.exists() {
        return Ok(());
    }

    let repo_names = list_repo_subdirs(ws_dir);

    for repo_name in &repo_names {
        let repo_path = ws_dir.join(repo_name);

        let reset = Command::new("git")
            .args(["reset", "--hard", "HEAD"])
            .current_dir(&repo_path)
            .output()
            .map_err(|e| format!("failed to run git reset in {repo_name}: {e}"))?;
        if !reset.status.success() {
            let stderr = String::from_utf8_lossy(&reset.stderr);
            return Err(format!("git reset failed in {repo_name}: {stderr}"));
        }

        let clean = Command::new("git")
            .args(["clean", "-fd"])
            .current_dir(&repo_path)
            .output()
            .map_err(|e| format!("failed to run git clean in {repo_name}: {e}"))?;
        if !clean.status.success() {
            let stderr = String::from_utf8_lossy(&clean.stderr);
            return Err(format!("git clean failed in {repo_name}: {stderr}"));
        }

        let detach = Command::new("git")
            .args(["checkout", "--detach"])
            .current_dir(&repo_path)
            .output()
            .map_err(|e| format!("failed to run git checkout --detach in {repo_name}: {e}"))?;
        if !detach.status.success() {
            let stderr = String::from_utf8_lossy(&detach.stderr);
            return Err(format!(
                "git checkout --detach failed in {repo_name}: {stderr}"
            ));
        }
    }

    fs::remove_file(&task_path)
        .map_err(|e| format!("failed to delete task.json: {e}"))?;

    Ok(())
}

fn read_task_title(task_path: &Path) -> Result<String, String> {
    read_json_file(task_path).map(|task| {
        task.get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    })
}

pub(crate) fn write_task_json(ws_path: &Path, title: &str) -> Result<(), String> {
    let task = serde_json::json!({
        "uid": Uuid::new_v4().to_string(),
        "title": title,
        "started_at": date_format("%Y-%m-%d"),
    });
    write_pretty_json(&ws_path.join("task.json"), &task)
}

/// Errors unless every repo worktree is idle: clean (no modified or untracked files) and on a detached HEAD.
/// The error lists each offending repo on a single line (the error banner collapses newlines).
fn ensure_no_active_work(ws_dir: &Path) -> Result<(), String> {
    let mut blockers = Vec::new();

    for repo_name in list_repo_subdirs(ws_dir) {
        let status = get_single_repo_status(&repo_name, &ws_dir.join(&repo_name))?;

        let mut reasons = Vec::new();
        if !status.is_detached {
            reasons.push(format!(
                "on branch '{}' (expected detached HEAD)",
                status.branch
            ));
        }
        if status.modified > 0 {
            reasons.push(format!("{} modified", status.modified));
        }
        if status.untracked > 0 {
            reasons.push(format!("{} untracked", status.untracked));
        }

        if !reasons.is_empty() {
            blockers.push(format!("{repo_name}: {}", reasons.join(", ")));
        }
    }

    if blockers.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "workspace not ready for a new task — {}",
            blockers.join("; ")
        ))
    }
}

fn active_task_path(workspace_path: &str) -> Result<PathBuf, String> {
    let ws_dir = Path::new(workspace_path);
    if !ws_dir.exists() {
        return Err(format!("workspace path does not exist: {workspace_path}"));
    }
    let task_path = ws_dir.join("task.json");
    if !task_path.exists() {
        return Err(format!(
            "no active task in workspace (task.json not found): {workspace_path}"
        ));
    }
    Ok(task_path)
}

pub fn prepare_repurpose_task(workspace_path: &str) -> Result<RepurposeContext, String> {
    let task_path = active_task_path(workspace_path)?;
    ensure_no_active_work(Path::new(workspace_path))?;
    let current_title = read_task_title(&task_path)?;
    Ok(RepurposeContext { current_title })
}

pub fn repurpose_task(workspace_path: &str, new_title: &str) -> Result<(), String> {
    active_task_path(workspace_path)?;
    let ws_dir = Path::new(workspace_path);
    ensure_no_active_work(ws_dir)?;
    write_task_json(ws_dir, new_title)
}

pub fn delete_workspace(workspace_path: &str) -> Result<(), String> {
    let ws_dir = Path::new(workspace_path);
    let repo_names = list_repo_subdirs(ws_dir);

    let handles: Vec<_> = repo_names
        .into_iter()
        .map(|repo_name| {
            let repo_path = ws_dir.join(&repo_name).to_path_buf();
            thread::spawn(move || {
                let result = Command::new("git")
                    .args(["worktree", "remove", "--force", "."])
                    .current_dir(&repo_path)
                    .output()
                    .map_err(|e| {
                        format!("failed to run git worktree remove in {repo_name}: {e}")
                    })?;
                if !result.status.success() {
                    if repo_path.exists() {
                        let stderr = String::from_utf8_lossy(&result.stderr);
                        return Err(format!(
                            "git worktree remove failed in {repo_name}: {stderr}"
                        ));
                    }
                    return Ok(());
                }
                Ok(())
            })
        })
        .collect();

    for handle in handles {
        match handle.join() {
            Ok(Ok(())) => {}
            Ok(Err(e)) => return Err(e),
            Err(_) => {
                return Err("thread panicked during worktree removal".to_string())
            }
        }
    }

    fs::remove_dir_all(ws_dir)
        .map_err(|e| format!("failed to remove workspace directory: {e}"))?;

    Ok(())
}

pub fn save_profiling_report(profile_path: &str, report_json: &str) -> Result<String, String> {
    let diagnostics_dir = Path::new(profile_path).join("diagnostics");
    fs::create_dir_all(&diagnostics_dir)
        .map_err(|e| format!("failed to create diagnostics dir: {e}"))?;

    let timestamp = date_format("%Y%m%d-%H%M%S");
    let filename = format!("hub-profile-{timestamp}.json");
    let file_path = diagnostics_dir.join(&filename);

    fs::write(&file_path, report_json)
        .map_err(|e| format!("failed to write profiling report: {e}"))?;

    Ok(file_path.to_string_lossy().into_owned())
}

fn date_format(fmt: &str) -> String {
    let output = Command::new("date")
        .args([&format!("+{fmt}")])
        .output()
        .expect("failed to run date command");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

struct GitStatusBranchPorcelain {
    branch: String,
    is_detached: bool,
    modified: usize,
    untracked: usize,
}

fn parse_git_status_branch_porcelain(output: &str) -> GitStatusBranchPorcelain {
    let mut branch = String::new();
    let mut is_detached = false;
    let mut modified = 0usize;
    let mut untracked = 0usize;

    for line in output.lines() {
        if let Some(header) = line.strip_prefix("## ") {
            if header.starts_with("HEAD (no branch)") {
                is_detached = true;
            } else if let Some(rest) = header.strip_prefix("No commits yet on ") {
                branch = rest.to_string();
            } else {
                branch = header
                    .split("...")
                    .next()
                    .unwrap_or("")
                    .to_string();
            }
        } else if line.starts_with("??") {
            untracked += 1;
        } else if !line.is_empty() {
            modified += 1;
        }
    }

    GitStatusBranchPorcelain {
        branch,
        is_detached,
        modified,
        untracked,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    // Serializes tests that mutate process-global env vars (HOME / DEVORA_CONFIG_PATH) read by `global_config_path`; concurrent mutation would make config reads non-deterministic.
    // `into_inner` recovers from a poisoned lock so one failing test doesn't cascade.
    static ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn test_expand_tilde_with_home() {
        let result = expand_tilde("~/some/path").unwrap();
        let home = std::env::var("HOME").unwrap();
        assert_eq!(result, format!("{home}/some/path"));
    }

    #[test]
    fn test_expand_tilde_without_tilde() {
        let result = expand_tilde("/absolute/path").unwrap();
        assert_eq!(result, "/absolute/path");
    }

    #[test]
    fn test_expand_tilde_bare() {
        let result = expand_tilde("~").unwrap();
        let home = std::env::var("HOME").unwrap();
        assert_eq!(result, home);
    }

    #[test]
    fn test_get_value_by_dot_path() {
        let json: Value = serde_json::json!({
            "terminal": {
                "default-app": "nvim"
            }
        });
        let val = get_value_by_dot_path(&json, "terminal.default-app");
        assert_eq!(val.unwrap().as_str().unwrap(), "nvim");
    }

    #[test]
    fn test_get_value_by_dot_path_missing() {
        let json: Value = serde_json::json!({"a": 1});
        let val = get_value_by_dot_path(&json, "b.c");
        assert!(val.is_none());
    }

    #[test]
    fn test_find_next_workspace_id_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let id = find_next_workspace_id(tmp.path()).unwrap();
        assert_eq!(id, "ws-1");
    }

    #[test]
    fn test_find_next_workspace_id_nonexistent() {
        let tmp = tempfile::tempdir().unwrap();
        let nonexistent = tmp.path().join("does-not-exist");
        let id = find_next_workspace_id(&nonexistent).unwrap();
        assert_eq!(id, "ws-1");
    }

    #[test]
    fn test_find_next_workspace_id_fills_gaps() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir(tmp.path().join("ws-1")).unwrap();
        fs::create_dir(tmp.path().join("ws-3")).unwrap();
        let id = find_next_workspace_id(tmp.path()).unwrap();
        assert_eq!(id, "ws-2");
    }

    #[test]
    fn test_find_next_workspace_id_sequential() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir(tmp.path().join("ws-1")).unwrap();
        fs::create_dir(tmp.path().join("ws-2")).unwrap();
        let id = find_next_workspace_id(tmp.path()).unwrap();
        assert_eq!(id, "ws-3");
    }

    #[test]
    fn test_list_repo_subdirs_skips_dotfiles() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir(tmp.path().join("repo-a")).unwrap();
        fs::create_dir(tmp.path().join(".hidden")).unwrap();
        fs::create_dir(tmp.path().join("repo-b")).unwrap();

        let repos = list_repo_subdirs(tmp.path());
        assert_eq!(repos, vec!["repo-a", "repo-b"]);
    }

    #[test]
    fn test_list_profiles_no_config() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        // When HOME points to a temp dir with no config, should return empty
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("HOME", tmp.path());
        let result = list_profiles().unwrap();
        assert!(result.is_empty());
        // Restore HOME
        std::env::remove_var("HOME");
    }

    #[test]
    fn test_list_workspaces_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let result = list_workspaces(&tmp.path().to_string_lossy(), &mut Vec::new()).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_list_workspaces_malformed_task_json_pushes_warning() {
        let tmp = tempfile::tempdir().unwrap();
        let ws1 = tmp.path().join("workspaces").join("ws-1");
        fs::create_dir_all(&ws1).unwrap();
        fs::write(ws1.join("initialized"), "").unwrap();
        fs::write(ws1.join("task.json"), "{not json").unwrap();

        let mut warnings = Vec::new();
        let result = list_workspaces(&tmp.path().to_string_lossy(), &mut warnings).unwrap();

        // The workspace is still listed (active, with an empty title)…
        assert_eq!(result.len(), 1);
        assert!(result[0].active);
        assert_eq!(result[0].task_title, "");

        // …but the parse failure is reported instead of swallowed.
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("failed to parse"), "warning: {}", warnings[0]);
        assert!(warnings[0].contains("task.json"), "warning: {}", warnings[0]);
    }

    #[test]
    fn test_list_workspaces_with_active_and_inactive() {
        let tmp = tempfile::tempdir().unwrap();
        let ws_dir = tmp.path().join("workspaces");

        // Active workspace
        let ws1 = ws_dir.join("ws-1");
        fs::create_dir_all(&ws1).unwrap();
        fs::write(ws1.join("initialized"), "").unwrap();
        fs::write(
            ws1.join("task.json"),
            r#"{"uid":"abc","title":"my task","started_at":"2026-01-01"}"#,
        )
        .unwrap();
        fs::create_dir(ws1.join("my-repo")).unwrap();

        // Inactive workspace
        let ws2 = ws_dir.join("ws-2");
        fs::create_dir_all(&ws2).unwrap();
        fs::write(ws2.join("initialized"), "").unwrap();

        // Non-workspace dir (no initialized marker)
        let ws3 = ws_dir.join("ws-3");
        fs::create_dir_all(&ws3).unwrap();

        let result = list_workspaces(&tmp.path().to_string_lossy(), &mut Vec::new()).unwrap();
        assert_eq!(result.len(), 2);

        assert_eq!(result[0].id, "ws-1");
        assert!(result[0].active);
        assert_eq!(result[0].task_title, "my task");
        assert_eq!(result[0].repos, vec!["my-repo"]);

        assert_eq!(result[1].id, "ws-2");
        assert!(!result[1].active);
        assert_eq!(result[1].task_title, "");
    }

    #[test]
    fn test_discover_repos_from_repos_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let repos_dir = tmp.path().join("repos");
        fs::create_dir_all(&repos_dir).unwrap();

        // Repo with .git
        let repo_a = repos_dir.join("repo-a");
        fs::create_dir_all(repo_a.join(".git")).unwrap();

        // Dir without .git (not a repo)
        let not_repo = repos_dir.join("not-a-repo");
        fs::create_dir_all(&not_repo).unwrap();

        let repos = discover_repos_from_repos_dir(tmp.path());
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].name, "repo-a");
    }

    #[test]
    fn test_global_config_path_override() {
        // Serialize with other tests that read/write DEVORA_CONFIG_PATH - this test mutates that shared env var, so without the lock it can clobber a concurrent test's config path.
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let original = std::env::var("DEVORA_CONFIG_PATH").ok();
        std::env::set_var("DEVORA_CONFIG_PATH", "/tmp/test-config.json");
        let path = global_config_path().unwrap();
        assert_eq!(path, PathBuf::from("/tmp/test-config.json"));
        match original {
            Some(v) => std::env::set_var("DEVORA_CONFIG_PATH", v),
            None => std::env::remove_var("DEVORA_CONFIG_PATH"),
        }
    }

    #[test]
    fn test_get_git_shortcuts_enabled() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let original = std::env::var("DEVORA_CONFIG_PATH").ok();

        // Point the global config at a guaranteed-absent file so global resolution is deterministic (returns None -> default true).
        let tmp = tempfile::tempdir().unwrap();
        let global_config = tmp.path().join("global-config.json");
        std::env::set_var("DEVORA_CONFIG_PATH", &global_config);

        // No profile, no global config file -> default true.
        assert!(get_git_shortcuts_enabled(None));

        // Global config disables it (no profile override present).
        fs::write(&global_config, r#"{"terminal":{"git-shortcuts":false}}"#).unwrap();
        assert!(!get_git_shortcuts_enabled(None));

        // Profile override wins over global.
        let profile_dir = tmp.path().join("profile");
        fs::create_dir_all(&profile_dir).unwrap();
        fs::write(
            profile_dir.join("config.json"),
            r#"{"terminal":{"git-shortcuts":true}}"#,
        )
        .unwrap();
        let profile_path = profile_dir.to_string_lossy().to_string();
        assert!(get_git_shortcuts_enabled(Some(&profile_path)));

        // Profile with the key absent falls through to global (false here).
        fs::write(profile_dir.join("config.json"), r#"{"name":"p"}"#).unwrap();
        assert!(!get_git_shortcuts_enabled(Some(&profile_path)));

        match original {
            Some(v) => std::env::set_var("DEVORA_CONFIG_PATH", v),
            None => std::env::remove_var("DEVORA_CONFIG_PATH"),
        }
    }

    #[test]
    fn test_claude_effort_levels_are_canonical() {
        assert_eq!(CLAUDE_EFFORT_LEVELS, ["low", "medium", "high", "xhigh", "max"]);
    }

    #[test]
    fn test_claude_settings_resolution_and_write() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let original = std::env::var("DEVORA_CONFIG_PATH").ok();
        let tmp = tempfile::tempdir().unwrap();
        let global_config = tmp.path().join("global-config.json");
        std::env::set_var("DEVORA_CONFIG_PATH", &global_config);

        let opus = "ANTHROPIC_DEFAULT_OPUS_MODEL";
        let sonnet = "ANTHROPIC_DEFAULT_SONNET_MODEL";
        let haiku = "ANTHROPIC_DEFAULT_HAIKU_MODEL";
        let effort = "DEVORA_CCC_EFFORT";
        let get = |env: &HashMap<String, String>, k: &str| env.get(k).cloned();

        // 1. No config anywhere -> all Devora defaults are set.
        let env = claude_launch_env(None);
        assert_eq!(get(&env, opus).as_deref(), Some("claude-opus-4-8"));
        assert_eq!(get(&env, sonnet).as_deref(), Some("claude-opus-4-8"));
        assert_eq!(get(&env, haiku).as_deref(), Some("claude-sonnet-4-6"));
        assert_eq!(get(&env, effort).as_deref(), Some("xhigh"));

        // 2. User-level: a value wins; null = None (env var omitted); absent = default.
        fs::write(
            &global_config,
            r#"{"claude":{"opus-model":"claude-fable-5","haiku-model":null}}"#,
        )
        .unwrap();
        let env = claude_launch_env(None);
        assert_eq!(get(&env, opus).as_deref(), Some("claude-fable-5"));
        assert_eq!(get(&env, sonnet).as_deref(), Some("claude-opus-4-8"));
        assert!(!env.contains_key(haiku)); // null -> None -> omitted
        assert_eq!(get(&env, effort).as_deref(), Some("xhigh"));

        // 3. Profile overrides user per key; profile-absent keys fall through to user.
        let profile_dir = tmp.path().join("profile");
        fs::create_dir_all(&profile_dir).unwrap();
        let profile_path = profile_dir.to_string_lossy().to_string();
        fs::write(
            profile_dir.join("config.json"),
            r#"{"name":"p","claude":{"opus-model":null,"effort":"max"}}"#,
        )
        .unwrap();
        let env = claude_launch_env(Some(&profile_path));
        assert!(!env.contains_key(opus)); // profile null wins -> None
        assert_eq!(get(&env, effort).as_deref(), Some("max"));
        assert!(!env.contains_key(haiku)); // profile absent -> user null -> None
        assert_eq!(get(&env, sonnet).as_deref(), Some("claude-opus-4-8")); // both absent -> default

        // 4. Malformed values (wrong type, blank string) fall through to the default.
        fs::write(
            &global_config,
            r#"{"claude":{"opus-model":42,"effort":"   "}}"#,
        )
        .unwrap();
        let env = claude_launch_env(None);
        assert_eq!(get(&env, opus).as_deref(), Some("claude-opus-4-8"));
        assert_eq!(get(&env, effort).as_deref(), Some("xhigh"));

        // 5. read_claude_settings: `stored` distinguishes value / null / absent; `resolved` gives the effective value (string or null=None).
        fs::write(
            &global_config,
            r#"{"profiles":[],"claude":{"opus-model":"claude-fable-5","haiku-model":null}}"#,
        )
        .unwrap();
        let settings = read_claude_settings(None).unwrap();
        assert_eq!(
            settings.stored.get("opus-model"),
            Some(&Value::String("claude-fable-5".into()))
        );
        assert_eq!(settings.stored.get("haiku-model"), Some(&Value::Null));
        assert!(!settings.stored.contains_key("sonnet-model")); // absent
        assert_eq!(
            settings.resolved.get("opus-model"),
            Some(&Value::String("claude-fable-5".into()))
        );
        assert_eq!(
            settings.resolved.get("sonnet-model"),
            Some(&Value::String("claude-opus-4-8".into())) // default
        );
        assert_eq!(settings.resolved.get("haiku-model"), Some(&Value::Null)); // None

        // 6. write_claude_setting round-trips value/none/default and preserves siblings.
        write_claude_setting(None, "sonnet-model", "value", Some("claude-opus-4-7")).unwrap();
        let cfg = read_json_file(&global_config).unwrap();
        assert_eq!(cfg["claude"]["sonnet-model"], Value::String("claude-opus-4-7".into()));
        assert_eq!(cfg["claude"]["opus-model"], Value::String("claude-fable-5".into())); // sibling kept
        assert_eq!(cfg["profiles"], serde_json::json!([])); // unrelated key kept

        write_claude_setting(None, "opus-model", "none", None).unwrap();
        assert_eq!(read_json_file(&global_config).unwrap()["claude"]["opus-model"], Value::Null);

        write_claude_setting(None, "sonnet-model", "default", None).unwrap();
        assert!(read_json_file(&global_config).unwrap()["claude"]
            .get("sonnet-model")
            .is_none());

        // 7. Validation: blank value, bad effort, and unknown key are rejected.
        assert!(write_claude_setting(None, "opus-model", "value", Some("   ")).is_err());
        assert!(write_claude_setting(None, "effort", "value", Some("turbo")).is_err());
        assert!(write_claude_setting(None, "effort", "value", Some("max")).is_ok());
        assert!(write_claude_setting(None, "unknown-key", "value", Some("x")).is_err());

        match original {
            Some(v) => std::env::set_var("DEVORA_CONFIG_PATH", v),
            None => std::env::remove_var("DEVORA_CONFIG_PATH"),
        }
    }

    #[test]
    fn test_write_dot_path_set_remove_prune() {
        let mut cfg = serde_json::json!({ "name": "p" });

        // Set a deeply-nested leaf, creating intermediate objects; siblings untouched.
        write_dot_path(
            &mut cfg,
            "task-tracker.asana.project-id",
            Some(Value::String("99".into())),
        )
        .unwrap();
        assert_eq!(cfg["task-tracker"]["asana"]["project-id"], Value::String("99".into()));
        assert_eq!(cfg["name"], Value::String("p".into()));

        // A second leaf under the same branch + a top-level bool branch.
        write_dot_path(&mut cfg, "task-tracker.provider", Some(Value::String("asana".into()))).unwrap();
        write_dot_path(&mut cfg, "pr.auto-merge", Some(Value::Bool(false))).unwrap();
        assert_eq!(cfg["pr"]["auto-merge"], Value::Bool(false));

        // Removing the deep leaf prunes the now-empty `asana` object but keeps `task-tracker` (still has provider).
        write_dot_path(&mut cfg, "task-tracker.asana.project-id", None).unwrap();
        assert!(cfg["task-tracker"].get("asana").is_none());
        assert_eq!(cfg["task-tracker"]["provider"], Value::String("asana".into()));

        // Removing the last leaf prunes the whole branch.
        write_dot_path(&mut cfg, "task-tracker.provider", None).unwrap();
        assert!(cfg.get("task-tracker").is_none());

        // Removing an absent key is a no-op (no empty branch created).
        write_dot_path(&mut cfg, "feature.branch-prefix", None).unwrap();
        assert!(cfg.get("feature").is_none());
    }

    #[test]
    fn test_config_settings_resolution_and_write() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let original = std::env::var("DEVORA_CONFIG_PATH").ok();
        let tmp = tempfile::tempdir().unwrap();
        let global_config = tmp.path().join("global-config.json");
        std::env::set_var("DEVORA_CONFIG_PATH", &global_config);

        // 1. Nothing stored: resolved uses built-ins (git-shortcuts true), null for Debi-owned keys.
        let s = read_config_settings(None).unwrap();
        assert!(s.stored.is_empty());
        assert_eq!(s.resolved.get("terminal.git-shortcuts"), Some(&Value::Bool(true)));
        assert_eq!(s.resolved.get("terminal.default-app"), Some(&Value::Null));
        assert_eq!(s.resolved.get("pr.auto-merge"), Some(&Value::Null));
        assert_eq!(s.resolved.get("feature.branch-prefix"), Some(&Value::Null));

        // 2. Write at the user scope: a bool stays a JSON bool, a string stays a string.
        write_config_setting(None, "terminal.git-shortcuts", "value", Some("false")).unwrap();
        write_config_setting(None, "feature.branch-prefix", "value", Some("feat")).unwrap();
        write_config_setting(None, "terminal.default-app", "value", Some("nvim")).unwrap();
        let cfg = read_json_file(&global_config).unwrap();
        assert_eq!(cfg["terminal"]["git-shortcuts"], Value::Bool(false));
        assert_eq!(cfg["feature"]["branch-prefix"], Value::String("feat".into()));
        assert_eq!(cfg["terminal"]["default-app"], Value::String("nvim".into()));
        let s = read_config_settings(None).unwrap();
        assert_eq!(s.stored.get("terminal.git-shortcuts"), Some(&Value::Bool(false)));
        assert_eq!(s.resolved.get("terminal.git-shortcuts"), Some(&Value::Bool(false)));

        // 3. Profile overrides user per key; profile-absent keys fall through to user.
        let profile_dir = tmp.path().join("profile");
        fs::create_dir_all(&profile_dir).unwrap();
        let profile_path = profile_dir.to_string_lossy().to_string();
        fs::write(profile_dir.join("config.json"), r#"{"name":"p"}"#).unwrap();
        write_config_setting(Some(&profile_path), "feature.branch-prefix", "value", Some("hotfix")).unwrap();
        let s = read_config_settings(Some(&profile_path)).unwrap();
        assert_eq!(s.stored.get("feature.branch-prefix"), Some(&Value::String("hotfix".into())));
        assert_eq!(s.resolved.get("feature.branch-prefix"), Some(&Value::String("hotfix".into())));
        assert!(!s.stored.contains_key("terminal.git-shortcuts")); // absent at profile
        assert_eq!(s.resolved.get("terminal.git-shortcuts"), Some(&Value::Bool(false))); // from user
        // get_default_app routes through the same resolver: profile unset → user "nvim".
        assert_eq!(get_default_app(&profile_path).unwrap().as_deref(), Some("nvim"));

        // 4. Enum: empty value = explicit "none"; a valid value accepted; an invalid one rejected.
        write_config_setting(Some(&profile_path), "task-tracker.provider", "value", Some("asana")).unwrap();
        assert_eq!(
            read_json_file(&profile_dir.join("config.json")).unwrap()["task-tracker"]["provider"],
            Value::String("asana".into())
        );
        write_config_setting(Some(&profile_path), "task-tracker.provider", "value", Some("")).unwrap();
        assert_eq!(
            read_json_file(&profile_dir.join("config.json")).unwrap()["task-tracker"]["provider"],
            Value::String(String::new())
        );
        assert!(write_config_setting(Some(&profile_path), "task-tracker.provider", "value", Some("jira")).is_err());

        // 5. `default` removes the key and prunes empties; bad inputs are rejected.
        write_config_setting(Some(&profile_path), "task-tracker.provider", "default", None).unwrap();
        assert!(read_json_file(&profile_dir.join("config.json")).unwrap().get("task-tracker").is_none());
        assert!(write_config_setting(None, "feature.branch-prefix", "value", Some("   ")).is_err());
        assert!(write_config_setting(None, "unknown.key", "value", Some("x")).is_err());
        assert!(write_config_setting(None, "pr.auto-merge", "bogus", None).is_err());
        assert!(write_config_setting(None, "pr.auto-merge", "value", Some("yes")).is_err());

        // 6. Bool write round-trips and reads back typed.
        write_config_setting(None, "pr.auto-merge", "value", Some("true")).unwrap();
        assert_eq!(
            read_config_settings(None).unwrap().resolved.get("pr.auto-merge"),
            Some(&Value::Bool(true))
        );

        match original {
            Some(v) => std::env::set_var("DEVORA_CONFIG_PATH", v),
            None => std::env::remove_var("DEVORA_CONFIG_PATH"),
        }
    }

    #[test]
    fn test_get_registered_repos_merges_sources() {
        let tmp = tempfile::tempdir().unwrap();

        // Profile config with explicit repos
        let config = serde_json::json!({
            "name": "test-profile",
            "repos": ["/some/explicit/repo-x"]
        });
        fs::write(
            tmp.path().join("config.json"),
            serde_json::to_string(&config).unwrap(),
        )
        .unwrap();

        // Auto-discovered repo
        let repos_dir = tmp.path().join("repos");
        let repo_y = repos_dir.join("repo-y");
        fs::create_dir_all(repo_y.join(".git")).unwrap();

        let repos = get_registered_repos(&tmp.path().to_string_lossy()).unwrap();
        assert_eq!(repos.len(), 2);
        // Sorted alphabetically
        assert_eq!(repos[0].name, "repo-x");
        assert_eq!(repos[0].source, "registered");
        assert_eq!(repos[1].name, "repo-y");
        assert_eq!(repos[1].source, "auto-discovered");
    }

    #[test]
    fn test_save_profiling_report_creates_file() {
        let tmp = tempfile::tempdir().unwrap();
        let profile_path = tmp.path().to_string_lossy().to_string();
        let report = r#"{"timestamp":"2026-01-01T00:00:00Z","phases":{}}"#;

        let result = save_profiling_report(&profile_path, report).unwrap();

        assert!(result.contains("diagnostics/hub-profile-"));
        assert!(result.ends_with(".json"));

        let written = fs::read_to_string(&result).unwrap();
        assert_eq!(written, report);
    }

    #[test]
    fn test_save_profiling_report_creates_diagnostics_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let profile_path = tmp.path().to_string_lossy().to_string();
        let report = "{}";

        save_profiling_report(&profile_path, report).unwrap();

        assert!(tmp.path().join("diagnostics").exists());
    }

    #[test]
    fn test_parse_git_status_branch_porcelain_attached_branch() {
        let output = "## main\n";
        let result = parse_git_status_branch_porcelain(output);
        assert_eq!(result.branch, "main");
        assert!(!result.is_detached);
        assert_eq!(result.modified, 0);
        assert_eq!(result.untracked, 0);
    }

    #[test]
    fn test_parse_git_status_branch_porcelain_attached_with_tracking() {
        let output = "## main...origin/main\n";
        let result = parse_git_status_branch_porcelain(output);
        assert_eq!(result.branch, "main");
        assert!(!result.is_detached);
        assert_eq!(result.modified, 0);
        assert_eq!(result.untracked, 0);
    }

    #[test]
    fn test_parse_git_status_branch_porcelain_attached_with_ahead_behind() {
        let output = "## main...origin/main [ahead 3]\n";
        let result = parse_git_status_branch_porcelain(output);
        assert_eq!(result.branch, "main");
        assert!(!result.is_detached);
        assert_eq!(result.modified, 0);
        assert_eq!(result.untracked, 0);
    }

    #[test]
    fn test_parse_git_status_branch_porcelain_attached_with_behind() {
        let output = "## main...origin/main [behind 2]\n";
        let result = parse_git_status_branch_porcelain(output);
        assert_eq!(result.branch, "main");
        assert!(!result.is_detached);
    }

    #[test]
    fn test_parse_git_status_branch_porcelain_detached_head() {
        let output = "## HEAD (no branch)\n";
        let result = parse_git_status_branch_porcelain(output);
        assert_eq!(result.branch, "");
        assert!(result.is_detached);
        assert_eq!(result.modified, 0);
        assert_eq!(result.untracked, 0);
    }

    #[test]
    fn test_parse_git_status_branch_porcelain_with_modified_files() {
        let output = "## feature-x\n M file1.txt\nM  file2.txt\n";
        let result = parse_git_status_branch_porcelain(output);
        assert_eq!(result.branch, "feature-x");
        assert!(!result.is_detached);
        assert_eq!(result.modified, 2);
        assert_eq!(result.untracked, 0);
    }

    #[test]
    fn test_parse_git_status_branch_porcelain_with_untracked_files() {
        let output = "## main\n?? new_file.txt\n?? another.txt\n";
        let result = parse_git_status_branch_porcelain(output);
        assert_eq!(result.branch, "main");
        assert_eq!(result.modified, 0);
        assert_eq!(result.untracked, 2);
    }

    #[test]
    fn test_parse_git_status_branch_porcelain_with_modified_and_untracked() {
        let output = "## develop...origin/develop\nM  src/lib.rs\n?? temp.log\nA  new_file.rs\n";
        let result = parse_git_status_branch_porcelain(output);
        assert_eq!(result.branch, "develop");
        assert!(!result.is_detached);
        assert_eq!(result.modified, 2);
        assert_eq!(result.untracked, 1);
    }

    #[test]
    fn test_parse_git_status_branch_porcelain_empty_no_changes() {
        let output = "## main...origin/main\n";
        let result = parse_git_status_branch_porcelain(output);
        assert_eq!(result.branch, "main");
        assert_eq!(result.modified, 0);
        assert_eq!(result.untracked, 0);
    }

    #[test]
    fn test_parse_git_status_branch_porcelain_no_commits_yet() {
        let output = "## No commits yet on main\n?? newfile.txt\n";
        let result = parse_git_status_branch_porcelain(output);
        assert_eq!(result.branch, "main");
        assert!(!result.is_detached);
        assert_eq!(result.modified, 0);
        assert_eq!(result.untracked, 1);
    }

    #[test]
    fn test_get_workspace_status_uses_provided_repo_names() {
        let tmp = tempfile::tempdir().unwrap();
        let ws_path = tmp.path();

        // Create two git repos in the workspace directory
        for name in ["repo-a", "repo-b"] {
            let repo_dir = ws_path.join(name);
            fs::create_dir_all(&repo_dir).unwrap();
            let init = Command::new("git")
                .args(["init"])
                .current_dir(&repo_dir)
                .output()
                .unwrap();
            assert!(init.status.success(), "git init failed for {name}");
        }

        // Only pass repo-a; repo-b exists on disk but should be ignored
        let result = get_workspace_status(
            &ws_path.to_string_lossy(),
            vec!["repo-a".to_string()],
        )
        .unwrap();

        assert_eq!(result.statuses.len(), 1);
        assert_eq!(result.statuses[0].name, "repo-a");
    }

    /// Creates a git repo with one committed file, optionally on a detached HEAD.
    fn init_committed_repo(repo_dir: &Path, detach: bool) {
        fs::create_dir_all(repo_dir).unwrap();
        let run = |args: &[&str]| {
            let out = Command::new("git")
                .args(args)
                .current_dir(repo_dir)
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "git {args:?} failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        };
        run(&["init"]);
        fs::write(repo_dir.join("tracked.txt"), "content\n").unwrap();
        run(&["add", "tracked.txt"]);
        run(&[
            "-c",
            "user.name=test",
            "-c",
            "user.email=test@test",
            "commit",
            "-m",
            "init",
        ]);
        if detach {
            run(&["checkout", "--detach"]);
        }
    }

    const TEST_TASK_JSON: &str =
        r#"{"uid":"old-uid","title":"Old task","started_at":"2026-01-01"}"#;

    #[test]
    fn test_prepare_repurpose_task_returns_current_title() {
        let tmp = tempfile::tempdir().unwrap();
        init_committed_repo(&tmp.path().join("repo-a"), true);
        fs::write(tmp.path().join("task.json"), TEST_TASK_JSON).unwrap();

        let context = prepare_repurpose_task(&tmp.path().to_string_lossy()).unwrap();
        assert_eq!(context.current_title, "Old task");
    }

    #[test]
    fn test_prepare_repurpose_task_errors_without_task_json() {
        let tmp = tempfile::tempdir().unwrap();
        init_committed_repo(&tmp.path().join("repo-a"), true);

        let err = prepare_repurpose_task(&tmp.path().to_string_lossy()).unwrap_err();
        assert!(err.contains("no active task"), "error: {err}");
    }

    #[test]
    fn test_prepare_repurpose_task_blocks_on_untracked_file() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("repo-a");
        init_committed_repo(&repo, true);
        fs::write(repo.join("scratch.txt"), "wip\n").unwrap();
        fs::write(tmp.path().join("task.json"), TEST_TASK_JSON).unwrap();

        let err = prepare_repurpose_task(&tmp.path().to_string_lossy()).unwrap_err();
        assert!(err.contains("repo-a: 1 untracked"), "error: {err}");
        assert!(err.contains("not ready for a new task"), "error: {err}");
    }

    #[test]
    fn test_prepare_repurpose_task_blocks_on_modified_file() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("repo-a");
        init_committed_repo(&repo, true);
        fs::write(repo.join("tracked.txt"), "changed\n").unwrap();
        fs::write(tmp.path().join("task.json"), TEST_TASK_JSON).unwrap();

        let err = prepare_repurpose_task(&tmp.path().to_string_lossy()).unwrap_err();
        assert!(err.contains("repo-a: 1 modified"), "error: {err}");
    }

    #[test]
    fn test_prepare_repurpose_task_blocks_on_attached_branch() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("repo-a");
        init_committed_repo(&repo, false);
        let out = Command::new("git")
            .args(["checkout", "-b", "wip"])
            .current_dir(&repo)
            .output()
            .unwrap();
        assert!(out.status.success());
        fs::write(tmp.path().join("task.json"), TEST_TASK_JSON).unwrap();

        let err = prepare_repurpose_task(&tmp.path().to_string_lossy()).unwrap_err();
        assert!(
            err.contains("repo-a: on branch 'wip' (expected detached HEAD)"),
            "error: {err}"
        );
    }

    #[test]
    fn test_prepare_repurpose_task_aggregates_multiple_repos() {
        let tmp = tempfile::tempdir().unwrap();
        let repo_a = tmp.path().join("repo-a");
        init_committed_repo(&repo_a, true);
        fs::write(repo_a.join("scratch.txt"), "wip\n").unwrap();
        let repo_b = tmp.path().join("repo-b");
        init_committed_repo(&repo_b, false);
        let out = Command::new("git")
            .args(["checkout", "-b", "wip"])
            .current_dir(&repo_b)
            .output()
            .unwrap();
        assert!(out.status.success());
        fs::write(tmp.path().join("task.json"), TEST_TASK_JSON).unwrap();

        let err = prepare_repurpose_task(&tmp.path().to_string_lossy()).unwrap_err();
        assert!(err.contains("repo-a: 1 untracked"), "error: {err}");
        assert!(err.contains("repo-b: on branch 'wip'"), "error: {err}");
        assert!(err.contains("; "), "error: {err}");
    }

    #[test]
    fn test_repurpose_task_writes_new_task_identity() {
        let tmp = tempfile::tempdir().unwrap();
        init_committed_repo(&tmp.path().join("repo-a"), true);
        fs::write(tmp.path().join("task.json"), TEST_TASK_JSON).unwrap();

        repurpose_task(&tmp.path().to_string_lossy(), "Follow-up task").unwrap();

        let task = read_json_file(&tmp.path().join("task.json")).unwrap();
        assert_eq!(task.get("title").unwrap().as_str().unwrap(), "Follow-up task");

        let uid = task.get("uid").unwrap().as_str().unwrap();
        assert_ne!(uid, "old-uid");
        assert_eq!(uid.len(), 36, "uid should be a uuid: {uid}");

        let started_at = task.get("started_at").unwrap().as_str().unwrap();
        assert_eq!(started_at.len(), 10, "started_at: {started_at}");
        for (i, c) in started_at.chars().enumerate() {
            if i == 4 || i == 7 {
                assert_eq!(c, '-', "started_at: {started_at}");
            } else {
                assert!(c.is_ascii_digit(), "started_at: {started_at}");
            }
        }
    }

    #[test]
    fn test_task_json_written_with_four_space_indent() {
        let tmp = tempfile::tempdir().unwrap();

        write_task_json(tmp.path(), "Some task").unwrap();

        let raw = fs::read_to_string(tmp.path().join("task.json")).unwrap();
        assert!(raw.contains("\n    \"title\""), "expected 4-space indent, got: {raw}");
    }

    #[test]
    fn test_repurpose_task_revalidates_and_leaves_task_untouched() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("repo-a");
        init_committed_repo(&repo, true);
        fs::write(repo.join("scratch.txt"), "wip\n").unwrap();
        fs::write(tmp.path().join("task.json"), TEST_TASK_JSON).unwrap();

        let err = repurpose_task(&tmp.path().to_string_lossy(), "Follow-up task").unwrap_err();
        assert!(err.contains("repo-a: 1 untracked"), "error: {err}");

        let task_content = fs::read_to_string(tmp.path().join("task.json")).unwrap();
        assert_eq!(task_content, TEST_TASK_JSON);
    }
}

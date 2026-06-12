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
}

#[derive(Serialize)]
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

fn expand_tilde(path: &str) -> Result<String, String> {
    if let Some(rest) = path.strip_prefix("~/") {
        let home = home_dir()?;
        Ok(home.join(rest).to_string_lossy().into_owned())
    } else {
        Ok(path.to_string())
    }
}

fn global_config_path() -> Result<PathBuf, String> {
    if let Ok(override_path) = std::env::var("DEVORA_CONFIG_PATH") {
        return Ok(PathBuf::from(override_path));
    }
    let home = home_dir()?;
    Ok(home.join(".config/devora/config.json"))
}

fn read_json_file(path: &Path) -> Result<Value, String> {
    let content =
        fs::read_to_string(path).map_err(|e| format!("failed to read {}: {e}", path.display()))?;
    serde_json::from_str(&content)
        .map_err(|e| format!("failed to parse {}: {e}", path.display()))
}

fn get_value_by_dot_path<'a>(root: &'a Value, dot_path: &str) -> Option<&'a Value> {
    let mut current = root;
    for key in dot_path.split('.') {
        current = current.get(key)?;
    }
    Some(current)
}

fn list_repo_subdirs(dir: &Path) -> Vec<String> {
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

pub fn get_default_app(profile_path: &str) -> Result<Option<String>, String> {
    // Check profile-level config first
    let profile_config_path = Path::new(profile_path).join("config.json");
    if profile_config_path.exists() {
        let profile_config = read_json_file(&profile_config_path)?;
        if let Some(val) = get_value_by_dot_path(&profile_config, "terminal.default-app") {
            if let Some(s) = val.as_str() {
                return Ok(Some(s.to_string()));
            }
        }
    }

    // Fall back to global config
    match get_config_value("terminal.default-app")? {
        Some(val) => Ok(val.as_str().map(|s| s.to_string())),
        None => Ok(None),
    }
}

fn find_next_workspace_id(workspaces_dir: &Path) -> Result<String, String> {
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

fn mise_available() -> bool {
    Command::new("which")
        .arg("mise")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn run_mise_trust(worktree_path: &Path) -> Result<(), String> {
    let output = Command::new("mise")
        .args(["trust"])
        .current_dir(worktree_path)
        .output()
        .map_err(|e| format!("failed to spawn mise trust: {e}"))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(format!("mise trust exited with {}: {}", output.status, stderr.trim()))
}

fn run_prepare_command(prepare_cmd: &str, worktree_path: &Path) -> Result<(), String> {
    let output = Command::new("sh")
        .args(["-c", prepare_cmd])
        .current_dir(worktree_path)
        .output()
        .map_err(|e| format!("failed to spawn prepare-command: {e}"))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(format!("prepare-command exited with {}: {}", output.status, stderr.trim()))
}

pub fn create_workspace(
    profile_path: &str,
    repo_paths: Vec<String>,
    task_name: &str,
    warnings: &mut Vec<String>,
) -> Result<CreatedWorkspace, String> {
    let profile_dir = Path::new(profile_path);
    let workspaces_dir = profile_dir.join("workspaces");

    fs::create_dir_all(&workspaces_dir)
        .map_err(|e| format!("failed to create workspaces dir: {e}"))?;

    let ws_name = find_next_workspace_id(&workspaces_dir)?;
    let ws_path = workspaces_dir.join(&ws_name);

    fs::create_dir_all(&ws_path).map_err(|e| format!("failed to create {}: {e}", ws_name))?;

    // Acquire creation lock
    let lock_path = workspaces_dir.join(".creation-lock");
    let lock_file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|e| format!("failed to open creation lock: {e}"))?;

    use fs2::FileExt;
    lock_file
        .lock_exclusive()
        .map_err(|e| format!("failed to acquire creation lock: {e}"))?;

    let result = (|| -> Result<(), String> {
        // Determine default branch per repo and create worktrees
        for repo_path_str in &repo_paths {
            let expanded = expand_tilde(repo_path_str)?;
            let source_repo = Path::new(&expanded);
            let repo_name = source_repo
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .ok_or_else(|| format!("invalid repo path: {expanded}"))?;

            // Determine the default branch
            let branch_output = Command::new("git")
                .args(["symbolic-ref", "refs/remotes/origin/HEAD", "--short"])
                .current_dir(source_repo)
                .output()
                .map_err(|e| format!("failed to determine default branch for {repo_name}: {e}"))?;

            let default_branch = if branch_output.status.success() {
                String::from_utf8_lossy(&branch_output.stdout)
                    .trim()
                    .to_string()
            } else {
                // Fallback: try origin/main then origin/master
                "origin/main".to_string()
            };

            // Fetch the branch
            let fetch_branch = default_branch.strip_prefix("origin/").unwrap_or(&default_branch);
            let fetch_result = Command::new("git")
                .args(["fetch", "origin", fetch_branch])
                .current_dir(source_repo)
                .output()
                .map_err(|e| format!("failed to fetch for {repo_name}: {e}"))?;

            if !fetch_result.status.success() {
                let stderr = String::from_utf8_lossy(&fetch_result.stderr);
                return Err(format!("git fetch failed for {repo_name}: {stderr}"));
            }

            // Create worktree
            let worktree_path = ws_path.join(&repo_name);
            let worktree_result = Command::new("git")
                .args([
                    "worktree",
                    "add",
                    "--detach",
                    &worktree_path.to_string_lossy(),
                    &default_branch,
                ])
                .current_dir(source_repo)
                .output()
                .map_err(|e| format!("failed to create worktree for {repo_name}: {e}"))?;

            if !worktree_result.status.success() {
                let stderr = String::from_utf8_lossy(&worktree_result.stderr);
                return Err(format!(
                    "git worktree add failed for {repo_name}: {stderr}"
                ));
            }

            // Run mise trust if available — non-fatal, but the failure is reported
            if mise_available() {
                if let Err(e) = run_mise_trust(&worktree_path) {
                    warnings.push(format!("{repo_name}: {e}"));
                }
            }
        }

        // Run prepare-command if configured
        let global_config_path = global_config_path()?;
        if global_config_path.exists() {
            if let Ok(config) = read_json_file(&global_config_path) {
                if let Some(prepare_cmd) = config.get("prepare-command").and_then(|v| v.as_str()) {
                    for repo_path_str in &repo_paths {
                        let expanded = expand_tilde(repo_path_str)?;
                        let repo_name = Path::new(&expanded)
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        let worktree_path = ws_path.join(&repo_name);
                        if worktree_path.exists() {
                            // Non-fatal, but the failure is reported
                            if let Err(e) = run_prepare_command(prepare_cmd, &worktree_path) {
                                warnings.push(format!("{repo_name}: {e}"));
                            }
                        }
                    }
                }
            }
        }

        // Write initialized marker
        fs::write(ws_path.join("initialized"), "")
            .map_err(|e| format!("failed to write initialized marker: {e}"))?;

        // Write task.json
        write_task_json(&ws_path, task_name)?;

        // Write CLAUDE.md template if more than one repo
        if repo_paths.len() > 1 {
            let repo_names: Vec<String> = repo_paths
                .iter()
                .filter_map(|p| {
                    Path::new(p)
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                })
                .collect();
            let repos_list = repo_names
                .iter()
                .map(|n| format!("- `{n}/`"))
                .collect::<Vec<_>>()
                .join("\n");
            let claude_md = format!(
                "## The Workspace\n\nThis workspace contains the following repositories:\n\n{repos_list}\n"
            );
            fs::write(ws_path.join("CLAUDE.md"), claude_md)
                .map_err(|e| format!("failed to write CLAUDE.md: {e}"))?;
        }

        Ok(())
    })();

    // Release lock (happens automatically on drop, but be explicit)
    let _ = lock_file.unlock();

    result?;

    Ok(CreatedWorkspace {
        path: ws_path.to_string_lossy().into_owned(),
        name: ws_name,
    })
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

fn write_task_json(ws_path: &Path, title: &str) -> Result<(), String> {
    let task = serde_json::json!({
        "uid": Uuid::new_v4().to_string(),
        "title": title,
        "started_at": date_format("%Y-%m-%d"),
    });
    let task_json = serde_json::to_string_pretty(&task)
        .map_err(|e| format!("failed to serialize task.json: {e}"))?;
    fs::write(ws_path.join("task.json"), task_json)
        .map_err(|e| format!("failed to write task.json: {e}"))
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
    fn test_run_prepare_command_success() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(run_prepare_command("true", tmp.path()).is_ok());
    }

    #[test]
    fn test_run_prepare_command_failure_returns_stderr() {
        let tmp = tempfile::tempdir().unwrap();
        let err = run_prepare_command("echo boom >&2; exit 1", tmp.path()).unwrap_err();
        assert!(err.contains("boom"), "error: {err}");
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
        assert_eq!(repos[1].name, "repo-y");
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

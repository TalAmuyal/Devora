use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;

use serde::Serialize;
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

pub fn list_workspaces(profile_path: &str) -> Result<Vec<WorkspaceInfo>, String> {
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
            match read_json_file(&task_path) {
                Ok(task) => task
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                Err(_) => String::new(),
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

pub fn get_workspace_status(workspace_path: &str) -> Result<Vec<RepoStatus>, String> {
    let ws_dir = Path::new(workspace_path);
    if !ws_dir.exists() {
        return Err(format!("workspace path does not exist: {workspace_path}"));
    }

    let repo_names = list_repo_subdirs(ws_dir);

    let handles: Vec<_> = repo_names
        .into_iter()
        .map(|name| {
            let repo_path = ws_dir.join(&name);
            thread::spawn(move || get_single_repo_status(&name, &repo_path))
        })
        .collect();

    let mut statuses = Vec::new();
    for handle in handles {
        match handle.join() {
            Ok(Ok(status)) => statuses.push(status),
            Ok(Err(e)) => return Err(e),
            Err(_) => return Err("thread panicked while getting repo status".to_string()),
        }
    }

    statuses.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(statuses)
}

fn get_single_repo_status(name: &str, repo_path: &Path) -> Result<RepoStatus, String> {
    let branch_output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("failed to run git rev-parse in {name}: {e}"))?;

    let branch_raw = String::from_utf8_lossy(&branch_output.stdout)
        .trim()
        .to_string();

    let is_detached = branch_raw == "HEAD";

    let branch = if is_detached {
        let hash_output = Command::new("git")
            .args(["rev-parse", "--short", "HEAD"])
            .current_dir(repo_path)
            .output()
            .map_err(|e| format!("failed to get commit hash in {name}: {e}"))?;
        String::from_utf8_lossy(&hash_output.stdout)
            .trim()
            .to_string()
    } else {
        branch_raw
    };

    let status_output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("failed to run git status in {name}: {e}"))?;

    let status_text = String::from_utf8_lossy(&status_output.stdout);
    let mut modified = 0usize;
    let mut untracked = 0usize;
    for line in status_text.lines() {
        if line.starts_with("??") {
            untracked += 1;
        } else if !line.is_empty() {
            modified += 1;
        }
    }

    Ok(RepoStatus {
        name: name.to_string(),
        branch,
        is_detached,
        modified,
        untracked,
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

pub fn create_workspace(
    profile_path: &str,
    repo_paths: Vec<String>,
    task_name: &str,
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

            // Run mise trust if available
            if mise_available() {
                let _ = Command::new("mise")
                    .args(["trust"])
                    .current_dir(&worktree_path)
                    .output();
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
                            let _ = Command::new("sh")
                                .args(["-c", prepare_cmd])
                                .current_dir(&worktree_path)
                                .output();
                        }
                    }
                }
            }
        }

        // Write initialized marker
        fs::write(ws_path.join("initialized"), "")
            .map_err(|e| format!("failed to write initialized marker: {e}"))?;

        // Write task.json
        let task = serde_json::json!({
            "uid": Uuid::new_v4().to_string(),
            "title": task_name,
            "started_at": chrono_free_today(),
        });
        let task_json = serde_json::to_string_pretty(&task)
            .map_err(|e| format!("failed to serialize task.json: {e}"))?;
        fs::write(ws_path.join("task.json"), task_json)
            .map_err(|e| format!("failed to write task.json: {e}"))?;

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

/// Returns today's date as YYYY-MM-DD without pulling in the chrono crate.
fn chrono_free_today() -> String {
    let output = Command::new("date")
        .args(["+%Y-%m-%d"])
        .output()
        .expect("failed to run date command");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
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
        let result = list_workspaces(&tmp.path().to_string_lossy()).unwrap();
        assert!(result.is_empty());
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

        let result = list_workspaces(&tmp.path().to_string_lossy()).unwrap();
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
}

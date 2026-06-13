//! Non-blocking, cancellable, progress-streaming task creation.
//!
//! `create_workspace` (the Tauri command in `commands.rs`) registers a [`CreationHandle`] with the [`WorkspaceCreationManager`] and spawns a worker thread that runs [`run`].
//! The worker reuses an inactive workspace when one matches (refreshing it to the latest default branch and re-running the prepare-command), or builds a fresh one otherwise.
//! Every phase is reported through a `Channel<CreationEvent>` (high-level steps plus streamed subprocess output), and the work can be cancelled mid-flight — even during a long prepare-command — via `cancel_workspace_creation`.

use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use serde::Serialize;
use tauri::ipc::Channel;
use tauri::Manager;

use crate::workspace::{self, CreatedWorkspace};

/// How often the wait loop polls a running child for completion / cancellation.
const POLL_MS: u64 = 50;

/// A progress event streamed to the frontend over the creation channel.
#[derive(Serialize, Clone)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum CreationEvent {
    /// A new high-level phase has started (shown as a step in the progress overlay).
    Step { label: String },
    /// A line of subprocess output (shown in the expandable live log).
    Log { line: String },
    /// Creation finished successfully; carries the created/reused workspace.
    Done { workspace: CreatedWorkspace },
    /// Creation failed; `message` is also surfaced as an error banner.
    Failed { message: String },
    /// Creation was cancelled by the user; the partial work has been cleaned up.
    Cancelled,
}

/// Shared cancellation state for one in-flight creation.
/// Cheap to clone (just `Arc`s): the manager, the worker thread, and the wait loop all hold the same flag and current-child slot.
#[derive(Clone)]
pub struct CreationHandle {
    cancel: Arc<AtomicBool>,
    current_child: Arc<Mutex<Option<Child>>>,
}

impl CreationHandle {
    pub fn new() -> Self {
        Self {
            cancel: Arc::new(AtomicBool::new(false)),
            current_child: Arc::new(Mutex::new(None)),
        }
    }

    /// Request cancellation: set the flag and kill the currently-running subprocess (if any) so a long `git fetch` / prepare-command is interrupted promptly rather than after it finishes.
    pub fn request_cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
        if let Ok(mut slot) = self.current_child.lock() {
            if let Some(child) = slot.as_mut() {
                let _ = child.kill();
            }
        }
    }

    fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }
}

/// Registry of in-flight creations, managed as Tauri state so `cancel_workspace_creation` can find a creation by id.
/// Mirrors the `PtyManager` pattern.
pub struct WorkspaceCreationManager {
    handles: HashMap<u32, CreationHandle>,
    next_id: u32,
}

impl WorkspaceCreationManager {
    pub fn new() -> Self {
        Self {
            handles: HashMap::new(),
            next_id: 1,
        }
    }

    /// Allocate an id and a handle, retaining a clone so the creation can be cancelled later.
    pub fn register(&mut self) -> (u32, CreationHandle) {
        let id = self.next_id;
        self.next_id += 1;
        let handle = CreationHandle::new();
        self.handles.insert(id, handle.clone());
        (id, handle)
    }

    fn remove(&mut self, id: u32) {
        self.handles.remove(&id);
    }

    /// Signal cancellation for the given creation. Unknown ids are ignored (the creation already
    /// finished and deregistered).
    pub fn cancel(&mut self, id: u32) {
        if let Some(handle) = self.handles.get(&id) {
            handle.request_cancel();
        }
    }
}

enum Mode {
    Fresh,
    Reuse,
}

enum Outcome {
    Done(CreatedWorkspace),
    Cancelled,
}

/// Worker-thread entry point: run the creation, deregister, report warnings, and emit the terminal event.
/// Spawned by the `create_workspace` command.
pub fn run(
    app: tauri::AppHandle,
    id: u32,
    handle: CreationHandle,
    profile_path: String,
    repo_paths: Vec<String>,
    task_name: String,
    on_event: Channel<CreationEvent>,
) {
    let mut warnings = Vec::new();
    let result = run_inner(
        &handle,
        &profile_path,
        &repo_paths,
        &task_name,
        &on_event,
        &mut warnings,
    );

    if let Some(state) = app.try_state::<Mutex<WorkspaceCreationManager>>() {
        if let Ok(mut manager) = state.lock() {
            manager.remove(id);
        }
    }

    for warning in &warnings {
        crate::logging::report_error(&app, warning);
    }

    match result {
        Ok(Outcome::Done(workspace)) => {
            let _ = on_event.send(CreationEvent::Done { workspace });
        }
        Ok(Outcome::Cancelled) => {
            let _ = on_event.send(CreationEvent::Cancelled);
        }
        Err(message) => {
            crate::logging::report_error(&app, &message);
            let _ = on_event.send(CreationEvent::Failed { message });
        }
    }
}

fn run_inner(
    handle: &CreationHandle,
    profile_path: &str,
    repo_paths: &[String],
    task_name: &str,
    on_event: &Channel<CreationEvent>,
    warnings: &mut Vec<String>,
) -> Result<Outcome, String> {
    let workspaces_dir = Path::new(profile_path).join("workspaces");
    fs::create_dir_all(&workspaces_dir)
        .map_err(|e| format!("failed to create workspaces dir: {e}"))?;

    // Resolve each repo to (source path, worktree name).
    let mut repos: Vec<(String, String)> = Vec::new();
    for repo_path in repo_paths {
        let expanded = workspace::expand_tilde(repo_path)?;
        let name = Path::new(&expanded)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .ok_or_else(|| format!("invalid repo path: {expanded}"))?;
        repos.push((expanded, name));
    }
    let repo_names: Vec<String> = repos.iter().map(|(_, name)| name.clone()).collect();

    let _ = on_event.send(CreationEvent::Step {
        label: "Resolving workspace".to_string(),
    });

    let (ws_path, ws_name, mode) = select_workspace(&workspaces_dir, &repo_names, task_name)?;

    let body = run_body(handle, &repos, &ws_path, &mode, on_event, warnings);

    match body {
        Ok(()) if handle.is_cancelled() => {
            cleanup_aborted(&ws_path, &repo_names, &mode);
            Ok(Outcome::Cancelled)
        }
        Ok(()) => {
            if matches!(mode, Mode::Fresh) {
                finalize_fresh(&ws_path, &repos, task_name)?;
            }
            Ok(Outcome::Done(CreatedWorkspace {
                path: ws_path.to_string_lossy().into_owned(),
                name: ws_name,
            }))
        }
        Err(message) => {
            cleanup_aborted(&ws_path, &repo_names, &mode);
            if handle.is_cancelled() {
                Ok(Outcome::Cancelled)
            } else {
                Err(message)
            }
        }
    }
}

/// Critical section (under the workspaces creation lock): pick a reusable inactive workspace and claim it by writing `task.json`, or allocate a fresh `ws-N` directory.
/// The lock is held only here (~ms) so concurrent creations cannot select the same workspace, then released for the slow git/prepare work.
fn select_workspace(
    workspaces_dir: &Path,
    repo_names: &[String],
    task_name: &str,
) -> Result<(PathBuf, String, Mode), String> {
    use fs2::FileExt;

    let lock_path = workspaces_dir.join(".creation-lock");
    let lock_file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|e| format!("failed to open creation lock: {e}"))?;
    lock_file
        .lock_exclusive()
        .map_err(|e| format!("failed to acquire creation lock: {e}"))?;

    let result = (|| {
        if let Some(reused) = find_reusable_workspace(workspaces_dir, repo_names) {
            // Claim immediately so a concurrent creation's search skips this now-active workspace.
            workspace::write_task_json(&reused, task_name)?;
            let name = reused
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            Ok((reused, name, Mode::Reuse))
        } else {
            let name = workspace::find_next_workspace_id(workspaces_dir)?;
            let path = workspaces_dir.join(&name);
            fs::create_dir_all(&path).map_err(|e| format!("failed to create {name}: {e}"))?;
            Ok((path, name, Mode::Fresh))
        }
    })();

    let _ = lock_file.unlock();
    result
}

/// First reusable workspace: `initialized`, no `task.json` (inactive), and whose worktree set exactly matches the requested repos.
/// Picks the lowest `ws-N` for determinism.
pub(crate) fn find_reusable_workspace(workspaces_dir: &Path, repo_names: &[String]) -> Option<PathBuf> {
    let mut wanted: Vec<String> = repo_names.to_vec();
    wanted.sort();

    let mut candidates: Vec<PathBuf> = Vec::new();
    for entry in fs::read_dir(workspaces_dir).ok()?.flatten() {
        let dir_name = entry.file_name().to_string_lossy().into_owned();
        if !dir_name.starts_with("ws-") {
            continue;
        }
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if !path.join("initialized").exists() {
            continue;
        }
        if path.join("task.json").exists() {
            continue;
        }
        let mut got = workspace::list_repo_subdirs(&path);
        got.sort();
        if got == wanted {
            candidates.push(path);
        }
    }

    candidates.sort_by_key(|p| workspace_id_number(p).unwrap_or(usize::MAX));
    candidates.into_iter().next()
}

fn workspace_id_number(ws_path: &Path) -> Option<usize> {
    ws_path
        .file_name()
        .and_then(|n| n.to_str())
        .and_then(|n| n.strip_prefix("ws-"))
        .and_then(|n| n.parse::<usize>().ok())
}

fn run_body(
    handle: &CreationHandle,
    repos: &[(String, String)],
    ws_path: &Path,
    mode: &Mode,
    on_event: &Channel<CreationEvent>,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    match mode {
        Mode::Fresh => {
            for (source, name) in repos {
                if handle.is_cancelled() {
                    return Ok(());
                }
                let source_repo = Path::new(source);
                let worktree_path = ws_path.join(name);
                let default_branch = determine_default_branch(source_repo);
                let fetch_branch = default_branch
                    .strip_prefix("origin/")
                    .unwrap_or(&default_branch)
                    .to_string();

                let _ = on_event.send(CreationEvent::Step {
                    label: format!("Fetching {name}"),
                });
                let ok = run_streamed(
                    "git",
                    &["fetch", "origin", &fetch_branch],
                    source_repo,
                    on_event,
                    handle,
                )?;
                if !ok {
                    if handle.is_cancelled() {
                        return Ok(());
                    }
                    return Err(format!("git fetch failed for {name} (see log)"));
                }

                let _ = on_event.send(CreationEvent::Step {
                    label: format!("Creating worktree {name}"),
                });
                let worktree_str = worktree_path.to_string_lossy().to_string();
                let ok = run_streamed(
                    "git",
                    &["worktree", "add", "--detach", &worktree_str, &default_branch],
                    source_repo,
                    on_event,
                    handle,
                )?;
                if !ok {
                    if handle.is_cancelled() {
                        return Ok(());
                    }
                    return Err(format!("git worktree add failed for {name} (see log)"));
                }

                if workspace::mise_available() {
                    let _ = on_event.send(CreationEvent::Step {
                        label: format!("Trusting mise for {name}"),
                    });
                    let ok = run_streamed("mise", &["trust"], &worktree_path, on_event, handle)?;
                    if !ok && !handle.is_cancelled() {
                        warnings.push(format!("{name}: mise trust failed (see log)"));
                    }
                    if handle.is_cancelled() {
                        return Ok(());
                    }
                }
            }
        }
        Mode::Reuse => {
            for (_source, name) in repos {
                if handle.is_cancelled() {
                    return Ok(());
                }
                let worktree_path = ws_path.join(name);
                let _ = on_event.send(CreationEvent::Step {
                    label: format!("Refreshing {name}"),
                });
                refresh_worktree(&worktree_path, on_event, handle)
                    .map_err(|e| format!("{name}: {e}"))?;
                if handle.is_cancelled() {
                    return Ok(());
                }
            }
        }
    }

    run_prepare(handle, repos, ws_path, on_event, warnings)
}

/// Re-run the configured prepare-command in every worktree (both fresh and reused).
/// Failures are non-fatal (reported as warnings) — matching the original behaviour — but the dependency cache is still in place, so a warm install is fast.
fn run_prepare(
    handle: &CreationHandle,
    repos: &[(String, String)],
    ws_path: &Path,
    on_event: &Channel<CreationEvent>,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    let Some(prepare_cmd) = get_prepare_command() else {
        return Ok(());
    };

    for (_source, name) in repos {
        if handle.is_cancelled() {
            return Ok(());
        }
        let worktree_path = ws_path.join(name);
        if !worktree_path.exists() {
            continue;
        }
        let _ = on_event.send(CreationEvent::Step {
            label: format!("Preparing {name}"),
        });
        let ok = run_streamed("sh", &["-c", &prepare_cmd], &worktree_path, on_event, handle)?;
        if !ok && !handle.is_cancelled() {
            warnings.push(format!("{name}: prepare-command failed (see log)"));
        }
    }
    Ok(())
}

/// Refresh a reused worktree to the latest default branch.
/// An inactive workspace's worktrees are already clean and detached, so a plain `fetch` + detached `checkout origin/<default>` suffices — no `reset`/`clean`, which keeps the untracked dependency cache (`node_modules`, `.venv`, …).
pub(crate) fn refresh_worktree(
    worktree: &Path,
    on_event: &Channel<CreationEvent>,
    handle: &CreationHandle,
) -> Result<(), String> {
    let default_branch = determine_default_branch(worktree);
    let fetch_branch = default_branch
        .strip_prefix("origin/")
        .unwrap_or(&default_branch)
        .to_string();

    let ok = run_streamed("git", &["fetch", "origin", &fetch_branch], worktree, on_event, handle)?;
    if !ok {
        if handle.is_cancelled() {
            return Ok(());
        }
        return Err("git fetch failed (see log)".to_string());
    }

    let ok = run_streamed("git", &["checkout", &default_branch], worktree, on_event, handle)?;
    if !ok {
        if handle.is_cancelled() {
            return Ok(());
        }
        return Err("git checkout failed (see log)".to_string());
    }

    Ok(())
}

fn determine_default_branch(repo_dir: &Path) -> String {
    let output = Command::new("git")
        .args(["symbolic-ref", "refs/remotes/origin/HEAD", "--short"])
        .current_dir(repo_dir)
        .output();
    match output {
        Ok(out) if out.status.success() => {
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        }
        _ => "origin/main".to_string(),
    }
}

fn get_prepare_command() -> Option<String> {
    let path = workspace::global_config_path().ok()?;
    if !path.exists() {
        return None;
    }
    let config = workspace::read_json_file(&path).ok()?;
    config
        .get("prepare-command")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn finalize_fresh(ws_path: &Path, repos: &[(String, String)], task_name: &str) -> Result<(), String> {
    fs::write(ws_path.join("initialized"), "")
        .map_err(|e| format!("failed to write initialized marker: {e}"))?;
    workspace::write_task_json(ws_path, task_name)?;

    if repos.len() > 1 {
        let repos_list = repos
            .iter()
            .map(|(_, name)| format!("- `{name}/`"))
            .collect::<Vec<_>>()
            .join("\n");
        let claude_md = format!(
            "## The Workspace\n\nThis workspace contains the following repositories:\n\n{repos_list}\n"
        );
        fs::write(ws_path.join("CLAUDE.md"), claude_md)
            .map_err(|e| format!("failed to write CLAUDE.md: {e}"))?;
    }
    Ok(())
}

/// Undo a creation that was cancelled or failed.
/// A fresh workspace is removed entirely (worktrees deregistered, then directory deleted); a reused workspace reverts to inactive by dropping the claim (`task.json`) while keeping its cached worktrees.
fn cleanup_aborted(ws_path: &Path, repo_names: &[String], mode: &Mode) {
    match mode {
        Mode::Fresh => {
            for name in repo_names {
                let worktree = ws_path.join(name);
                if worktree.join(".git").exists() {
                    let _ = Command::new("git")
                        .args(["worktree", "remove", "--force", "."])
                        .current_dir(&worktree)
                        .output();
                }
            }
            let _ = fs::remove_dir_all(ws_path);
        }
        Mode::Reuse => {
            let _ = fs::remove_file(ws_path.join("task.json"));
        }
    }
}

/// Run a subprocess, streaming each stdout/stderr line as a `Log` event, while keeping the child killable for cancellation.
/// Returns `Ok(success)`; `Err` only when the process cannot be spawned.
fn run_streamed(
    program: &str,
    args: &[&str],
    cwd: &Path,
    on_event: &Channel<CreationEvent>,
    handle: &CreationHandle,
) -> Result<bool, String> {
    let mut child = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to spawn {program}: {e}"))?;

    // Take the pipes (owned) before handing the child to the shared slot.
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    {
        let mut slot = handle.current_child.lock().unwrap();
        *slot = Some(child);
    }

    let mut readers: Vec<JoinHandle<()>> = Vec::new();
    if let Some(out) = stdout {
        readers.push(spawn_reader(Box::new(out), on_event.clone()));
    }
    if let Some(err) = stderr {
        readers.push(spawn_reader(Box::new(err), on_event.clone()));
    }

    let success = loop {
        {
            let mut slot = handle.current_child.lock().unwrap();
            match slot.as_mut() {
                Some(child) => {
                    if handle.is_cancelled() {
                        let _ = child.kill();
                    }
                    match child.try_wait() {
                        Ok(Some(status)) => {
                            let success = status.success();
                            *slot = None;
                            break success;
                        }
                        Ok(None) => {}
                        Err(e) => {
                            *slot = None;
                            return Err(format!("failed to wait for {program}: {e}"));
                        }
                    }
                }
                None => break false,
            }
        }
        thread::sleep(Duration::from_millis(POLL_MS));
    };

    for reader in readers {
        let _ = reader.join();
    }

    Ok(success)
}

fn spawn_reader(reader: Box<dyn Read + Send>, on_event: Channel<CreationEvent>) -> JoinHandle<()> {
    thread::spawn(move || {
        let buffered = BufReader::new(reader);
        for line in buffered.lines() {
            match line {
                Ok(line) => {
                    let _ = on_event.send(CreationEvent::Log { line });
                }
                Err(_) => break,
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::time::Instant;

    fn noop_channel() -> Channel<CreationEvent> {
        Channel::new(|_| Ok(()))
    }

    fn collecting_channel() -> (Channel<CreationEvent>, Arc<Mutex<Vec<serde_json::Value>>>) {
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink = events.clone();
        let channel = Channel::new(move |body| {
            let parsed = match body {
                tauri::ipc::InvokeResponseBody::Json(s) => {
                    serde_json::from_str::<serde_json::Value>(&s).ok()
                }
                tauri::ipc::InvokeResponseBody::Raw(bytes) => {
                    serde_json::from_slice::<serde_json::Value>(&bytes).ok()
                }
            };
            if let Some(value) = parsed {
                sink.lock().unwrap().push(value);
            }
            Ok(())
        });
        (channel, events)
    }

    fn make_ws(dir: &Path, id: &str, initialized: bool, active: bool, repos: &[&str]) -> PathBuf {
        let ws = dir.join(id);
        fs::create_dir_all(&ws).unwrap();
        if initialized {
            fs::write(ws.join("initialized"), "").unwrap();
        }
        if active {
            fs::write(
                ws.join("task.json"),
                r#"{"uid":"x","title":"t","started_at":"2026-01-01"}"#,
            )
            .unwrap();
        }
        for repo in repos {
            fs::create_dir_all(ws.join(repo)).unwrap();
        }
        ws
    }

    fn git(dir: &Path, args: &[&str]) -> std::process::Output {
        let out = Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        out
    }

    #[test]
    fn find_reusable_matches_inactive_same_repo_set() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        make_ws(dir, "ws-1", true, true, &["repo-a"]); // active — skipped
        let ws2 = make_ws(dir, "ws-2", true, false, &["repo-a"]); // inactive match
        assert_eq!(
            find_reusable_workspace(dir, &["repo-a".to_string()]),
            Some(ws2)
        );
    }

    #[test]
    fn find_reusable_skips_active_only() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        make_ws(dir, "ws-1", true, true, &["repo-a"]);
        assert_eq!(find_reusable_workspace(dir, &["repo-a".to_string()]), None);
    }

    #[test]
    fn find_reusable_skips_mismatched_repos() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        make_ws(dir, "ws-1", true, false, &["repo-a", "repo-b"]);
        assert_eq!(find_reusable_workspace(dir, &["repo-a".to_string()]), None);
    }

    #[test]
    fn find_reusable_skips_uninitialized() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        make_ws(dir, "ws-1", false, false, &["repo-a"]);
        assert_eq!(find_reusable_workspace(dir, &["repo-a".to_string()]), None);
    }

    #[test]
    fn find_reusable_picks_lowest_id() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        let ws2 = make_ws(dir, "ws-2", true, false, &["repo-a"]);
        make_ws(dir, "ws-10", true, false, &["repo-a"]);
        assert_eq!(
            find_reusable_workspace(dir, &["repo-a".to_string()]),
            Some(ws2)
        );
    }

    #[test]
    fn refresh_worktree_advances_and_preserves_untracked_cache() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        let bare = root.join("origin.git");
        fs::create_dir_all(&bare).unwrap();
        git(&bare, &["init", "--bare", "-b", "main"]);

        let source = root.join("source");
        git(root, &["clone", bare.to_str().unwrap(), source.to_str().unwrap()]);
        git(&source, &["config", "user.email", "t@t"]);
        git(&source, &["config", "user.name", "t"]);
        fs::write(source.join("file.txt"), "v1\n").unwrap();
        git(&source, &["add", "."]);
        git(&source, &["commit", "-m", "c1"]);
        git(&source, &["push", "origin", "main"]);

        let wt = root.join("wt");
        git(&source, &["worktree", "add", "--detach", wt.to_str().unwrap(), "origin/main"]);
        let head_before = String::from_utf8_lossy(&git(&wt, &["rev-parse", "HEAD"]).stdout)
            .trim()
            .to_string();

        // Stand-in for a cached dependency dir: untracked, must survive the refresh.
        fs::write(wt.join("node_modules_marker"), "cache").unwrap();

        // Someone advanced the default branch upstream.
        fs::write(source.join("file.txt"), "v2\n").unwrap();
        git(&source, &["add", "."]);
        git(&source, &["commit", "-m", "c2"]);
        git(&source, &["push", "origin", "main"]);
        let head_upstream = String::from_utf8_lossy(&git(&source, &["rev-parse", "HEAD"]).stdout)
            .trim()
            .to_string();

        refresh_worktree(&wt, &noop_channel(), &CreationHandle::new()).unwrap();

        let head_after = String::from_utf8_lossy(&git(&wt, &["rev-parse", "HEAD"]).stdout)
            .trim()
            .to_string();
        assert_ne!(head_after, head_before, "worktree should advance");
        assert_eq!(head_after, head_upstream, "worktree should be at latest origin");
        assert!(
            wt.join("node_modules_marker").exists(),
            "untracked cache must survive the refresh"
        );
        assert_eq!(fs::read_to_string(wt.join("file.txt")).unwrap(), "v2\n");
    }

    #[test]
    fn run_streamed_reports_success_and_failure() {
        let tmp = tempfile::tempdir().unwrap();
        let handle = CreationHandle::new();
        let channel = noop_channel();
        assert!(run_streamed("sh", &["-c", "exit 0"], tmp.path(), &channel, &handle).unwrap());
        assert!(!run_streamed("sh", &["-c", "exit 1"], tmp.path(), &channel, &handle).unwrap());
    }

    #[test]
    fn run_streamed_emits_log_events() {
        let tmp = tempfile::tempdir().unwrap();
        let (channel, events) = collecting_channel();
        let ok = run_streamed(
            "sh",
            &["-c", "echo hello-stream"],
            tmp.path(),
            &channel,
            &CreationHandle::new(),
        )
        .unwrap();
        assert!(ok);
        let events = events.lock().unwrap();
        let found = events.iter().any(|v| {
            v.get("type").and_then(|t| t.as_str()) == Some("log")
                && v.get("line").and_then(|l| l.as_str()) == Some("hello-stream")
        });
        assert!(found, "expected a log event with 'hello-stream', got {events:?}");
    }

    #[test]
    fn run_streamed_cancellation_kills_child_promptly() {
        let tmp = tempfile::tempdir().unwrap();
        let handle = CreationHandle::new();
        let channel = noop_channel();

        let canceller_handle = handle.clone();
        let canceller = thread::spawn(move || {
            thread::sleep(Duration::from_millis(200));
            canceller_handle.request_cancel();
        });

        let start = Instant::now();
        let success = run_streamed("sleep", &["5"], tmp.path(), &channel, &handle).unwrap();
        let elapsed = start.elapsed();

        canceller.join().unwrap();
        assert!(!success, "a cancelled command should report failure");
        assert!(
            elapsed < Duration::from_secs(3),
            "cancellation should be prompt, took {elapsed:?}"
        );
    }
}

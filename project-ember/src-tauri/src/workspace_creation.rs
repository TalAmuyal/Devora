//! Non-blocking, cancellable, progress-streaming task creation.
//!
//! `create_workspace` (the Tauri command in `commands.rs`) registers a [`CreationHandle`] with the [`WorkspaceCreationManager`] and spawns a worker thread that runs [`run`].
//! The worker reuses an inactive workspace when one matches (refreshing it to the latest default branch and re-running the prepare-command), or builds a fresh one otherwise.
//! Every phase is reported through a `Channel<CreationEvent>` (high-level steps plus streamed subprocess output), and the work can be cancelled mid-flight — even during a long prepare-command — via `cancel_workspace_creation`.

use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::path::{Component, Path, PathBuf};
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

#[derive(Debug)]
enum Outcome {
    Done(CreatedWorkspace),
    Cancelled,
}

/// One repo to materialise as a worktree, with its checkout target resolved up front so the worktree is created/checked-out exactly once (and the prepare-command runs on the intended commit).
struct RepoSpec {
    /// The registered repo the worktree is created from.
    source: String,
    /// The worktree directory name (the repo basename).
    name: String,
    /// The ref to check out: a pinned commit SHA (duplication) or `origin/<default>` (latest).
    target_ref: String,
    /// Whether to `git fetch origin` before checking out. True only for the latest case — a
    /// pinned SHA is already present in the shared object store.
    fetch_first: bool,
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
    source_workspace_path: Option<String>,
    on_event: Channel<CreationEvent>,
) {
    let mut warnings = Vec::new();
    let result = run_inner(
        &handle,
        &profile_path,
        &repo_paths,
        &task_name,
        source_workspace_path.as_deref(),
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
    source_workspace_path: Option<&str>,
    on_event: &Channel<CreationEvent>,
    warnings: &mut Vec<String>,
) -> Result<Outcome, String> {
    let workspaces_dir = Path::new(profile_path).join("workspaces");
    fs::create_dir_all(&workspaces_dir)
        .map_err(|e| format!("failed to create workspaces dir: {e}"))?;

    // Resolve each repo to a worktree spec with its checkout target decided up front: the source workspace's current commit when duplicating a repo it already has, otherwise the latest default branch.
    let mut repos: Vec<RepoSpec> = Vec::new();
    for repo_path in repo_paths {
        let expanded = workspace::expand_tilde(repo_path)?;
        let name = Path::new(&expanded)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .ok_or_else(|| format!("invalid repo path: {expanded}"))?;
        let pinned = source_workspace_path
            .and_then(|src| resolve_pinned_commit(Path::new(src), &name, Path::new(&expanded)));
        let (target_ref, fetch_first) = match pinned {
            Some(sha) => (sha, false),
            None => (determine_default_branch(Path::new(&expanded)), true),
        };
        repos.push(RepoSpec {
            source: expanded,
            name,
            target_ref,
            fetch_first,
        });
    }
    let repo_names: Vec<String> = repos.iter().map(|r| r.name.clone()).collect();

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
                finalize_fresh(&ws_path, task_name)?;
            }
            // Duplication overwrites the new workspace's CLAUDE.md with the source's (for both fresh and reused workspaces), so it carries the source's workspace-level guidance.
            if let Some(src) = source_workspace_path {
                copy_source_claude_md(Path::new(src), &ws_path)?;
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

/// Worker-thread entry point for adding a single repo as a worktree to an existing workspace.
/// Spawned by the `add_repo_to_workspace` command.
/// Mirrors [`run`]: do the work, deregister, report warnings, then emit the terminal event.
/// On cancel (or a failure observed after cancellation) the partially-added worktree is rolled back so the workspace returns to its prior state.
pub fn run_add_repo(
    app: tauri::AppHandle,
    id: u32,
    handle: CreationHandle,
    workspace_path: String,
    source_repo_path: String,
    worktree_dir_name: String,
    on_event: Channel<CreationEvent>,
) {
    let mut warnings = Vec::new();
    let result = run_add_repo_inner(
        &handle,
        &workspace_path,
        &source_repo_path,
        &worktree_dir_name,
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

    let target = Path::new(&workspace_path).join(&worktree_dir_name);
    match result {
        Ok(Outcome::Done(workspace)) => {
            let _ = on_event.send(CreationEvent::Done { workspace });
        }
        Ok(Outcome::Cancelled) => {
            cleanup_added_worktree(&target);
            let _ = on_event.send(CreationEvent::Cancelled);
        }
        Err(message) => {
            if handle.is_cancelled() {
                cleanup_added_worktree(&target);
                let _ = on_event.send(CreationEvent::Cancelled);
            } else {
                crate::logging::report_error(&app, &message);
                let _ = on_event.send(CreationEvent::Failed { message });
            }
        }
    }
}

fn run_add_repo_inner(
    handle: &CreationHandle,
    workspace_path: &str,
    source_repo_path: &str,
    worktree_dir_name: &str,
    on_event: &Channel<CreationEvent>,
    warnings: &mut Vec<String>,
) -> Result<Outcome, String> {
    // Guard against a name that would escape the workspace (e.g. a postfix of "../x").
    let mut components = Path::new(worktree_dir_name).components();
    let single_normal = matches!(components.next(), Some(Component::Normal(_)))
        && components.next().is_none();
    if worktree_dir_name.is_empty() || !single_normal {
        return Err(format!("invalid worktree name: {worktree_dir_name}"));
    }

    let ws_path = Path::new(workspace_path);
    let target = ws_path.join(worktree_dir_name);
    if target.exists() {
        return Err(format!(
            "Directory '{worktree_dir_name}' already exists. Use a different postfix."
        ));
    }

    let source = workspace::expand_tilde(source_repo_path)?;
    let default_branch = determine_default_branch(Path::new(&source));

    if !create_worktree_for_repo(
        Path::new(&source),
        worktree_dir_name,
        &target,
        &default_branch,
        true,
        on_event,
        handle,
        warnings,
    )? {
        return Ok(Outcome::Cancelled);
    }

    run_prepare(
        handle,
        &[RepoSpec {
            source,
            name: worktree_dir_name.to_string(),
            target_ref: default_branch,
            fetch_first: true,
        }],
        ws_path,
        on_event,
        warnings,
    )?;
    if handle.is_cancelled() {
        return Ok(Outcome::Cancelled);
    }

    ensure_workspace_claude_md(ws_path)?;

    let name = ws_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    Ok(Outcome::Done(CreatedWorkspace {
        path: workspace_path.to_string(),
        name,
    }))
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
    repos: &[RepoSpec],
    ws_path: &Path,
    mode: &Mode,
    on_event: &Channel<CreationEvent>,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    match mode {
        Mode::Fresh => {
            for spec in repos {
                if handle.is_cancelled() {
                    return Ok(());
                }
                let worktree_path = ws_path.join(&spec.name);
                if !create_worktree_for_repo(
                    Path::new(&spec.source),
                    &spec.name,
                    &worktree_path,
                    &spec.target_ref,
                    spec.fetch_first,
                    on_event,
                    handle,
                    warnings,
                )? {
                    return Ok(());
                }
            }
        }
        Mode::Reuse => {
            for spec in repos {
                if handle.is_cancelled() {
                    return Ok(());
                }
                let worktree_path = ws_path.join(&spec.name);
                let _ = on_event.send(CreationEvent::Step {
                    label: format!("Refreshing {}", spec.name),
                });
                refresh_worktree(&worktree_path, &spec.target_ref, spec.fetch_first, on_event, handle)
                    .map_err(|e| format!("{}: {e}", spec.name))?;
                if handle.is_cancelled() {
                    return Ok(());
                }
            }
        }
    }

    run_prepare(handle, repos, ws_path, on_event, warnings)
}

/// Create one worktree from `source_repo`, detached at `target_ref`, then trust mise.
/// `fetch_first` controls whether `origin` is fetched before the single checkout — needed only for the latest case (a pinned commit is already in the shared object store).
/// `label` drives the streamed step labels.
/// Returns `Ok(true)` on completion, `Ok(false)` if cancelled mid-flight, `Err` on hard failure.
fn create_worktree_for_repo(
    source_repo: &Path,
    label: &str,
    worktree_path: &Path,
    target_ref: &str,
    fetch_first: bool,
    on_event: &Channel<CreationEvent>,
    handle: &CreationHandle,
    warnings: &mut Vec<String>,
) -> Result<bool, String> {
    if fetch_first {
        let fetch_branch = target_ref
            .strip_prefix("origin/")
            .unwrap_or(target_ref)
            .to_string();

        let _ = on_event.send(CreationEvent::Step {
            label: format!("Fetching {label}"),
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
                return Ok(false);
            }
            return Err(format!("git fetch failed for {label} (see log)"));
        }
    }

    let _ = on_event.send(CreationEvent::Step {
        label: format!("Creating worktree {label}"),
    });
    let worktree_str = worktree_path.to_string_lossy().to_string();
    let ok = run_streamed(
        "git",
        &["worktree", "add", "--detach", &worktree_str, target_ref],
        source_repo,
        on_event,
        handle,
    )?;
    if !ok {
        if handle.is_cancelled() {
            return Ok(false);
        }
        return Err(format!("git worktree add failed for {label} (see log)"));
    }

    if workspace::mise_available() {
        let _ = on_event.send(CreationEvent::Step {
            label: format!("Trusting mise for {label}"),
        });
        let ok = run_streamed("mise", &["trust"], worktree_path, on_event, handle)?;
        if !ok && !handle.is_cancelled() {
            warnings.push(format!("{label}: mise trust failed (see log)"));
        }
        if handle.is_cancelled() {
            return Ok(false);
        }
    }

    Ok(true)
}

/// Re-run the configured prepare-command in every worktree (both fresh and reused).
/// Failures are non-fatal (reported as warnings) — matching the original behaviour — but the dependency cache is still in place, so a warm install is fast.
fn run_prepare(
    handle: &CreationHandle,
    repos: &[RepoSpec],
    ws_path: &Path,
    on_event: &Channel<CreationEvent>,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    let Some(prepare_cmd) = get_prepare_command() else {
        return Ok(());
    };

    for spec in repos {
        if handle.is_cancelled() {
            return Ok(());
        }
        let name = &spec.name;
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

/// Move a reused worktree to `target_ref` (detached), fetching `origin` first only for the latest case (`fetch_first`).
/// An inactive workspace's worktrees are already clean and detached, so a plain (optional) `fetch` + detached `checkout` suffices — no `reset`/`clean`, which keeps the untracked dependency cache (`node_modules`, `.venv`, …).
pub(crate) fn refresh_worktree(
    worktree: &Path,
    target_ref: &str,
    fetch_first: bool,
    on_event: &Channel<CreationEvent>,
    handle: &CreationHandle,
) -> Result<(), String> {
    if fetch_first {
        let fetch_branch = target_ref
            .strip_prefix("origin/")
            .unwrap_or(target_ref)
            .to_string();

        let ok = run_streamed("git", &["fetch", "origin", &fetch_branch], worktree, on_event, handle)?;
        if !ok {
            if handle.is_cancelled() {
                return Ok(());
            }
            return Err("git fetch failed (see log)".to_string());
        }
    }

    let ok = run_streamed("git", &["checkout", target_ref], worktree, on_event, handle)?;
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

/// Resolve the commit a duplicated worktree should be pinned to: the HEAD of the source workspace's worktree named `name`, but only when that commit is reachable in `selected_repo` (the registered repo the new worktree is created from).
///
/// Returns `None` — so the caller falls back to the latest default branch — when the source has no such worktree, its HEAD can't be read, or the commit isn't present in `selected_repo` (e.g. a basename collision between two registered repos, or a postfix-renamed worktree).
/// The reachability check also guarantees the later `git worktree add` won't hard-fail.
fn resolve_pinned_commit(source_ws: &Path, name: &str, selected_repo: &Path) -> Option<String> {
    let worktree = source_ws.join(name);
    if !worktree.exists() {
        return None;
    }

    let head = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&worktree)
        .output()
        .ok()?;
    if !head.status.success() {
        return None;
    }
    let sha = String::from_utf8_lossy(&head.stdout).trim().to_string();
    if sha.is_empty() {
        return None;
    }

    let reachable = Command::new("git")
        .args([
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{sha}^{{commit}}"),
        ])
        .current_dir(selected_repo)
        .output()
        .ok()?;
    reachable.status.success().then_some(sha)
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

fn finalize_fresh(ws_path: &Path, task_name: &str) -> Result<(), String> {
    fs::write(ws_path.join("initialized"), "")
        .map_err(|e| format!("failed to write initialized marker: {e}"))?;
    workspace::write_task_json(ws_path, task_name)?;
    ensure_workspace_claude_md(ws_path)
}

/// Write a workspace-level `CLAUDE.md` listing the workspace's repos, but only when the workspace has more than one repo and no `CLAUDE.md` already exists — so a hand-edited file is never clobbered.
/// Idempotent: safe to call after every worktree add as well as at fresh creation.
fn ensure_workspace_claude_md(ws_path: &Path) -> Result<(), String> {
    let claude_path = ws_path.join("CLAUDE.md");
    if claude_path.exists() {
        return Ok(());
    }
    let repos = workspace::list_repo_subdirs(ws_path);
    if repos.len() <= 1 {
        return Ok(());
    }
    let repos_list = repos
        .iter()
        .map(|name| format!("- `{name}/`"))
        .collect::<Vec<_>>()
        .join("\n");
    let claude_md = format!(
        "## The Workspace\n\nThis workspace contains the following repositories:\n\n{repos_list}\n"
    );
    fs::write(&claude_path, claude_md).map_err(|e| format!("failed to write CLAUDE.md: {e}"))
}

/// Copy the source workspace's `CLAUDE.md` over the destination workspace's, overwriting it.
/// A no-op when the source workspace has no `CLAUDE.md` (the destination keeps whatever `ensure_workspace_claude_md` produced).
fn copy_source_claude_md(src_ws: &Path, dst_ws: &Path) -> Result<(), String> {
    let src = src_ws.join("CLAUDE.md");
    if !src.exists() {
        return Ok(());
    }
    fs::copy(&src, dst_ws.join("CLAUDE.md"))
        .map(|_| ())
        .map_err(|e| format!("failed to copy CLAUDE.md: {e}"))
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

/// Roll back a single added worktree (used when an add-repo is cancelled): deregister it from its source repo, then remove the directory.
/// The surrounding workspace pre-exists and is left intact.
fn cleanup_added_worktree(target: &Path) {
    if target.join(".git").exists() {
        let _ = Command::new("git")
            .args(["worktree", "remove", "--force", "."])
            .current_dir(target)
            .output();
    }
    let _ = fs::remove_dir_all(target);
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

        refresh_worktree(&wt, "origin/main", true, &noop_channel(), &CreationHandle::new()).unwrap();

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

    /// Build a bare `origin.git` plus a `source` clone with one pushed commit on `main`.
    /// Returns the `source` repo path (its `origin/main` points at the pushed commit).
    fn make_origin_and_source(root: &Path) -> PathBuf {
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
        source
    }

    #[test]
    fn create_worktree_for_repo_creates_detached_worktree_at_origin_default() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let source = make_origin_and_source(root);
        let origin_head =
            String::from_utf8_lossy(&git(&source, &["rev-parse", "origin/main"]).stdout)
                .trim()
                .to_string();

        let ws = root.join("ws-1");
        fs::create_dir_all(&ws).unwrap();
        let target = ws.join("source");

        let completed = create_worktree_for_repo(
            &source,
            "source",
            &target,
            "origin/main",
            true,
            &noop_channel(),
            &CreationHandle::new(),
            &mut Vec::new(),
        )
        .unwrap();

        assert!(completed);
        assert!(target.join(".git").exists(), "worktree should be created");
        let head = String::from_utf8_lossy(&git(&target, &["rev-parse", "HEAD"]).stdout)
            .trim()
            .to_string();
        assert_eq!(head, origin_head, "worktree should be at origin's default branch head");
        let symbolic = Command::new("git")
            .args(["symbolic-ref", "-q", "HEAD"])
            .current_dir(&target)
            .output()
            .unwrap();
        assert!(!symbolic.status.success(), "worktree should be in detached HEAD");
    }

    #[test]
    fn cleanup_added_worktree_removes_and_deregisters() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let source = make_origin_and_source(root);

        let ws = root.join("ws-1");
        fs::create_dir_all(&ws).unwrap();
        let target = ws.join("source");
        create_worktree_for_repo(
            &source,
            "source",
            &target,
            "origin/main",
            true,
            &noop_channel(),
            &CreationHandle::new(),
            &mut Vec::new(),
        )
        .unwrap();
        assert!(target.exists());

        cleanup_added_worktree(&target);

        assert!(!target.exists(), "added worktree dir should be removed");
        let list = String::from_utf8_lossy(&git(&source, &["worktree", "list"]).stdout).to_string();
        assert!(
            !list.contains(target.to_str().unwrap()),
            "worktree should be deregistered from the source repo: {list}"
        );
    }

    #[test]
    fn ensure_workspace_claude_md_writes_for_multi_repo_when_absent() {
        let tmp = tempfile::tempdir().unwrap();
        let ws = tmp.path();
        fs::create_dir_all(ws.join("repo-a")).unwrap();
        fs::create_dir_all(ws.join("repo-b")).unwrap();

        ensure_workspace_claude_md(ws).unwrap();

        let content = fs::read_to_string(ws.join("CLAUDE.md")).unwrap();
        assert!(content.contains("- `repo-a/`"), "got: {content}");
        assert!(content.contains("- `repo-b/`"), "got: {content}");
    }

    #[test]
    fn ensure_workspace_claude_md_does_not_overwrite_existing() {
        let tmp = tempfile::tempdir().unwrap();
        let ws = tmp.path();
        fs::create_dir_all(ws.join("repo-a")).unwrap();
        fs::create_dir_all(ws.join("repo-b")).unwrap();
        fs::write(ws.join("CLAUDE.md"), "hand-edited\n").unwrap();

        ensure_workspace_claude_md(ws).unwrap();

        assert_eq!(fs::read_to_string(ws.join("CLAUDE.md")).unwrap(), "hand-edited\n");
    }

    #[test]
    fn ensure_workspace_claude_md_skips_single_repo() {
        let tmp = tempfile::tempdir().unwrap();
        let ws = tmp.path();
        fs::create_dir_all(ws.join("repo-a")).unwrap();

        ensure_workspace_claude_md(ws).unwrap();

        assert!(!ws.join("CLAUDE.md").exists());
    }

    #[test]
    fn add_repo_inner_rejects_existing_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let ws = tmp.path();
        fs::create_dir_all(ws.join("repo")).unwrap();

        let err = run_add_repo_inner(
            &CreationHandle::new(),
            ws.to_str().unwrap(),
            "/does/not/matter",
            "repo",
            &noop_channel(),
            &mut Vec::new(),
        )
        .unwrap_err();
        assert_eq!(err, "Directory 'repo' already exists. Use a different postfix.");
    }

    #[test]
    fn add_repo_inner_rejects_escaping_name() {
        let tmp = tempfile::tempdir().unwrap();
        let ws = tmp.path();

        for bad in ["../escape", "a/b", "", ".", ".."] {
            let err = run_add_repo_inner(
                &CreationHandle::new(),
                ws.to_str().unwrap(),
                "/x",
                bad,
                &noop_channel(),
                &mut Vec::new(),
            )
            .unwrap_err();
            assert!(err.contains("invalid worktree name"), "for {bad:?} got: {err}");
        }
    }

    #[test]
    fn add_repo_inner_adds_worktree_and_writes_claude_md_for_second_repo() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let source = make_origin_and_source(root);

        let ws = root.join("ws-1");
        fs::create_dir_all(ws.join("existing")).unwrap(); // a pre-existing repo dir

        let outcome = run_add_repo_inner(
            &CreationHandle::new(),
            ws.to_str().unwrap(),
            source.to_str().unwrap(),
            "source",
            &noop_channel(),
            &mut Vec::new(),
        )
        .unwrap();

        assert!(matches!(outcome, Outcome::Done(_)));
        assert!(ws.join("source").join(".git").exists(), "worktree should be created");
        let content = fs::read_to_string(ws.join("CLAUDE.md")).unwrap();
        assert!(content.contains("- `source/`"), "got: {content}");
        assert!(content.contains("- `existing/`"), "got: {content}");
    }

    /// Build a bare origin + a `source` clone named `name` with two pushed commits on `main`.
    /// Returns `(source_path, first_commit, second_commit)`; `origin/main` ends at the second.
    /// The file content is namespaced by `name` so two repos never share a commit SHA — git hashes are otherwise identical when same-content commits land in the same second.
    fn make_repo_with_two_commits(root: &Path, name: &str) -> (PathBuf, String, String) {
        let bare = root.join(format!("{name}.git"));
        fs::create_dir_all(&bare).unwrap();
        git(&bare, &["init", "--bare", "-b", "main"]);

        let source = root.join(name);
        git(root, &["clone", bare.to_str().unwrap(), source.to_str().unwrap()]);
        git(&source, &["config", "user.email", "t@t"]);
        git(&source, &["config", "user.name", "t"]);

        fs::write(source.join("file.txt"), format!("{name} v1\n")).unwrap();
        git(&source, &["add", "."]);
        git(&source, &["commit", "-m", "c1"]);
        git(&source, &["push", "origin", "main"]);
        let c1 = head_of(&source);

        fs::write(source.join("file.txt"), format!("{name} v2\n")).unwrap();
        git(&source, &["add", "."]);
        git(&source, &["commit", "-m", "c2"]);
        git(&source, &["push", "origin", "main"]);
        let c2 = head_of(&source);

        (source, c1, c2)
    }

    /// Add a detached worktree of `repo` at `commit` under `<ws>/<name>` — mirrors a source session's worktree on disk.
    fn add_source_worktree(repo: &Path, ws: &Path, name: &str, commit: &str) {
        fs::create_dir_all(ws).unwrap();
        git(
            repo,
            &["worktree", "add", "--detach", ws.join(name).to_str().unwrap(), commit],
        );
    }

    fn head_of(dir: &Path) -> String {
        String::from_utf8_lossy(&git(dir, &["rev-parse", "HEAD"]).stdout)
            .trim()
            .to_string()
    }

    fn is_detached(dir: &Path) -> bool {
        !Command::new("git")
            .args(["symbolic-ref", "-q", "HEAD"])
            .current_dir(dir)
            .output()
            .unwrap()
            .status
            .success()
    }

    #[test]
    fn resolve_pinned_commit_returns_source_head_when_reachable() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let (repo, c1, _c2) = make_repo_with_two_commits(root, "repo");
        let ws = root.join("source-ws");
        add_source_worktree(&repo, &ws, "repo", &c1);

        assert_eq!(resolve_pinned_commit(&ws, "repo", &repo), Some(c1));
    }

    #[test]
    fn resolve_pinned_commit_none_when_commit_not_in_selected_repo() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let (repo_a, a1, _a2) = make_repo_with_two_commits(root, "a");
        let (repo_b, _b1, _b2) = make_repo_with_two_commits(root, "b");
        let ws = root.join("source-ws");
        // The source worktree is named "b" but holds repo_a's commit; resolving it against repo_b (the basename match) must reject it — the commit isn't reachable there.
        add_source_worktree(&repo_a, &ws, "b", &a1);

        assert_eq!(resolve_pinned_commit(&ws, "b", &repo_b), None);
    }

    #[test]
    fn resolve_pinned_commit_none_when_subdir_absent() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let (repo, _c1, _c2) = make_repo_with_two_commits(root, "repo");
        let ws = root.join("source-ws");
        fs::create_dir_all(&ws).unwrap();

        assert_eq!(resolve_pinned_commit(&ws, "repo", &repo), None);
    }

    #[test]
    fn create_worktree_for_repo_pins_to_given_commit_without_fetch() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let (source, c1, c2) = make_repo_with_two_commits(root, "repo");
        assert_ne!(c1, c2);

        let target = root.join("ws-1").join("repo");
        let completed = create_worktree_for_repo(
            &source,
            "repo",
            &target,
            &c1,
            false,
            &noop_channel(),
            &CreationHandle::new(),
            &mut Vec::new(),
        )
        .unwrap();

        assert!(completed);
        assert_eq!(head_of(&target), c1, "worktree should be pinned to the requested commit");
        assert!(is_detached(&target), "worktree should be detached");
    }

    #[test]
    fn refresh_worktree_pins_reused_worktree_to_given_commit() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let (source, c1, c2) = make_repo_with_two_commits(root, "repo");

        // A reused worktree starts at the latest tip (c2); pinning moves it back to c1.
        let wt = root.join("wt");
        git(&source, &["worktree", "add", "--detach", wt.to_str().unwrap(), &c2]);
        assert_eq!(head_of(&wt), c2);

        refresh_worktree(&wt, &c1, false, &noop_channel(), &CreationHandle::new()).unwrap();

        assert_eq!(head_of(&wt), c1, "reused worktree should be pinned to the requested commit");
        assert!(is_detached(&wt));
    }

    #[test]
    fn copy_source_claude_md_overwrites_destination() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let src = root.join("src");
        let dst = root.join("dst");
        fs::create_dir_all(&src).unwrap();
        fs::create_dir_all(&dst).unwrap();
        fs::write(src.join("CLAUDE.md"), "SOURCE\n").unwrap();
        fs::write(dst.join("CLAUDE.md"), "DEST\n").unwrap();

        copy_source_claude_md(&src, &dst).unwrap();

        assert_eq!(fs::read_to_string(dst.join("CLAUDE.md")).unwrap(), "SOURCE\n");
    }

    #[test]
    fn copy_source_claude_md_noop_when_source_absent() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let src = root.join("src");
        let dst = root.join("dst");
        fs::create_dir_all(&src).unwrap();
        fs::create_dir_all(&dst).unwrap();
        fs::write(dst.join("CLAUDE.md"), "DEST\n").unwrap();

        copy_source_claude_md(&src, &dst).unwrap();

        assert_eq!(fs::read_to_string(dst.join("CLAUDE.md")).unwrap(), "DEST\n");
    }

    #[test]
    fn run_inner_duplicate_pins_source_repo_and_copies_claude_md() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        // Pinned repo: origin/main is at c2, but the source workspace's worktree sits on c1.
        let (repo_pinned, c1, c2) = make_repo_with_two_commits(root, "pinned");
        assert_ne!(c1, c2);
        // Added repo (not in the source workspace): should land at the latest tip.
        let (repo_added, _a1, a_latest) = make_repo_with_two_commits(root, "added");

        let source_ws = root.join("source-ws");
        add_source_worktree(&repo_pinned, &source_ws, "pinned", &c1);
        fs::write(source_ws.join("CLAUDE.md"), "SOURCE CLAUDE\n").unwrap();

        let profile = root.join("profile");
        fs::create_dir_all(&profile).unwrap();

        let repo_paths = vec![
            repo_pinned.to_string_lossy().into_owned(),
            repo_added.to_string_lossy().into_owned(),
        ];

        let outcome = run_inner(
            &CreationHandle::new(),
            profile.to_str().unwrap(),
            &repo_paths,
            "Dup task",
            Some(source_ws.to_str().unwrap()),
            &noop_channel(),
            &mut Vec::new(),
        )
        .unwrap();

        let ws_path = match outcome {
            Outcome::Done(ws) => PathBuf::from(ws.path),
            other => panic!("expected Done, got {other:?}"),
        };

        let pinned_wt = ws_path.join("pinned");
        assert_eq!(head_of(&pinned_wt), c1, "source repo should be pinned to the source commit");
        assert!(is_detached(&pinned_wt));

        let added_wt = ws_path.join("added");
        assert_eq!(head_of(&added_wt), a_latest, "added repo should be at the latest tip");
        assert!(is_detached(&added_wt));

        assert_eq!(
            fs::read_to_string(ws_path.join("CLAUDE.md")).unwrap(),
            "SOURCE CLAUDE\n",
            "the source workspace CLAUDE.md should overwrite the generated one"
        );
    }
}

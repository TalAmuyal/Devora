use std::collections::HashMap;
use std::sync::Mutex;

use tauri::ipc::Channel;
use tauri::State;

use crate::ipc_server::IpcState;
use crate::logging::LogState;
use crate::profile;
use crate::pty::PtyManager;
use crate::test_harness::TestHarnessState;
use crate::workspace;
use crate::workspace_creation::{self, WorkspaceCreationManager};

#[tauri::command]
pub fn create_pty(
    state: State<'_, Mutex<PtyManager>>,
    ipc_state: State<'_, IpcState>,
    cwd: Option<String>,
    shell: Option<String>,
    env: Option<HashMap<String, String>>,
    app_command: Option<String>,
    profile_path: Option<String>,
    cols: u16,
    rows: u16,
    on_output: Channel<Vec<u8>>,
    on_exit: Channel<u32>,
) -> Result<u32, String> {
    let ipc_port = ipc_state.port;
    let git_shortcuts_enabled = workspace::get_git_shortcuts_enabled(profile_path.as_deref());

    // Inject the resolved Claude model/effort env vars (profile → user → Devora default).
    // An explicit frontend-supplied `env` still wins on key collision.
    let mut session_env = workspace::claude_launch_env(profile_path.as_deref());
    if let Some(extra) = env {
        session_env.extend(extra);
    }

    let mut manager = state
        .lock()
        .map_err(|e| format!("failed to lock pty manager: {e}"))?;

    manager.create_session(
        cwd.as_deref(),
        shell.as_deref(),
        Some(&session_env),
        app_command.as_deref(),
        git_shortcuts_enabled,
        ipc_port,
        cols,
        rows,
        on_output,
        on_exit,
    )
}

#[tauri::command]
pub fn write_pty(
    state: State<'_, Mutex<PtyManager>>,
    id: u32,
    data: Vec<u8>,
) -> Result<(), String> {
    let mut manager = state
        .lock()
        .map_err(|e| format!("failed to lock pty manager: {e}"))?;

    manager.write(id, &data)
}

#[tauri::command]
pub fn resize_pty(
    state: State<'_, Mutex<PtyManager>>,
    id: u32,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    let mut manager = state
        .lock()
        .map_err(|e| format!("failed to lock pty manager: {e}"))?;

    manager.resize(id, cols, rows)
}

#[tauri::command]
pub fn close_pty(state: State<'_, Mutex<PtyManager>>, id: u32) -> Result<(), String> {
    let mut manager = state
        .lock()
        .map_err(|e| format!("failed to lock pty manager: {e}"))?;

    manager.close(id)
}

#[tauri::command]
pub fn list_profiles() -> Result<Vec<workspace::ProfileInfo>, String> {
    workspace::list_profiles()
}

#[tauri::command]
pub fn register_profile(
    path: String,
    name: String,
) -> Result<profile::RegisteredProfile, String> {
    let config_path = workspace::global_config_path()?;
    profile::register_profile(&config_path, &path, &name)
}

#[tauri::command]
pub fn unregister_profile(path: String) -> Result<(), String> {
    let config_path = workspace::global_config_path()?;
    profile::unregister_profile(&config_path, &path)
}

#[tauri::command]
pub fn validate_profile_path(path: String) -> Result<profile::ProfilePathValidation, String> {
    profile::validate_profile_path(&path)
}

#[tauri::command]
pub fn list_workspaces(
    app: tauri::AppHandle,
    profile_path: String,
) -> Result<Vec<workspace::WorkspaceInfo>, String> {
    let mut warnings = Vec::new();
    let result = workspace::list_workspaces(&profile_path, &mut warnings);
    for warning in &warnings {
        crate::logging::report_error(&app, warning);
    }
    result
}

#[tauri::command]
pub fn get_workspace_status(
    workspace_path: String,
    repo_names: Vec<String>,
) -> Result<workspace::WorkspaceStatusResult, String> {
    workspace::get_workspace_status(&workspace_path, repo_names)
}

#[tauri::command]
pub fn get_all_workspace_statuses(
    app: tauri::AppHandle,
    workspaces: Vec<workspace::WorkspaceStatusInput>,
) -> Result<workspace::BatchWorkspaceStatusResult, String> {
    let mut warnings = Vec::new();
    let result = workspace::get_all_workspace_statuses(workspaces, &mut warnings);
    for warning in &warnings {
        crate::logging::report_error(&app, warning);
    }
    result
}

#[tauri::command]
pub fn get_registered_repos(
    profile_path: String,
) -> Result<Vec<workspace::RepoInfo>, String> {
    workspace::get_registered_repos(&profile_path)
}

#[tauri::command]
pub fn get_default_app(profile_path: String) -> Result<Option<String>, String> {
    workspace::get_default_app(&profile_path)
}

/// Reads the Claude model/effort settings for one scope (a profile path, or `None` for the user-level/global scope): the raw stored values plus the effective resolved values.
#[tauri::command]
pub fn get_claude_settings(
    profile_path: Option<String>,
) -> Result<workspace::ClaudeSettingsResponse, String> {
    workspace::read_claude_settings(profile_path.as_deref())
}

/// Writes one Claude model/effort setting at the given scope.
/// `state`: "value" (store `value`), "none" (store JSON null), "default" (remove the key).
#[tauri::command]
pub fn set_claude_setting(
    profile_path: Option<String>,
    key: String,
    state: String,
    value: Option<String>,
) -> Result<(), String> {
    workspace::write_claude_setting(profile_path.as_deref(), &key, &state, value.as_deref())
}

/// Start a non-blocking task creation.
/// Returns a creation id immediately; progress (steps + streamed subprocess output) and the terminal outcome arrive on `on_event`.
/// The worker reuses a matching inactive workspace (refreshing it) or builds a fresh one.
/// When `source_workspace_path` is set (duplication), each repo that also exists in the source workspace is checked out at that worktree's current commit (detached); other repos use the latest default branch, and the source workspace's `CLAUDE.md` is copied over the new one.
#[tauri::command]
pub fn create_workspace(
    app: tauri::AppHandle,
    state: State<'_, Mutex<WorkspaceCreationManager>>,
    profile_path: String,
    repo_paths: Vec<String>,
    task_name: String,
    source_workspace_path: Option<String>,
    on_event: Channel<workspace_creation::CreationEvent>,
) -> Result<u32, String> {
    let (id, handle) = {
        let mut manager = state
            .lock()
            .map_err(|e| format!("failed to lock creation manager: {e}"))?;
        manager.register()
    };

    std::thread::spawn(move || {
        workspace_creation::run(
            app,
            id,
            handle,
            profile_path,
            repo_paths,
            task_name,
            source_workspace_path,
            on_event,
        );
    });

    Ok(id)
}

#[tauri::command]
pub fn cancel_workspace_creation(
    state: State<'_, Mutex<WorkspaceCreationManager>>,
    id: u32,
) -> Result<(), String> {
    let mut manager = state
        .lock()
        .map_err(|e| format!("failed to lock creation manager: {e}"))?;
    manager.cancel(id);
    Ok(())
}

/// Add a single repo as a git worktree to an existing (active) workspace.
/// Returns a creation id immediately; progress (steps + streamed subprocess output) and the outcome arrive on `on_event`.
/// Cancellable via `cancel_workspace_creation` (shared creation manager).
#[tauri::command]
pub fn add_repo_to_workspace(
    app: tauri::AppHandle,
    state: State<'_, Mutex<WorkspaceCreationManager>>,
    workspace_path: String,
    source_repo_path: String,
    worktree_dir_name: String,
    on_event: Channel<workspace_creation::CreationEvent>,
) -> Result<u32, String> {
    let (id, handle) = {
        let mut manager = state
            .lock()
            .map_err(|e| format!("failed to lock creation manager: {e}"))?;
        manager.register()
    };

    std::thread::spawn(move || {
        workspace_creation::run_add_repo(
            app,
            id,
            handle,
            workspace_path,
            source_repo_path,
            worktree_dir_name,
            on_event,
        );
    });

    Ok(id)
}

#[tauri::command]
pub fn remove_task(workspace_path: String) -> Result<(), String> {
    workspace::remove_task(&workspace_path)
}

#[tauri::command]
pub fn prepare_repurpose_task(
    workspace_path: String,
) -> Result<workspace::RepurposeContext, String> {
    workspace::prepare_repurpose_task(&workspace_path)
}

#[tauri::command]
pub fn repurpose_task(workspace_path: String, new_title: String) -> Result<(), String> {
    workspace::repurpose_task(&workspace_path, &new_title)
}

#[tauri::command]
pub fn delete_workspace(workspace_path: String) -> Result<(), String> {
    workspace::delete_workspace(&workspace_path)
}

#[tauri::command]
pub fn save_profiling_report(
    profile_path: String,
    report_json: String,
) -> Result<String, String> {
    workspace::save_profiling_report(&profile_path, &report_json)
}

#[tauri::command]
pub fn log_error(
    state: State<'_, LogState>,
    level: String,
    message: String,
) -> Result<(), String> {
    state.write(&level, &message);
    Ok(())
}

#[tauri::command]
pub fn read_text_file(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path)
        .map_err(|e| format!("failed to read file {path}: {e}"))
}

#[tauri::command]
pub fn get_user_guide_path() -> Result<String, String> {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let path = std::path::Path::new(manifest_dir)
        .parent() // project-ember/
        .and_then(|p| p.parent()) // Devora/
        .map(|p| p.join("USER_GUIDE.md"))
        .ok_or_else(|| "could not determine repo root".to_string())?;
    if path.exists() {
        Ok(path.to_string_lossy().into_owned())
    } else {
        Err(format!("USER_GUIDE.md not found at {}", path.display()))
    }
}

#[tauri::command]
pub fn crit_overlay_dismissed(
    ipc_state: State<'_, IpcState>,
    pty_id: u32,
) -> Result<(), String> {
    ipc_state.resolve(pty_id, "dismissed".to_string());
    Ok(())
}

#[tauri::command]
pub fn __test_report(
    state: State<'_, TestHarnessState>,
    id: String,
    result: Option<String>,
    error: Option<String>,
) {
    state.resolve(&id, result, error);
}

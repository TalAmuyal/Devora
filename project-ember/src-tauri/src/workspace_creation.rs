//! Ember's entry points for task creation: run the core's creation work on a worker thread, then deregister it, surface warnings and errors through the error banner (ADR-002), and send the terminal event.
//!
//! The work itself (worktrees, prepare-command, cancellation, streamed progress) lives in the core's `devora_core::workspace_creation`.

use std::path::Path;
use std::sync::{Arc, Mutex};

use tauri::ipc::Channel;
use tauri::Manager;

pub use devora_core::workspace_creation::{CreationEvent, CreationHandle, WorkspaceCreationManager};
use devora_core::workspace_creation::{self as creation, EventSink, Outcome};

/// Adapts the frontend's channel to the core's event sink; a send that fails (the frontend went away) is dropped, as before.
fn sink(on_event: &Channel<CreationEvent>) -> EventSink {
    let channel = on_event.clone();
    Arc::new(move |event| {
        let _ = channel.send(event);
    })
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
    let result = creation::create(
        &handle,
        &profile_path,
        &repo_paths,
        &task_name,
        source_workspace_path.as_deref(),
        &sink(&on_event),
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
    let result = creation::add_repo(
        &handle,
        &workspace_path,
        &source_repo_path,
        &worktree_dir_name,
        &sink(&on_event),
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
            creation::cleanup_added_worktree(&target);
            let _ = on_event.send(CreationEvent::Cancelled);
        }
        Err(message) => {
            if handle.is_cancelled() {
                creation::cleanup_added_worktree(&target);
                let _ = on_event.send(CreationEvent::Cancelled);
            } else {
                crate::logging::report_error(&app, &message);
                let _ = on_event.send(CreationEvent::Failed { message });
            }
        }
    }
}


/// Spawned by the `clone_repo_into_profile` command.
/// Clones a git repo into `<profile>/repos/<name>`, detached, streaming progress; deregisters, then emits the terminal event.
pub fn run_clone_repo(
    app: tauri::AppHandle,
    id: u32,
    handle: CreationHandle,
    profile_path: String,
    url: String,
    on_event: Channel<CreationEvent>,
) {
    let result = creation::clone_repo(&handle, &profile_path, &url, &sink(&on_event));

    if let Some(state) = app.try_state::<Mutex<WorkspaceCreationManager>>() {
        if let Ok(mut manager) = state.lock() {
            manager.remove(id);
        }
    }

    match result {
        Ok(Outcome::Done(workspace)) => {
            let _ = on_event.send(CreationEvent::Done { workspace });
        }
        Ok(Outcome::Cancelled) => {
            let _ = on_event.send(CreationEvent::Cancelled);
        }
        Err(message) => {
            // A failure observed after cancellation reads as a cancellation; the partial clone is already cleaned up below.
            if handle.is_cancelled() {
                let _ = on_event.send(CreationEvent::Cancelled);
            } else {
                crate::logging::report_error(&app, &message);
                let _ = on_event.send(CreationEvent::Failed { message });
            }
        }
    }
}

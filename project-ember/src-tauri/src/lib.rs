mod background;
mod commands;
mod http_util;
mod logging;
mod test_harness;
mod theme;
mod workspace_creation;

// The non-UI modules live in the shared core (`project-core/`, ADR-007); importing them here keeps their `crate::` paths stable.
use devora_core::{bundle, config, credentials, health, ipc, profile, pty, terminal, workspace};

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::webview::WebviewWindowBuilder;
use tauri::{Emitter, Manager};

fn is_test_mode() -> bool {
    std::env::var("DEVORA_TEST_MODE").is_ok_and(|v| v == "1")
}

pub fn run() {
    let log = logging::init();

    let shutting_down = Arc::new(AtomicBool::new(false));
    let shutdown_done = Arc::new(AtomicBool::new(false));
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Mutex::new(pty::PtyManager::new()))
        .manage(Mutex::new(
            workspace_creation::WorkspaceCreationManager::new(),
        ))
        .manage(log)
        .setup(|app| {
            let handle = app.handle().clone();
            logging::listen_for_errors(&handle);
            let background = (|| {
                let state_dir = std::env::var_os("DEVORA_STATE_DIR")
                    .filter(|path| !path.is_empty())
                    .map(std::path::PathBuf::from)
                    .map(Ok)
                    .unwrap_or_else(|| {
                        app.path().app_local_data_dir().map_err(|e| e.to_string())
                    })?;
                background::Running::open(
                    config::global_config_path()?,
                    state_dir,
                    app.state::<Arc<logging::Log>>().inner().clone(),
                )
            })();
            if let Err(error) = &background {
                app.state::<Arc<logging::Log>>()
                    .write(logging::Level::Warn, error);
            }
            app.manage(Arc::new(background::Background(background)));

            let window_config = app
                .config()
                .app
                .windows
                .first()
                .expect("tauri.conf.json must have at least one window config")
                .clone();
            let resource_dir = app.path().resource_dir().ok();
            let version = bundle::AppVersion::read(
                resource_dir.as_deref(),
                &app.package_info().version.to_string(),
            );

            WebviewWindowBuilder::from_config(&handle, &window_config)
                .expect("failed to create WebviewWindowBuilder from config")
                .title(version.window_title("Devora Ember"))
                .build()
                .expect("failed to build main window");

            let preview_handle = handle.clone();
            let error_handle = handle.clone();
            let ipc_server = ipc::start(ipc::IpcHandlers {
                on_preview_open: Box::new(move |request| {
                    preview_handle
                        .emit("preview-open", request)
                        .map_err(|e| format!("failed to emit event: {e}"))
                }),
                on_error: Box::new(move |message| logging::report_error(&error_handle, message)),
            })
            .expect("failed to start the IPC server");
            eprintln!("Devora IPC server on port {}", ipc_server.port());
            app.manage(ipc_server);

            if is_test_mode() {
                let webview = app
                    .webview_windows()
                    .remove("main")
                    .expect("test_harness: no webview with label 'main'");
                let harness_state = test_harness::start(handle, webview);
                let port = harness_state.port;
                app.manage(harness_state);
                eprintln!("Devora test harness on port {port}");
            } else {
                app.manage(test_harness::TestHarnessState::inactive());
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::create_pty,
            commands::write_pty,
            commands::resize_pty,
            commands::close_pty,
            commands::list_profiles,
            commands::register_profile,
            commands::unregister_profile,
            commands::validate_profile_path,
            commands::list_workspaces,
            commands::get_workspace_status,
            commands::get_all_workspace_statuses,
            commands::get_registered_repos,
            commands::get_claude_settings,
            commands::set_claude_setting,
            commands::get_config_settings,
            commands::get_global_paused,
            commands::toggle_global_paused,
            commands::run_repo_updates,
            commands::set_config_setting,
            commands::get_asana_token_status,
            commands::set_asana_token,
            commands::clear_asana_token,
            commands::create_workspace,
            commands::cancel_workspace_creation,
            commands::add_repo_to_workspace,
            commands::clone_repo_into_profile,
            commands::run_health_check,
            commands::remove_task,
            commands::prepare_repurpose_task,
            commands::repurpose_task,
            commands::delete_workspace,
            commands::save_profiling_report,
            commands::log_error,
            commands::read_text_file,
            commands::get_user_guide_path,
            theme::get_theme,
            commands::__test_report,
        ])
        .build(tauri::generate_context!())
        .expect("error while building devora ember")
        .run(move |app, event| {
            if let tauri::RunEvent::ExitRequested { api, code, .. } = event {
                if !shutdown_done.load(Ordering::Acquire) {
                    api.prevent_exit();
                    if !shutting_down.swap(true, Ordering::AcqRel) {
                        let background = app.state::<Arc<background::Background>>().inner().clone();
                        let done = shutdown_done.clone();
                        let app = app.clone();
                        std::thread::spawn(move || {
                            background.shutdown();
                            done.store(true, Ordering::Release);
                            app.exit(code.unwrap_or(0));
                        });
                    }
                }
            }
        });
}

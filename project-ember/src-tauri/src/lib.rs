mod commands;
mod health;
mod http_util;
mod ipc_server;
mod logging;
mod pty;
mod test_harness;
mod theme;
mod workspace_creation;

// The non-UI modules live in the shared core (`project-core/`, ADR-007); importing them here keeps their `crate::` paths stable.
use devora_core::{credentials, profile, repo_clone, workspace};

use std::sync::Mutex;
use tauri::webview::WebviewWindowBuilder;
use tauri::Manager;

fn is_test_mode() -> bool {
    std::env::var("DEVORA_TEST_MODE").map_or(false, |v| v == "1")
}

pub fn run() {
    let log_path = logging::init();
    eprintln!("Devora Ember log: {log_path}");

    let crash_log = log_path.clone();
    std::panic::set_hook(Box::new(move |info| {
        let msg = format!("PANIC: {info}\n{:?}", std::backtrace::Backtrace::force_capture());
        eprintln!("{msg}");
        let _ = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&crash_log)
            .and_then(|mut f| {
                use std::io::Write;
                writeln!(f, "[CRASH] {msg}")
            });
    }));

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Mutex::new(pty::PtyManager::new()))
        .manage(Mutex::new(workspace_creation::WorkspaceCreationManager::new()))
        .manage(logging::LogState::new(&log_path))
        .setup(|app| {
            let handle = app.handle().clone();

            let window_config = app
                .config()
                .app
                .windows
                .first()
                .expect("tauri.conf.json must have at least one window config")
                .clone();
            let read_resource = |name: &str| {
                app.path()
                    .resource_dir()
                    .ok()
                    .and_then(|dir| std::fs::read_to_string(dir.join(name)).ok())
                    .map(|v| v.trim().to_string())
            };
            let version = read_resource("VERSION")
                .unwrap_or_else(|| app.package_info().version.to_string());

            // INSTALL_SUFFIX is written by `mise mac-install --suffix` to tell side-by-side installs apart.
            let title = match read_resource("INSTALL_SUFFIX") {
                Some(suffix) => format!("Devora {version} [{suffix}]"),
                None => format!("Devora {version}"),
            };

            WebviewWindowBuilder::from_config(&handle, &window_config)
                .expect("failed to create WebviewWindowBuilder from config")
                .title(&title)
                .build()
                .expect("failed to build main window");

            let ipc_state = ipc_server::start(handle.clone());
            eprintln!("Devora IPC server on port {}", ipc_state.port);
            app.manage(ipc_state);

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
            commands::get_default_app,
            commands::get_claude_settings,
            commands::set_claude_setting,
            commands::get_config_settings,
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
        .run(tauri::generate_context!())
        .expect("error while running devora ember");
}

mod commands;
mod http_util;
mod ipc_server;
mod logging;
mod pty;
mod test_harness;
mod theme;
mod workspace;

use std::sync::Mutex;
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
        .manage(Mutex::new(pty::PtyManager::new()))
        .manage(logging::LogState::new(&log_path))
        .setup(|app| {
            let handle = app.handle().clone();

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
                let _ = std::fs::write("/tmp/devora-ember-test-port", port.to_string());
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
            commands::crit_overlay_dismissed,
            commands::list_profiles,
            commands::list_workspaces,
            commands::get_workspace_status,
            commands::get_registered_repos,
            commands::get_default_app,
            commands::create_workspace,
            commands::log_error,
            commands::read_text_file,
            commands::get_user_guide_path,
            theme::get_theme,
            commands::__test_report,
        ])
        .run(tauri::generate_context!())
        .expect("error while running devora ember");
}

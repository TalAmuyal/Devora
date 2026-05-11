mod commands;
mod ipc_server;
mod logging;
mod pty;
mod theme;
mod workspace;

use std::sync::Mutex;
use tauri::Manager;

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
            let ipc_state = ipc_server::start(handle);
            eprintln!("Devora IPC server on port {}", ipc_state.port);
            app.manage(ipc_state);
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running devora ember");
}

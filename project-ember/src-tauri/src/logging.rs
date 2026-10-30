//! Ember's side of the core's log (`devora_core::log`, ADR-002): reported errors reach the frontend as an "app-error" event, which it surfaces via showError.

use tauri::{AppHandle, Emitter, Manager, Runtime};

pub use devora_core::log::{Level, Log};

/// Opens this run's log at `/tmp/devora-ember-<timestamp>.log`, with crashes written to it too.
pub fn init() -> Log {
    let path = devora_core::log::session_log_path("devora-ember");
    eprintln!("Devora Ember log: {}", path.display());
    devora_core::log::install_panic_hook(path.clone());
    Log::open(&path).expect("failed to open log file")
}

/// Routes the log's reported errors to the frontend; the emit's failure is deliberately discarded, since the log write already happened.
pub fn listen_for_errors<R: Runtime>(app: &AppHandle<R>) {
    let handle = app.clone();
    app.state::<Log>()
        .set_error_listener(Box::new(move |message| {
            let _ = handle.emit("app-error", serde_json::json!({ "message": message }));
        }));
}

/// THE single way to report a Rust-side error from Ember (`Log::report_error`).
pub fn report_error<R: Runtime>(app: &AppHandle<R>, message: &str) {
    if let Some(log) = app.try_state::<Log>() {
        log.report_error(message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use tauri::Listener;

    #[test]
    fn test_report_error_writes_log_and_emits_app_error_event() {
        let tmp = tempfile::tempdir().unwrap();
        let log_path = tmp.path().join("test.log");
        let app = tauri::test::mock_app();
        app.manage(Log::open(&log_path).unwrap());
        listen_for_errors(app.handle());

        let received = Arc::new(AtomicBool::new(false));
        let received_clone = received.clone();
        app.listen_any("app-error", move |event| {
            assert!(
                event.payload().contains("boom"),
                "payload: {}",
                event.payload()
            );
            received_clone.store(true, Ordering::SeqCst);
        });

        report_error(app.handle(), "boom");

        let contents = std::fs::read_to_string(&log_path).unwrap();
        assert!(contents.contains("[ERROR] boom"), "log: {contents}");
        assert!(
            received.load(Ordering::SeqCst),
            "app-error event not received"
        );
    }
}

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager, Runtime};

pub struct LogState {
    file: Mutex<File>,
}

impl LogState {
    pub fn new(path: &str) -> Self {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .expect("failed to open log file");
        Self {
            file: Mutex::new(file),
        }
    }

    pub fn write(&self, level: &str, message: &str) {
        let timestamp = std::process::Command::new("date")
            .args(["+%Y-%m-%d %H:%M:%S"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default();

        if let Ok(mut f) = self.file.lock() {
            let _ = writeln!(f, "[{timestamp}] [{level}] {message}");
            let _ = f.flush();
        }
    }
}

/*
 * THE single way to report a Rust-side error: writes it to the log file and emits an "app-error" event that the frontend surfaces via showError.
 * Emit failure is deliberately discarded — the log write already happened.
 */
pub fn report_error<R: Runtime>(app: &AppHandle<R>, message: &str) {
    if let Some(log) = app.try_state::<LogState>() {
        log.write("ERROR", message);
    }
    let _ = app.emit("app-error", serde_json::json!({ "message": message }));
}

pub fn init() -> String {
    let timestamp = std::process::Command::new("date")
        .args(["+%Y%m%d-%H%M%S"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    format!("/tmp/devora-ember-{timestamp}.log")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use tauri::{Listener, Manager};

    #[test]
    fn test_report_error_writes_log_and_emits_app_error_event() {
        let tmp = tempfile::tempdir().unwrap();
        let log_path = tmp.path().join("test.log");
        let app = tauri::test::mock_app();
        app.manage(LogState::new(&log_path.to_string_lossy()));

        let received = Arc::new(AtomicBool::new(false));
        let received_clone = received.clone();
        app.listen_any("app-error", move |event| {
            assert!(event.payload().contains("boom"), "payload: {}", event.payload());
            received_clone.store(true, Ordering::SeqCst);
        });

        report_error(app.handle(), "boom");

        let contents = std::fs::read_to_string(&log_path).unwrap();
        assert!(contents.contains("[ERROR] boom"), "log: {contents}");
        assert!(received.load(Ordering::SeqCst), "app-error event not received");
    }
}

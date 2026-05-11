use std::fs::{File, OpenOptions};
use std::io::Write;
use std::sync::Mutex;

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

pub fn init() -> String {
    let timestamp = std::process::Command::new("date")
        .args(["+%Y%m%d-%H%M%S"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    format!("/tmp/devora-ember-{timestamp}.log")
}

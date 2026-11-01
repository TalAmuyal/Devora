//! The app's log file and its error reporting (ADR-002).
//!
//! [`Log::report_error`] is THE way to report an error from the app's Rust side: it writes the log line, then tells the UI through the error listener, so an error can lose its banner at worst, never its log line.

use std::ffi::CStr;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Mutex, RwLock};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Error,
    Warn,
}

impl Level {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Error => "ERROR",
            Self::Warn => "WARN",
        }
    }
}

impl FromStr for Level {
    type Err = String;

    fn from_str(level: &str) -> Result<Self, String> {
        match level {
            "ERROR" => Ok(Self::Error),
            "WARN" => Ok(Self::Warn),
            _ => Err(format!("unknown log level: {level}")),
        }
    }
}

/// How the UI surfaces a reported error (Ember shows a banner).
pub type ErrorListener = Box<dyn Fn(&str) + Send + Sync>;

pub struct Log {
    file: Mutex<File>,
    on_error: RwLock<Option<ErrorListener>>,
}

impl Log {
    /// Opens `path` for appending, creating it if needed.
    pub fn open(path: &Path) -> io::Result<Self> {
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        Ok(Self {
            file: Mutex::new(file),
            on_error: RwLock::new(None),
        })
    }

    /// Sets the listener [`Log::report_error`] notifies, replacing any earlier one.
    /// It is set after the log is opened, because the UI it notifies usually starts later.
    pub fn set_error_listener(&self, listener: ErrorListener) {
        if let Ok(mut on_error) = self.on_error.write() {
            *on_error = Some(listener);
        }
    }

    pub fn write(&self, level: Level, message: &str) {
        let timestamp = timestamp(c"%Y-%m-%d %H:%M:%S").unwrap_or_default();
        if let Ok(mut f) = self.file.lock() {
            let _ = writeln!(f, "[{timestamp}] [{}] {message}", level.as_str());
            let _ = f.flush();
        }
    }

    /// Writes `message` as an error, then notifies the error listener, if one is set.
    pub fn report_error(&self, message: &str) {
        self.write(Level::Error, message);
        if let Ok(on_error) = self.on_error.read() {
            if let Some(listener) = on_error.as_ref() {
                listener(message);
            }
        }
    }
}

/// A fresh log path for one run of `app` (e.g. `devora-ember`): `/tmp/<app>-<timestamp>.log`.
pub fn session_log_path(app: &str) -> PathBuf {
    let timestamp = timestamp(c"%Y%m%d-%H%M%S").unwrap_or_else(|| "unknown".to_string());
    PathBuf::from(format!("/tmp/{app}-{timestamp}.log"))
}

/// Writes every panic, with its backtrace, to stderr and as a `[CRASH]` line in the log at `path`.
/// The hook opens the file itself rather than going through a [`Log`], whose lock the panicking thread may hold.
pub fn install_panic_hook(path: PathBuf) {
    std::panic::set_hook(Box::new(move |info| {
        let msg = format!(
            "PANIC: {info}\n{:?}",
            std::backtrace::Backtrace::force_capture()
        );
        eprintln!("{msg}");
        let _ = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .and_then(|mut f| writeln!(f, "[CRASH] {msg}"));
    }));
}

/// The local time, using the same formats as `date(1)` without spawning a process.
fn timestamp(format: &CStr) -> Option<String> {
    // SAFETY: time accepts a null pointer when its return value is used directly.
    let now = unsafe { libc::time(std::ptr::null_mut()) };
    format_local_time(now, format)
}

fn format_local_time(time: libc::time_t, format: &CStr) -> Option<String> {
    let mut local = std::mem::MaybeUninit::<libc::tm>::uninit();
    // SAFETY: both pointers are valid and localtime_r initializes local on success.
    if unsafe { libc::localtime_r(&time, local.as_mut_ptr()) }.is_null() {
        return None;
    }
    let mut buffer = [0u8; 64];
    // SAFETY: local was initialized above; format is NUL-terminated and buffer is
    // writable for the supplied size. strftime does not retain any pointers.
    let length = unsafe {
        libc::strftime(
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            format.as_ptr(),
            local.as_ptr(),
        )
    };
    if length == 0 {
        return None;
    }
    String::from_utf8(buffer[..length].to_vec()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn local_timestamps_keep_the_date_command_formats() {
        // Construct a local civil time without changing the process-wide timezone.
        // SAFETY: zero is valid for every field of tm, including its pointer fields.
        let mut local: libc::tm = unsafe { std::mem::zeroed() };
        local.tm_year = 126;
        local.tm_mon = 9;
        local.tm_mday = 31;
        local.tm_hour = 13;
        local.tm_min = 4;
        local.tm_sec = 5;
        local.tm_isdst = -1;
        // SAFETY: mktime accepts a valid, writable tm and resolves local timezone/DST.
        let time = unsafe { libc::mktime(&mut local) };
        assert_ne!(time, -1);
        assert_eq!(
            format_local_time(time, c"%Y-%m-%d %H:%M:%S").unwrap(),
            "2026-10-31 13:04:05"
        );
        assert_eq!(
            format_local_time(time, c"%Y%m%d-%H%M%S").unwrap(),
            "20261031-130405"
        );
    }

    #[test]
    fn a_reported_error_is_logged_and_reaches_the_listener() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("app.log");
        let log = Log::open(&path).unwrap();
        let heard = Arc::new(Mutex::new(Vec::new()));
        let sink = heard.clone();
        log.set_error_listener(Box::new(move |message| {
            sink.lock().unwrap().push(message.to_string())
        }));

        let before = timestamp(c"%Y-%m-%d %H:%M:%S").unwrap();
        log.write(Level::Warn, "careful");
        log.report_error("boom");
        let after = timestamp(c"%Y-%m-%d %H:%M:%S").unwrap();

        let contents = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = contents.lines().collect();
        assert_eq!(lines.len(), 2, "{contents}");
        for (line, message) in lines.iter().zip(["[WARN] careful", "[ERROR] boom"]) {
            assert!(
                *line == format!("[{before}] {message}") || *line == format!("[{after}] {message}"),
                "{contents}"
            );
        }
        assert_eq!(*heard.lock().unwrap(), ["boom"], "a warning is log-only");
    }

    #[test]
    fn an_error_reported_before_the_ui_listens_still_reaches_the_log() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("app.log");
        let log = Log::open(&path).unwrap();

        log.report_error("early");

        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("[ERROR] early"), "{contents}");
    }

    #[test]
    fn each_app_gets_its_own_log() {
        let ember = session_log_path("devora-ember");
        let name = ember.file_name().unwrap().to_string_lossy();
        assert!(ember.starts_with("/tmp"), "{ember:?}");
        assert!(
            name.starts_with("devora-ember-") && name.ends_with(".log"),
            "{name}"
        );
        assert!(!session_log_path("devora-sapphire")
            .to_string_lossy()
            .contains("ember"));
    }
}

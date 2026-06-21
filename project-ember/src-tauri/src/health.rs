//! In-app health check: shells out to the bundled `debi health --json` and returns its JSON for the Health Hub to render.
//!
//! `debi` stays the single source of truth for the checks.
//! The only subtlety is the environment: a Finder-launched macOS app has a minimal PATH, so to see the same tools a Devora terminal session sees we run `debi` through a login+interactive shell (`zsh -l -i`), exactly like the PTY (pty.rs).
//! The shell's own (interactive rc) stdout is discarded; `debi`'s JSON is captured via a redirect to a temp file so rc-file noise can never corrupt it.

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use tauri::Manager;

/// Monotonic counter making each run's temp-file name unique, so overlapping runs (e.g. rapid re-runs) never read each other's output.
static RUN_SEQ: AtomicU64 = AtomicU64::new(0);

/// Hard cap on the shell-out.
/// `debi`'s own checks self-limit (per-dependency version commands time out at 5s, the `gh api` call at 10s), so this only fires if the login shell itself hangs.
const HEALTH_TIMEOUT: Duration = Duration::from_secs(60);

/// Resolve the `debi` binary to invoke.
/// Prefers the copy bundled in the app (`<resources>/bundled-apps/debi`); falls back to `debi` on PATH for dev builds that have no bundle.
fn resolve_debi_path(resource_dir: Option<&Path>) -> String {
    if let Some(dir) = resource_dir {
        let bundled = dir.join("bundled-apps").join("debi");
        if bundled.is_file() {
            return bundled.to_string_lossy().into_owned();
        }
    }
    "debi".to_string()
}

/// Run `debi health --json` and return its raw JSON stdout.
/// `profile_path`, when set, is forwarded as `--profile-path` so the credential rows reflect the active profile.
pub fn run(app: &tauri::AppHandle, profile_path: Option<String>) -> Result<String, String> {
    let resource_dir = app.path().resource_dir().ok();
    let debi_path = resolve_debi_path(resource_dir.as_deref());

    // Capture debi's JSON in a temp file, isolated from any interactive-rc stdout noise the login shell may emit.
    let out_path = std::env::temp_dir().join(format!(
        "devora-health-{}-{}.json",
        std::process::id(),
        RUN_SEQ.fetch_add(1, Ordering::Relaxed)
    ));

    let mut cmd = Command::new("zsh");
    // `exec "$@" > "$DEVORA_HEALTH_OUT"`: pass argv positionally (no shell quoting of paths), redirect only debi's stdout to the temp file.
    cmd.args([
        "-l",
        "-i",
        "-c",
        "exec \"$@\" > \"$DEVORA_HEALTH_OUT\"",
        "devora-health",
    ]);
    cmd.arg(&debi_path).arg("health").arg("--json");
    if let Some(path) = profile_path.as_deref() {
        cmd.arg("--profile-path").arg(path);
    }

    cmd.env("DEVORA_HEALTH_OUT", &out_path);
    if let Some(dir) = resource_dir.as_deref() {
        cmd.env("DEVORA_RESOURCES_DIR", dir);
        let bundled_apps = dir.join("bundled-apps");
        if bundled_apps.is_dir() {
            let existing = std::env::var("PATH").unwrap_or_default();
            cmd.env("PATH", format!("{}:{existing}", bundled_apps.display()));
        }
    }

    // The shell's own stdout (rc noise) is discarded; stderr is captured for diagnostics.
    // Debi's JSON lands in the temp file via the redirect.
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::piped());

    let result = run_with_timeout(cmd, HEALTH_TIMEOUT);

    // Best-effort: always remove the temp file, regardless of outcome.
    let read_json = std::fs::read_to_string(&out_path);
    let _ = std::fs::remove_file(&out_path);

    let outcome = result?;
    if !outcome.status_success {
        let stderr = outcome.stderr.trim();
        let detail = if stderr.is_empty() {
            "debi health exited with a non-zero status".to_string()
        } else {
            stderr.to_string()
        };
        return Err(format!("debi health failed: {detail}"));
    }

    let json = read_json.map_err(|e| format!("failed to read health output: {e}"))?;
    if json.trim().is_empty() {
        let stderr = outcome.stderr.trim();
        if stderr.is_empty() {
            return Err("debi health produced no output".to_string());
        }
        return Err(format!("debi health produced no output: {stderr}"));
    }
    Ok(json)
}

struct CommandOutcome {
    status_success: bool,
    stderr: String,
}

/// Spawn `cmd`, draining stderr on a side thread (so a full pipe can't block the child), and wait up to `timeout`.
/// On timeout the child is killed and an error is returned.
fn run_with_timeout(mut cmd: Command, timeout: Duration) -> Result<CommandOutcome, String> {
    let mut child = cmd.spawn().map_err(|e| format!("failed to launch health check: {e}"))?;

    let stderr_pipe = child.stderr.take();
    let stderr_reader = thread::spawn(move || {
        let mut buf = String::new();
        if let Some(mut pipe) = stderr_pipe {
            let _ = pipe.read_to_string(&mut buf);
        }
        buf
    });

    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!(
                        "health check timed out after {}s",
                        timeout.as_secs()
                    ));
                }
                thread::sleep(Duration::from_millis(50));
            }
            Err(e) => return Err(format!("health check failed: {e}")),
        }
    };

    let stderr = stderr_reader.join().unwrap_or_default();
    Ok(CommandOutcome {
        status_success: status.success(),
        stderr,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_debi_path_prefers_bundled() {
        let tmp = std::env::temp_dir().join(format!("devora-health-test-{}", std::process::id()));
        let bundled_apps = tmp.join("bundled-apps");
        std::fs::create_dir_all(&bundled_apps).unwrap();
        let debi = bundled_apps.join("debi");
        std::fs::write(&debi, b"#!/bin/sh\n").unwrap();

        let resolved = resolve_debi_path(Some(tmp.as_path()));
        assert_eq!(resolved, debi.to_string_lossy());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn resolve_debi_path_falls_back_to_path() {
        // A resource dir without bundled-apps/debi falls back to bare "debi".
        let tmp = std::env::temp_dir().join(format!("devora-health-empty-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();

        assert_eq!(resolve_debi_path(Some(tmp.as_path())), "debi");
        assert_eq!(resolve_debi_path(None), "debi");

        let _ = std::fs::remove_dir_all(&tmp);
    }
}

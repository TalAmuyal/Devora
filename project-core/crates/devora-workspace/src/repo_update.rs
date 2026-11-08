//! Checkout-latest for eligible parent clones. Only a fetch can be interrupted.
use crate::repo_lock::RepoLock;
use devora_platform::clock::{ContinuousClock, MonotonicClock};
use std::fs;
use std::io::Read;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

pub const FETCH_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Updated(String),
    Skipped(String),
}

pub(crate) fn git_output(repo: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo)
        .stdin(Stdio::null())
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .trim_end_matches(['\r', '\n'])
        .to_owned())
}

fn git_path(repo: &Path, name: &str) -> Result<PathBuf, String> {
    Ok(PathBuf::from(git_output(
        repo,
        &["rev-parse", "--path-format=absolute", "--git-path", name],
    )?))
}

fn leftover_lock(common: &Path) -> Result<Option<PathBuf>, String> {
    let mut pending = vec![common.to_owned()];
    while let Some(dir) = pending.pop() {
        if !dir.exists() {
            continue;
        }
        for entry in fs::read_dir(&dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if path == common.join("devora-repo.lock") {
                continue;
            }
            if path.extension().is_some_and(|ext| ext == "lock") {
                return Ok(Some(path));
            }
            if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
                if path == common.join("objects") {
                    pending.extend([path.join("info"), path.join("pack")]);
                } else {
                    pending.push(path);
                }
            }
        }
    }
    Ok(None)
}

fn eligible(repo: &Path) -> Result<Option<String>, String> {
    let common = git_output(
        repo,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )?;
    if let Some(path) = leftover_lock(Path::new(&common))? {
        return Ok(Some(format!("Git lock exists at {}", path.display())));
    }
    for marker in [
        "MERGE_HEAD",
        "rebase-merge",
        "rebase-apply",
        "CHERRY_PICK_HEAD",
        "REVERT_HEAD",
        "BISECT_LOG",
        "sequencer",
    ] {
        if git_path(repo, marker)?.exists() {
            return Ok(Some(format!("{marker} in progress")));
        }
    }
    let head = git_output(repo, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    if head != "HEAD" {
        return Ok(Some("attached HEAD".into()));
    }
    if !git_output(repo, &["status", "--porcelain=v1", "--untracked-files=all"])?.is_empty() {
        return Ok(Some("dirty worktree or index".into()));
    }
    Ok(None)
}

/// Resolves the same origin/HEAD, main, master fallback as Debi Go's gcl.
fn default_branch(repo: &Path) -> Result<String, String> {
    if let Ok(branch) = git_output(
        repo,
        &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
    ) {
        return Ok(branch);
    }
    for name in ["main", "master"] {
        if git_output(
            repo,
            &["rev-parse", "--verify", &format!("refs/heads/{name}")],
        )
        .is_ok()
        {
            return Ok(format!("origin/{name}"));
        }
    }
    Err("origin/HEAD is unset and neither main nor master exists".into())
}

fn fetch(repo: &Path, stop: &AtomicBool, timeout: Duration) -> Result<(), String> {
    let mut command = Command::new("git");
    command
        .args(["fetch", "origin"])
        .current_dir(repo)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .process_group(0)
        .env("GIT_TERMINAL_PROMPT", "0");
    // An explicit repository/user SSH command retains its configuration.
    if git_output(repo, &["config", "--get", "core.sshCommand"]).is_err() {
        command.env("GIT_SSH_COMMAND", "ssh -o BatchMode=yes");
    }
    let started = ContinuousClock.now().map_err(|e| e.to_string())?;
    let mut child = command.spawn().map_err(|e| e.to_string())?;
    let mut stderr = child.stderr.take().expect("piped stderr");
    let reader = thread::spawn(move || {
        let mut text = String::new();
        let _ = stderr.read_to_string(&mut text);
        text
    });
    let outcome = loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                break if status.success() {
                    Ok(())
                } else {
                    Err(format!("git fetch exited {status}"))
                }
            }
            Err(error) => {
                unsafe {
                    libc::kill(-(child.id() as i32), libc::SIGKILL);
                }
                let _ = child.wait();
                break Err(error.to_string());
            }
            Ok(None) => {}
        }
        let now = match ContinuousClock.now() {
            Ok(now) => now,
            Err(error) => {
                unsafe {
                    libc::kill(-(child.id() as i32), libc::SIGKILL);
                }
                let _ = child.wait();
                break Err(format!("fetch clock failed: {error}"));
            }
        };
        if stop.load(Ordering::Acquire) || now.saturating_sub(started) >= timeout {
            // The fetch owns its process group. Checkout never uses this path.
            unsafe {
                libc::kill(-(child.id() as i32), libc::SIGKILL);
            }
            let _ = child.wait();
            break Err(if stop.load(Ordering::Acquire) {
                "fetch cancelled on shutdown".into()
            } else {
                format!("fetch timed out after {} seconds", timeout.as_secs())
            });
        }
        thread::sleep(Duration::from_millis(50));
    };
    let stderr = reader.join().unwrap_or_default();
    outcome.map_err(|error| format!("{error}: {}", stderr.trim()))
}

/// Hold the cooperative lock through fetch and checkout. A running checkout is
/// always reaped to completion, even after shutdown is requested.
pub fn update(repo: &Path, stop: &AtomicBool) -> Result<Outcome, String> {
    let Some(_guard) = RepoLock::acquire(repo, || stop.load(Ordering::Acquire))? else {
        return Ok(Outcome::Skipped("shutting down".into()));
    };
    if let Some(reason) = eligible(repo)? {
        return Ok(Outcome::Skipped(reason));
    }
    fetch(repo, stop, FETCH_TIMEOUT)?;
    if stop.load(Ordering::Acquire) {
        return Ok(Outcome::Skipped("shutting down".into()));
    }
    if let Some(reason) = eligible(repo)? {
        return Ok(Outcome::Skipped(reason));
    }
    let target = default_branch(repo)?;
    git_output(repo, &["checkout", "--detach", &target])?;
    Ok(Outcome::Updated(target))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::Arc;

    struct Fixture {
        _root: tempfile::TempDir,
        source: PathBuf,
        repo: PathBuf,
    }

    impl Fixture {
        fn new(branch: &str) -> Self {
            let root = tempfile::tempdir().unwrap();
            let source = root.path().join("source");
            fs::create_dir(&source).unwrap();
            git_output(&source, &["init", "-b", branch]).unwrap();
            git_output(
                &source,
                &[
                    "-c",
                    "user.name=test",
                    "-c",
                    "user.email=test@example.com",
                    "commit",
                    "--allow-empty",
                    "-m",
                    "initial",
                ],
            )
            .unwrap();
            let origin = root.path().join("origin.git");
            git_output(
                root.path(),
                &[
                    "clone",
                    "--bare",
                    source.to_str().unwrap(),
                    origin.to_str().unwrap(),
                ],
            )
            .unwrap();
            git_output(
                &source,
                &["remote", "add", "origin", origin.to_str().unwrap()],
            )
            .unwrap();
            let repo = root.path().join("clone");
            git_output(
                root.path(),
                &["clone", origin.to_str().unwrap(), repo.to_str().unwrap()],
            )
            .unwrap();
            git_output(&repo, &["checkout", "--detach"]).unwrap();
            Self {
                _root: root,
                source,
                repo,
            }
        }
        fn advance(&self) -> String {
            fs::write(self.source.join("latest.txt"), "latest").unwrap();
            git_output(&self.source, &["add", "."]).unwrap();
            git_output(
                &self.source,
                &[
                    "-c",
                    "user.name=test",
                    "-c",
                    "user.email=test@example.com",
                    "commit",
                    "-m",
                    "advance",
                ],
            )
            .unwrap();
            git_output(&self.source, &["push", "origin", "HEAD"]).unwrap();
            git_output(&self.source, &["rev-parse", "HEAD"]).unwrap()
        }
    }

    #[test]
    fn updates_detached_clones_and_preserves_dirty_attached_and_locked_repos() {
        let f = Fixture::new("main");
        let latest = f.advance();
        let stop = AtomicBool::new(false);
        fs::write(f.repo.join("untracked"), "keep").unwrap();
        assert!(matches!(
            update(&f.repo, &stop).unwrap(),
            Outcome::Skipped(_)
        ));
        fs::remove_file(f.repo.join("untracked")).unwrap();
        git_output(&f.repo, &["checkout", "main"]).unwrap();
        assert!(matches!(
            update(&f.repo, &stop).unwrap(),
            Outcome::Skipped(_)
        ));
        git_output(&f.repo, &["checkout", "--detach"]).unwrap();
        for marker in [
            "MERGE_HEAD",
            "CHERRY_PICK_HEAD",
            "REVERT_HEAD",
            "BISECT_LOG",
            "index.lock",
            "refs/heads/stale.lock",
        ] {
            let path = git_path(&f.repo, marker).unwrap();
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, "keep").unwrap();
            assert!(
                matches!(update(&f.repo, &stop).unwrap(), Outcome::Skipped(_)),
                "{marker}"
            );
            assert_eq!(fs::read_to_string(&path).unwrap(), "keep");
            fs::remove_file(path).unwrap();
        }
        for marker in ["rebase-merge", "rebase-apply", "sequencer"] {
            let path = git_path(&f.repo, marker).unwrap();
            fs::create_dir(&path).unwrap();
            assert!(
                matches!(update(&f.repo, &stop).unwrap(), Outcome::Skipped(_)),
                "{marker}"
            );
            fs::remove_dir(path).unwrap();
        }
        assert_eq!(
            update(&f.repo, &stop).unwrap(),
            Outcome::Updated("origin/main".into())
        );
        assert_eq!(git_output(&f.repo, &["rev-parse", "HEAD"]).unwrap(), latest);
        assert!(git_output(&f.repo, &["status", "--porcelain"])
            .unwrap()
            .is_empty());
    }

    #[test]
    fn default_branch_fallback_and_fetch_failures() {
        let f = Fixture::new("master");
        f.advance();
        git_output(
            &f.repo,
            &["symbolic-ref", "--delete", "refs/remotes/origin/HEAD"],
        )
        .unwrap();
        assert_eq!(
            update(&f.repo, &AtomicBool::new(false)).unwrap(),
            Outcome::Updated("origin/master".into())
        );
        git_output(
            &f.repo,
            &["remote", "set-url", "origin", "/missing/devora-test-origin"],
        )
        .unwrap();
        assert!(update(&f.repo, &AtomicBool::new(false))
            .unwrap_err()
            .contains("fetch"));
    }

    fn fifo(path: &Path) {
        let name = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
    }

    #[test]
    fn shutdown_during_checkout_finishes_it_and_leaves_the_clone_clean() {
        let f = Fixture::new("main");
        let latest = f.advance();
        let ready = f._root.path().join("ready");
        let release = f._root.path().join("release");
        fifo(&ready);
        fifo(&release);
        let hook = f.repo.join(".git/hooks/post-checkout");
        fs::write(
            &hook,
            format!(
                "#!/bin/sh\nprintf ready > '{}'\nread release < '{}'\n",
                ready.display(),
                release.display()
            ),
        )
        .unwrap();
        fs::set_permissions(hook, fs::Permissions::from_mode(0o755)).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        std::thread::scope(|scope| {
            let updater = scope.spawn(|| update(&f.repo, &stop));
            let mut signal = String::new();
            fs::File::open(ready)
                .unwrap()
                .read_to_string(&mut signal)
                .unwrap();
            assert_eq!(signal, "ready");
            stop.store(true, Ordering::Release);
            fs::File::create(release)
                .unwrap()
                .write_all(b"finish\n")
                .unwrap();
            assert!(matches!(
                updater.join().unwrap().unwrap(),
                Outcome::Updated(_)
            ));
        });
        assert_eq!(git_output(&f.repo, &["rev-parse", "HEAD"]).unwrap(), latest);
        assert!(git_output(&f.repo, &["status", "--porcelain"])
            .unwrap()
            .is_empty());
        assert!(!f.repo.join(".git/index.lock").exists());
    }
    #[test]
    fn linked_worktrees_share_the_cooperative_lock() {
        use fs2::FileExt;
        let f = Fixture::new("main");
        let linked = f._root.path().join("linked");
        git_output(
            &f.repo,
            &[
                "worktree",
                "add",
                "--detach",
                linked.to_str().unwrap(),
                "HEAD",
            ],
        )
        .unwrap();
        let guard = RepoLock::acquire(&f.repo, || false).unwrap().unwrap();
        let common = git_output(
            &linked,
            &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        )
        .unwrap();
        let contender = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(Path::new(&common).join("devora-repo.lock"))
            .unwrap();
        assert_eq!(
            contender.try_lock_exclusive().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
        assert!(RepoLock::acquire(&linked, || true).unwrap().is_none());
        drop(guard);
        assert!(RepoLock::acquire(&linked, || false).unwrap().is_some());
    }

    #[test]
    fn fetch_timeout_and_shutdown_are_bounded() {
        let f = Fixture::new("main");
        let ready = f._root.path().join("fetch-ready");
        let release = f._root.path().join("fetch-release");
        fifo(&ready);
        fifo(&release);
        let script = f._root.path().join("upload-pack");
        fs::write(
            &script,
            format!(
                "#!/bin/sh\nprintf ready > '{}'\nread release < '{}'\n",
                ready.display(),
                release.display()
            ),
        )
        .unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        git_output(
            &f.repo,
            &[
                "config",
                "remote.origin.uploadpack",
                script.to_str().unwrap(),
            ],
        )
        .unwrap();
        let stop = AtomicBool::new(false);
        std::thread::scope(|scope| {
            let fetching = scope.spawn(|| fetch(&f.repo, &stop, FETCH_TIMEOUT));
            let mut signal = String::new();
            fs::File::open(&ready)
                .unwrap()
                .read_to_string(&mut signal)
                .unwrap();
            stop.store(true, Ordering::Release);
            assert!(fetching
                .join()
                .unwrap()
                .unwrap_err()
                .contains("cancelled on shutdown"));
        });
        stop.store(false, Ordering::Release);
        assert!(fetch(&f.repo, &stop, Duration::ZERO)
            .unwrap_err()
            .contains("timed out after 0 seconds"));
    }
}

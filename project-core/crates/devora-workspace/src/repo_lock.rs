//! A cooperative, cross-process lock for mutations of one Git common directory.
use fs2::FileExt;
use std::fs::{File, OpenOptions};
use std::path::Path;
use std::thread;
use std::time::Duration;

pub struct RepoLock(File);

impl RepoLock {
    pub fn acquire(repo: &Path, cancelled: impl Fn() -> bool) -> Result<Option<Self>, String> {
        let common = crate::repo_update::git_output(
            repo,
            &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        )?;
        let path = Path::new(&common).join("devora-repo.lock");
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        loop {
            if cancelled() {
                return Ok(None);
            }
            match file.try_lock_exclusive() {
                Ok(()) => return Ok(Some(Self(file))),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(50))
                }
                Err(error) => return Err(format!("locking {}: {error}", path.display())),
            }
        }
    }
}

impl Drop for RepoLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.0);
    }
}

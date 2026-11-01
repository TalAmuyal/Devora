//! Ownership-checked recovery of an interrupted workspace creation.
use super::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct RecoveryDescriptor {
    path: PathBuf,
    repos: Vec<String>,
    fresh: bool,
    token: String,
}

impl RecoveryDescriptor {
    pub fn encode(&self) -> Result<String, String> {
        serde_json::to_string(self).map_err(|e| e.to_string())
    }
    pub fn decode(encoded: &str) -> Result<Self, String> {
        serde_json::from_str(encoded).map_err(|e| e.to_string())
    }

    pub(super) fn new(path: &Path, repos: &[String], fresh: bool) -> Self {
        Self {
            path: path.into(),
            repos: repos.into(),
            fresh,
            token: uuid::Uuid::new_v4().to_string(),
        }
    }

    fn staging(&self) -> Result<PathBuf, String> {
        Ok(self
            .path
            .parent()
            .ok_or("workspace has no parent")?
            .join(format!(".creation-{}", self.token)))
    }

    pub(super) fn claim_fresh(&self, title: &str) -> Result<(), String> {
        // Publish the selected path only after its ownership token exists.
        // A crash before rename leaves a token-named staging directory recoverable from the journal.
        let staging = self.staging()?;
        fs::create_dir(&staging).map_err(|e| e.to_string())?;
        let claim = serde_json::json!({"title": title, "creation_token": self.token});
        fs::write(staging.join("task.json"), claim.to_string()).map_err(|e| e.to_string())?;
        fs::rename(staging, &self.path).map_err(|e| e.to_string())
    }

    pub(super) fn claim(&self, title: &str) -> Result<(), String> {
        let claim = serde_json::json!({"title": title, "creation_token": self.token});
        fs::write(self.path.join("task.json"), claim.to_string()).map_err(|e| e.to_string())
    }
}

/// Returns a warning if ownership changed or creation already completed.
/// Errors leave recovery pending for a later restart.
pub fn recover(descriptor: &RecoveryDescriptor) -> Result<Option<String>, String> {
    use fs2::FileExt;
    let parent = descriptor.path.parent().ok_or("workspace has no parent")?;
    let lock = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(parent.join(".creation-lock"))
        .map_err(|e| e.to_string())?;
    lock.lock_exclusive().map_err(|e| e.to_string())?;
    let staging = descriptor.staging()?;
    if descriptor.fresh && staging.exists() {
        fs::remove_dir_all(staging).map_err(|e| e.to_string())?;
    }
    if !descriptor.path.exists() {
        return Ok(None);
    }
    let claim = match fs::read(descriptor.path.join("task.json")) {
        Ok(bytes) => {
            serde_json::from_slice::<serde_json::Value>(&bytes).map_err(|e| e.to_string())?
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Some(format!(
                "recovery left {} alone: no matching claim",
                descriptor.path.display()
            )));
        }
        Err(e) => return Err(e.to_string()),
    };
    if claim.get("creation_token").and_then(|v| v.as_str()) != Some(&descriptor.token) {
        return Ok(Some(format!(
            "recovery left {} alone: completed or claimed by another creation",
            descriptor.path.display()
        )));
    }
    cleanup_path(&descriptor.path, &descriptor.repos, descriptor.fresh)?;
    Ok(None)
}

pub(super) fn cleanup_path(path: &Path, repos: &[String], fresh: bool) -> Result<(), String> {
    let mut errors = Vec::new();
    if fresh {
        for repo in repos {
            let worktree = path.join(repo);
            if worktree.join(".git").exists() {
                match Command::new("git")
                    .args(["worktree", "remove", "--force", "."])
                    .current_dir(&worktree)
                    .output()
                {
                    Ok(output) if output.status.success() => {}
                    Ok(output) => errors.push(String::from_utf8_lossy(&output.stderr).into_owned()),
                    Err(error) => errors.push(error.to_string()),
                }
            }
        }
        if let Err(error) = fs::remove_dir_all(path) {
            errors.push(error.to_string());
        }
    } else if let Err(error) = fs::remove_file(path.join("task.json")) {
        errors.push(error.to_string());
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleanup_continues_past_a_broken_worktree() {
        let root = tempfile::tempdir().unwrap();
        let ws = root.path().join("ws-1");
        let repo = root.path().join("source");
        fs::create_dir_all(ws.join("broken")).unwrap();
        fs::write(ws.join("broken/.git"), "invalid").unwrap();
        fs::create_dir(&repo).unwrap();
        for args in [
            vec!["init", "-q"],
            vec![
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "commit",
                "--allow-empty",
                "-qm",
                "initial",
            ],
            vec![
                "worktree",
                "add",
                "--detach",
                ws.join("good").to_str().unwrap(),
                "HEAD",
            ],
        ] {
            assert!(Command::new("git")
                .args(args)
                .current_dir(&repo)
                .status()
                .unwrap()
                .success());
        }
        assert!(cleanup_path(&ws, &["broken".into(), "good".into()], true).is_err());
        assert!(!ws.exists());
        let output = Command::new("git")
            .args(["worktree", "list", "--porcelain"])
            .current_dir(&repo)
            .output()
            .unwrap();
        assert!(!String::from_utf8_lossy(&output.stdout).contains("/good"));
    }

    #[test]
    fn malformed_recovery_paths_return_errors() {
        let descriptor = RecoveryDescriptor::new(Path::new("/"), &[], true);
        assert!(descriptor.staging().is_err());
        assert!(recover(&descriptor).is_err());
    }

    #[test]
    fn a_failed_journal_commit_prevents_the_claim() {
        let root = tempfile::tempdir().unwrap();
        let sink: EventSink = Arc::new(|_| {});
        let result = create_with_claim(
            &CreationHandle::new(),
            root.path().to_str().unwrap(),
            &[],
            "task",
            None,
            &sink,
            &mut Vec::new(),
            &|descriptor| {
                assert!(!descriptor.path.exists());
                Err("journal failed".into())
            },
        );
        assert_eq!(result.unwrap_err(), "journal failed");
        assert!(!root.path().join("workspaces/ws-1").exists());
    }

    #[test]
    fn recovery_removes_a_fresh_claim_interrupted_before_publication() {
        let root = tempfile::tempdir().unwrap();
        let descriptor = RecoveryDescriptor::new(&root.path().join("ws-1"), &[], true);
        fs::create_dir(descriptor.staging().unwrap()).unwrap();
        fs::write(descriptor.staging().unwrap().join("task.json"), "partial").unwrap();
        assert_eq!(recover(&descriptor).unwrap(), None);
        assert!(!descriptor.staging().unwrap().exists());
        assert!(!descriptor.path.exists());
    }
}

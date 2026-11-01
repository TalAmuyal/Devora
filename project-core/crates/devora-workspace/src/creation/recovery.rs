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

    fn staging(&self) -> PathBuf {
        self.path
            .parent()
            .unwrap()
            .join(format!(".creation-{}", self.token))
    }

    pub(super) fn claim_fresh(&self, title: &str) -> Result<(), String> {
        // Publish the selected path only after its ownership token exists.
        // A crash before rename leaves a token-named staging directory recoverable from the journal.
        let staging = self.staging();
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
    let staging = descriptor.staging();
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
    if fresh {
        for repo in repos {
            let worktree = path.join(repo);
            if worktree.join(".git").exists() {
                let output = Command::new("git")
                    .args(["worktree", "remove", "--force", "."])
                    .current_dir(&worktree)
                    .output()
                    .map_err(|e| e.to_string())?;
                if !output.status.success() {
                    return Err(String::from_utf8_lossy(&output.stderr).into_owned());
                }
            }
        }
        fs::remove_dir_all(path).map_err(|e| e.to_string())?;
    } else {
        fs::remove_file(path.join("task.json")).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
        fs::create_dir(descriptor.staging()).unwrap();
        fs::write(descriptor.staging().join("task.json"), "partial").unwrap();
        assert_eq!(recover(&descriptor).unwrap(), None);
        assert!(!descriptor.staging().exists());
        assert!(!descriptor.path.exists());
    }
}

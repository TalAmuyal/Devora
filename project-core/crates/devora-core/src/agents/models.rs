//! The models each profile's agents are offered (`devora_agent::models`), cached per profile: every agent start fills it
//! for free from its initialize reply, and `Agents::model_options` probes the CLI when the cache is missing, older than a
//! day, or from another CLI version.

use super::{lock, Agents, Inner};
use devora_agent::models::{self, ModelList};
use devora_agent_space::{LaunchContext, Space};
use devora_app::log::{Level, Log};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// How long a list serves before the CLI is asked again, even at the same version.
const FRESH: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Clone)]
pub(super) struct Cached {
    list: ModelList,
    at: Instant,
}

/// What an agent of `profile` was just offered: logged, as the record of what a real login gets, and cached.
pub(super) fn learned(inner: &Mutex<Inner>, log: &Log, profile: &Path, list: ModelList) {
    let models: Vec<String> = list
        .models
        .iter()
        .map(|model| match &model.resolved_model {
            Some(resolved) if *resolved != model.value => format!("{} → {resolved}", model.value),
            _ => model.value.clone(),
        })
        .collect();
    log.write(
        Level::Info,
        &format!(
            "Claude models for {} (CLI {}): {}",
            profile.display(),
            list.cli_version.as_deref().unwrap_or("unknown"),
            models.join(", ")
        ),
    );
    lock(inner).models.insert(
        profile.to_path_buf(),
        Cached {
            list,
            at: Instant::now(),
        },
    );
}

/// Whether a list cached `age` ago still answers for the CLI version installed now.
fn serves(list: &ModelList, age: Duration, installed: &str) -> bool {
    age < FRESH && list.cli_version.as_deref() == Some(installed)
}

impl Agents {
    /// The models an agent of `profile` is offered, launched with `context`: the cached list while it's under a day old
    /// and from the CLI version installed now, else a fresh one from an initialize-only launch.
    pub async fn model_options(
        &self,
        profile: &Path,
        context: LaunchContext,
    ) -> Result<ModelList, String> {
        let profile: PathBuf = profile.to_path_buf();
        let cached = lock(&self.inner).models.get(&profile).cloned();
        let inner = self.inner.clone();
        let log = self.log.clone();
        let (reply, received) = tokio::sync::oneshot::channel();
        self.runtime.spawn(async move {
            let result = async {
                let options = Space::probe_options(&context, &profile)?;
                if let Some(cached) = cached.filter(|cached| cached.at.elapsed() < FRESH) {
                    let installed = models::cli_version(&options).await?;
                    if serves(&cached.list, cached.at.elapsed(), &installed) {
                        return Ok(cached.list);
                    }
                }
                let list = models::discover(&options).await?;
                learned(&inner, &log, &profile, list.clone());
                Ok(list)
            }
            .await;
            let _ = reply.send(result);
        });
        received.await.map_err(|e| e.to_string())?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(version: Option<&str>) -> ModelList {
        ModelList {
            cli_version: version.map(str::to_owned),
            models: vec![],
        }
    }

    #[test]
    fn a_list_serves_for_a_day_and_only_the_cli_version_that_gave_it() {
        let hour = Duration::from_secs(60 * 60);
        assert!(serves(&list(Some("2.1.326")), hour, "2.1.326"));
        assert!(
            !serves(&list(Some("2.1.326")), hour, "2.1.330"),
            "an upgraded CLI is asked again"
        );
        assert!(
            !serves(&list(Some("2.1.326")), FRESH, "2.1.326"),
            "a day old"
        );
        assert!(
            !serves(&list(None), hour, "2.1.326"),
            "an unknown version never matches"
        );
    }
}

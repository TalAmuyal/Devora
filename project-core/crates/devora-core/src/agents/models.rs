//! The models each profile's agents are offered (`devora_agent::models`), cached per profile: every agent start fills it
//! for free from its initialize reply, and `Agents::model_options` probes the CLI when the cache is missing, older than a
//! day, from another CLI version, or from other launch settings (the profile's model tiers decide what an alias
//! resolves to).

use super::{lock, Agents, Inner};
use devora_agent::models::{self, ModelList};
use devora_agent_space::{LaunchContext, Space};
use devora_app::log::{Level, Log};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// How long a list serves before the CLI is asked again, even at the same version.
const FRESH: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Clone)]
pub(super) struct Cached {
    list: ModelList,
    /// The launch settings the list was given under.
    settings: Value,
    at: Instant,
}

/// What an agent of `profile`, launched with `settings`, was just offered: logged, as the record of what a real login
/// gets, and cached.
pub(super) fn learned(
    inner: &Mutex<Inner>,
    log: &Log,
    profile: &Path,
    settings: &Value,
    list: ModelList,
) {
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
            settings: settings.clone(),
            at: Instant::now(),
        },
    );
}

/// Whether a cached list still answers for the CLI version installed now and the launch settings an agent would get now.
fn serves(cached: &Cached, age: Duration, installed: &str, settings: &Value) -> bool {
    age < FRESH
        && cached.list.cli_version.as_deref() == Some(installed)
        && cached.settings == *settings
}

impl Agents {
    /// The models an agent of `profile` is offered, launched with `context`: the cached list while it's under a day old
    /// and from the CLI version and launch settings an agent would get now, else a fresh one from an initialize-only
    /// launch.
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
                    if serves(&cached, cached.at.elapsed(), &installed, &options.settings) {
                        return Ok(cached.list);
                    }
                }
                let list = models::discover(&options).await?;
                learned(&inner, &log, &profile, &options.settings, list.clone());
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
    use serde_json::json;

    fn tiers(opus: &str) -> Value {
        json!({"env": {"ANTHROPIC_DEFAULT_OPUS_MODEL": opus}})
    }

    fn cached(version: Option<&str>, settings: Value) -> Cached {
        Cached {
            list: ModelList {
                cli_version: version.map(str::to_owned),
                models: vec![],
            },
            settings,
            at: Instant::now(),
        }
    }

    #[test]
    fn a_list_serves_for_a_day_and_only_the_cli_version_that_gave_it() {
        let hour = Duration::from_secs(60 * 60);
        let settings = tiers("claude-opus-5-5");
        let list = cached(Some("2.1.326"), settings.clone());
        assert!(serves(&list, hour, "2.1.326", &settings));
        assert!(
            !serves(&list, hour, "2.1.330", &settings),
            "an upgraded CLI is asked again"
        );
        assert!(!serves(&list, FRESH, "2.1.326", &settings), "a day old");
        assert!(
            !serves(&cached(None, settings.clone()), hour, "2.1.326", &settings),
            "an unknown version never matches"
        );
    }

    #[test]
    fn a_list_serves_only_the_launch_settings_it_was_given_under() {
        let hour = Duration::from_secs(60 * 60);
        let list = cached(Some("2.1.326"), tiers("claude-opus-5-5"));
        assert!(
            serves(&list, hour, "2.1.326", &tiers("claude-opus-5-5")),
            "an unchanged tier"
        );
        assert!(
            !serves(&list, hour, "2.1.326", &tiers("claude-opus-4-8")),
            "a changed tier resolves the aliases differently"
        );
    }
}

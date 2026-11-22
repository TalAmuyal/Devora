//! The models each profile's agents are offered (`devora_agent::models`), cached per profile: every agent start fills it
//! for free from its initialize reply, and `Agents::model_options` probes the CLI when the cache is missing, older than a
//! day, from another CLI version, or from other launch settings (the profile's model tiers decide what an alias
//! resolves to).

use super::Agents;
use devora_agent::models::{self, ModelList};
use devora_agent_space::{LaunchContext, Space};
use devora_app::log::{Level, Log};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How long a list serves before the CLI is asked again, even at the same version.
const FRESH: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Clone)]
struct Cached {
    list: ModelList,
    /// The launch settings the list was given under.
    settings: Value,
    at: Instant,
}

/// The models each profile is offered, cached per profile. The agent registry keeps one, which every agent start fills;
/// a host without agents (Ember's settings) keeps its own.
pub struct ModelOptions {
    cache: Mutex<BTreeMap<PathBuf, Cached>>,
    log: Arc<Log>,
}

impl ModelOptions {
    pub fn new(log: Arc<Log>) -> Self {
        Self {
            cache: Mutex::new(BTreeMap::new()),
            log,
        }
    }

    /// What an agent of `profile`, launched with `settings`, was just offered: logged, as the record of what a real login
    /// gets, and cached.
    pub(super) fn learned(&self, profile: &Path, settings: &Value, list: ModelList) {
        let models: Vec<String> = list
            .models
            .iter()
            .map(|model| match &model.resolved_model {
                Some(resolved) if *resolved != model.value => {
                    format!("{} → {resolved}", model.value)
                }
                _ => model.value.clone(),
            })
            .collect();
        self.log.write(
            Level::Info,
            &format!(
                "Claude models for {} (CLI {}): {}",
                profile.display(),
                list.cli_version.as_deref().unwrap_or("unknown"),
                models.join(", ")
            ),
        );
        self.cache.lock().unwrap_or_else(|e| e.into_inner()).insert(
            profile.to_path_buf(),
            Cached {
                list,
                settings: settings.clone(),
                at: Instant::now(),
            },
        );
    }

    /// The list cached for `profile` under exactly these launch settings, however old: what an agent launched with them
    /// was offered, for reading its replies' models without asking the CLI.
    pub(super) fn cached(&self, profile: &Path, settings: &Value) -> Option<ModelList> {
        self.cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(profile)
            .filter(|cached| cached.settings == *settings)
            .map(|cached| cached.list.clone())
    }

    /// The models an agent of `profile` is offered, launched with `context`: the cached list while it's under a day old
    /// and from the CLI version and launch settings an agent would get now, else a fresh one from an initialize-only
    /// launch.
    pub async fn get(&self, profile: &Path, context: &LaunchContext) -> Result<ModelList, String> {
        let options = Space::probe_options(context, profile)?;
        let cached = self
            .cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(profile)
            .cloned();
        if let Some(cached) = cached.filter(|cached| cached.at.elapsed() < FRESH) {
            let installed = models::cli_version(&options).await?;
            if serves(&cached, cached.at.elapsed(), &installed, &options.settings) {
                return Ok(cached.list);
            }
        }
        let list = models::discover(&options).await?;
        self.learned(profile, &options.settings, list.clone());
        Ok(list)
    }
}

/// Whether a cached list still answers for the CLI version installed now and the launch settings an agent would get now.
fn serves(cached: &Cached, age: Duration, installed: &str, settings: &Value) -> bool {
    age < FRESH
        && cached.list.cli_version.as_deref() == Some(installed)
        && cached.settings == *settings
}

impl Agents {
    /// The models an agent of `profile` is offered, launched with `context` (`ModelOptions::get`), on the registry's runtime.
    pub async fn model_options(
        &self,
        profile: &Path,
        context: LaunchContext,
    ) -> Result<ModelList, String> {
        let profile: PathBuf = profile.to_path_buf();
        let models = super::lock(&self.inner).models.clone();
        self.runtime
            .spawn(async move { models.get(&profile, &context).await })
            .await
            .map_err(|e| e.to_string())?
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

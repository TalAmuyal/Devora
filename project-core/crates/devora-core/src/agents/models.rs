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
use tokio::sync::watch;
use tokio::task::JoinHandle;

/// How long a list serves before the CLI is asked again, even at the same version.
const FRESH: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Clone)]
struct Cached {
    list: ModelList,
    /// The launch settings the list was given under.
    settings: Value,
    at: Instant,
}

/// A lookup's outcome, once it has one.
type Outcome = Option<Result<ModelList, String>>;

/// A lookup under way: the profile and launch settings it asks for, its outcome as it arrives, and its task.
struct Probe {
    id: u64,
    profile: PathBuf,
    settings: Value,
    outcome: watch::Receiver<Outcome>,
    task: JoinHandle<()>,
}

#[derive(Default)]
struct Probes {
    running: Vec<Probe>,
    next_id: u64,
    /// Set by `shutdown`: no lookup starts after it.
    closed: bool,
}

/// The models each profile is offered, cached per profile. The agent registry keeps one, which every agent start fills;
/// a host without agents (Ember's settings) keeps its own. It runs one lookup at a time per profile and launch settings
/// (callers asking meanwhile share it), each as a task of its own that `shutdown` ends, so that a quitting app leaves no
/// CLI behind.
pub struct ModelOptions {
    cache: Mutex<BTreeMap<PathBuf, Cached>>,
    probes: Mutex<Probes>,
    /// The aliases Claude Code listed that Devora doesn't know as a tier, each logged once.
    unknown_aliases: Mutex<std::collections::BTreeSet<String>>,
    log: Arc<Log>,
}

impl ModelOptions {
    pub fn new(log: Arc<Log>) -> Self {
        Self {
            cache: Mutex::new(BTreeMap::new()),
            probes: Mutex::new(Probes::default()),
            unknown_aliases: Mutex::new(std::collections::BTreeSet::new()),
            log,
        }
    }

    /// What an agent of `profile`, launched with `settings`, was just offered: logged, as the record of what a real login
    /// gets, and cached.
    pub(super) fn learned(&self, profile: &Path, settings: &Value, list: ModelList) {
        for alias in list.unknown_aliases() {
            if self
                .unknown_aliases
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(alias.to_owned())
            {
                self.log.write(
                    Level::Info,
                    &format!("Claude Code lists `{alias}`, a tier Devora doesn't know; its model lists show it first"),
                );
            }
        }
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
    pub async fn get(
        self: &Arc<Self>,
        profile: &Path,
        context: &LaunchContext,
    ) -> Result<ModelList, String> {
        if self.lock_probes().closed {
            return Err(QUITTING.into());
        }
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
        let mut outcome = self.probe(profile, options)?;
        let result = outcome
            .wait_for(Option::is_some)
            .await
            .map_err(|_| "the model lookup was stopped".to_owned())?
            .clone();
        result.expect("waited for an outcome")
    }

    /// The lookup for `profile` under `options`' launch settings: the one under way, or a new one, started as a task of its
    /// own, which caches the list and then leaves the running lookups, failed or not, so that the next call asks again.
    fn probe(
        self: &Arc<Self>,
        profile: &Path,
        options: devora_agent::session::SessionOptions,
    ) -> Result<watch::Receiver<Outcome>, String> {
        let mut probes = self.lock_probes();
        if probes.closed {
            return Err(QUITTING.into());
        }
        if let Some(probe) = probes
            .running
            .iter()
            .find(|probe| probe.profile == profile && probe.settings == options.settings)
        {
            return Ok(probe.outcome.clone());
        }
        let (sender, outcome) = watch::channel(None);
        let id = probes.next_id;
        probes.next_id += 1;
        let this = self.clone();
        let key = profile.to_path_buf();
        let settings = options.settings.clone();
        let task = tokio::spawn(async move {
            let result = models::discover(&options).await;
            if let Ok(list) = &result {
                this.learned(&key, &options.settings, list.clone());
            }
            this.lock_probes().running.retain(|probe| probe.id != id);
            let _ = sender.send(Some(result));
        });
        probes.running.push(Probe {
            id,
            profile: profile.to_path_buf(),
            settings,
            outcome: outcome.clone(),
            task,
        });
        Ok(outcome)
    }

    /// Ends every lookup under way, which kills its CLI, and refuses new ones: a quitting app leaves no CLI behind. Waits at
    /// most `grace` for them to end; one that hasn't by then is logged, and the quit goes on.
    pub async fn shutdown(&self, grace: Duration) {
        let running = {
            let mut probes = self.lock_probes();
            probes.closed = true;
            std::mem::take(&mut probes.running)
        };
        let tasks: Vec<JoinHandle<()>> = running
            .into_iter()
            .map(|probe| {
                probe.task.abort();
                probe.task
            })
            .collect();
        let count = tasks.len();
        let ended = tokio::time::timeout(grace, async {
            for task in tasks {
                let _ = task.await;
            }
        });
        if ended.await.is_err() {
            self.log.write(
                Level::Warn,
                &format!("{count} model lookup(s) still ending after {grace:?}; quitting anyway"),
            );
        }
    }

    fn lock_probes(&self) -> std::sync::MutexGuard<'_, Probes> {
        self.probes.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// How long a quit waits for the model lookups it ends.
pub const MODEL_LOOKUP_GRACE: Duration = Duration::from_secs(2);

/// Why a lookup is refused once `shutdown` has run.
const QUITTING: &str = "Devora is quitting, so it doesn't ask Claude Code for its models";

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

    /// An initialize-only lookup by the fake CLI that never gets its answer (`DEVORA_FAKE_PROBE_STALL`).
    fn stalled_lookup(dir: &Path) -> devora_agent::session::SessionOptions {
        let agent = Path::new(env!("CARGO_MANIFEST_DIR")).join("../devora-agent/tests");
        devora_agent::session::SessionOptions {
            program: agent.join("support/fake_cli.py"),
            working_directory: dir.to_path_buf(),
            instructions: devora_agent::session::Instructions::Append(String::new()),
            model: String::new(),
            effort: None,
            permission_mode: "default".into(),
            plugins: vec![],
            settings: json!({}),
            environment: BTreeMap::from([
                ("PATH".into(), std::env::var("PATH").unwrap()),
                (
                    "DEVORA_FAKE_FIXTURES".into(),
                    agent.join("fixtures/claude-2.1.312").display().to_string(),
                ),
                (
                    "DEVORA_FAKE_REPORT".into(),
                    dir.join("report.jsonl").display().to_string(),
                ),
                ("DEVORA_FAKE_PROBE_STALL".into(), "1".into()),
            ]),
            resume: None,
        }
    }

    #[tokio::test]
    async fn lookups_asked_while_one_runs_share_it_and_quitting_ends_it_and_refuses_more() {
        let dir = std::env::temp_dir().join(format!("devora-core-lookups-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let models = Arc::new(ModelOptions::new(Arc::new(
            Log::open(&dir.join("log")).unwrap(),
        )));
        let profile = dir.join("profile");
        let mut first = models.probe(&profile, stalled_lookup(&dir)).unwrap();
        let second = models.probe(&profile, stalled_lookup(&dir)).unwrap();
        assert!(first.same_channel(&second), "one lookup for both");

        models.shutdown(MODEL_LOOKUP_GRACE).await;
        assert!(
            first.wait_for(Option::is_some).await.is_err(),
            "the lookup was ended, so its waiters get no list"
        );
        assert!(
            models.probe(&profile, stalled_lookup(&dir)).is_err(),
            "no lookup starts after the shutdown"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

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

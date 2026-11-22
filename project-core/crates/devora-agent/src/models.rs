//! The models Claude Code offers: the `models` list of its initialize reply, which every session receives, and which
//! [`discover`] asks for with an initialize-only launch when no session has.
//!
//! It's the CLI's aliases (`opus`, `sonnet`, ...) and what each resolves to today for this account and CLI version, not
//! every model ID the API accepts: older IDs stay free text.

use crate::protocol::InitializeResponse;
use crate::session::SessionOptions;
use serde_json::{json, Value};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::time::timeout;

/// How long the probe waits for the initialize reply, and then for the CLI to exit.
const PROBE_TIMEOUT: Duration = Duration::from_secs(30);

/// One model the CLI offers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelOption {
    /// What to pass as the model, and to store: an alias such as `opus`, which follows the CLI to newer models.
    pub value: String,
    /// The model ID the alias resolves to today, for showing beside it.
    pub resolved_model: Option<String>,
    pub display_name: String,
    pub description: Option<String>,
    /// The effort levels it supports, lowest first; empty when it takes none.
    pub effort_levels: Vec<String>,
}

/// The CLI's model list, with the CLI version that gave it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelList {
    pub cli_version: Option<String>,
    pub models: Vec<ModelOption>,
}

/// The models an initialize reply lists. An entry without a `value` is skipped; the other fields are optional.
pub fn decode(reply: &InitializeResponse) -> ModelList {
    let text = |model: &Value, key: &str| model.get(key).and_then(Value::as_str).map(str::to_owned);
    let models = reply
        .models
        .iter()
        .filter_map(|model| {
            let value = text(model, "value")?;
            let effort_levels =
                if model.get("supportsEffort").and_then(Value::as_bool) == Some(false) {
                    vec![]
                } else {
                    model
                        .get("supportedEffortLevels")
                        .and_then(Value::as_array)
                        .map(|levels| {
                            levels
                                .iter()
                                .filter_map(Value::as_str)
                                .map(str::to_owned)
                                .collect()
                        })
                        .unwrap_or_default()
                };
            Some(ModelOption {
                display_name: text(model, "displayName")
                    .filter(|name| !name.is_empty())
                    .unwrap_or_else(|| value.clone()),
                resolved_model: text(model, "resolvedModel"),
                description: text(model, "description"),
                effort_levels,
                value,
            })
        })
        .collect();
    ModelList {
        cli_version: reply
            .extra
            .get("claude_code_version")
            .and_then(Value::as_str)
            .map(str::to_owned),
        models,
    }
}

/// A full model ID to offer where only an ID will do (a model tier, `ANTHROPIC_DEFAULT_<TIER>_MODEL`, is what an alias
/// resolves to), with its model's name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelId {
    pub id: String,
    pub name: String,
}

impl ModelOption {
    /// An alias names a model it resolves to (`opus` → `claude-opus-5-5`); every other entry is a full ID.
    pub fn is_alias(&self) -> bool {
        self.resolved_model
            .as_ref()
            .is_some_and(|resolved| *resolved != self.value)
    }

    /// The full ID it stands for: what an alias resolves to, or the entry itself.
    pub fn model_id(&self) -> &str {
        self.resolved_model.as_deref().unwrap_or(&self.value)
    }
}

impl ModelList {
    /// The name the CLI gives the model `id` (a full ID), if it lists that model.
    pub fn name_of(&self, id: &str) -> Option<&str> {
        self.models
            .iter()
            .filter(|model| model.value != "default")
            .find(|model| same_model(model.resolved_model.as_deref().unwrap_or(&model.value), id))
            .map(|model| model.display_name.as_str())
    }

    /// The full ID an agent set to `setting` runs: an alias through the entry the CLI lists for it (which follows the
    /// profile's model tiers), a full ID as itself; none for an alias it doesn't list.
    pub fn resolve<'a>(&'a self, setting: &'a str) -> Option<&'a str> {
        match self.models.iter().find(|model| model.value == setting) {
            Some(model) => Some(model.resolved_model.as_deref().unwrap_or(&model.value)),
            None => setting.starts_with("claude-").then_some(setting),
        }
    }

    /// Whether a reply from `model` is what an agent set to `setting` runs: what the setting resolves to, or for
    /// `opusplan` its Opus or its Sonnet. A setting this list can't resolve expects anything, rather than a guess.
    pub fn expects(&self, setting: &str, model: &str) -> bool {
        if same_model(setting, model) {
            return true;
        }
        if setting == "opusplan" {
            return ["opus", "sonnet"].iter().any(|alias| {
                self.resolve(alias)
                    .is_some_and(|resolved| same_model(resolved, model))
            });
        }
        self.resolve(setting)
            .is_none_or(|resolved| same_model(resolved, model))
    }

    /// What an agent's model can be: every entry but Claude Code's `default`, which is a choice rather than a model, in
    /// [`precedes`]'s order. Picking an alias stores the alias, which follows Claude Code to newer models.
    pub fn agent_choices(&self) -> Vec<ModelOption> {
        let mut choices: Vec<_> = self
            .models
            .iter()
            .filter(|model| model.value != "default")
            .cloned()
            .collect();
        choices.sort_by(|a, b| order(a.model_id(), b.model_id()));
        choices
    }

    /// What a model tier can be: each full ID once, an alias's resolution included and named by its alias (`default`,
    /// a choice, names none), in [`precedes`]'s order.
    pub fn tier_choices(&self) -> Vec<ModelId> {
        let mut ids: Vec<ModelId> = Vec::new();
        for model in self.models.iter().filter(|model| model.value != "default") {
            if !ids.iter().any(|known| known.id == model.model_id()) {
                ids.push(ModelId {
                    id: model.model_id().to_owned(),
                    name: model.display_name.clone(),
                });
            }
        }
        ids.sort_by(|a, b| order(&a.id, &b.id));
        ids
    }
}

/// Whether model `first` reads before `second`: Claude's models (`claude-<tier>-<major>[-<minor>][-<date>]`) by
/// generation, newest first, then by tier (Fable, Opus, Sonnet, Haiku); every other ID after them, alphabetically.
pub fn precedes(first: &str, second: &str) -> bool {
    order(first, second) == std::cmp::Ordering::Less
}

fn order(first: &str, second: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    match (Generation::of(first), Generation::of(second)) {
        (Some(a), Some(b)) => b
            .version
            .cmp(&a.version)
            .then(a.tier.cmp(&b.tier))
            .then_with(|| first.cmp(second)),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => first.cmp(second),
    }
}

struct Generation {
    version: (u32, u32),
    tier: usize,
}

impl Generation {
    const TIERS: [&str; 4] = ["fable", "opus", "sonnet", "haiku"];

    fn of(id: &str) -> Option<Self> {
        let mut parts = id.split('-');
        if parts.next()? != "claude" {
            return None;
        }
        let family = parts.next()?;
        let tier = Self::TIERS.iter().position(|tier| *tier == family)?;
        let major = parts.next()?.parse().ok()?;
        // A date suffix (`-20251001`) isn't a minor version.
        let minor = parts
            .next()
            .and_then(|part| part.parse().ok())
            .filter(|minor| *minor < 1000)
            .unwrap_or(0);
        Some(Self {
            version: (major, minor),
            tier,
        })
    }
}

/// Launches the CLI as `options` describe a session (its program, directory, environment and settings, so its login and
/// tier settings are the ones an agent would have), asks only for its initialize reply, and closes its input, after which
/// it exits by itself. The launch starts none of the user's MCP servers or hooks and saves no session.
pub async fn discover(options: &SessionOptions) -> Result<ModelList, String> {
    let mut settings = options
        .settings
        .as_object()
        .cloned()
        .ok_or("settings must be a JSON object")?;
    settings.insert("disableAllHooks".into(), json!(true));
    let settings = serde_json::to_string(&settings).map_err(|error| error.to_string())?;
    let mut child = tokio::process::Command::new(&options.program)
        .args([
            "-p",
            "--input-format",
            "stream-json",
            "--output-format",
            "stream-json",
            "--verbose",
            "--strict-mcp-config",
            "--no-session-persistence",
            "--settings",
            &settings,
        ])
        .env_clear()
        .envs(&options.environment)
        .current_dir(&options.working_directory)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| format!("couldn't start Claude to list its models: {error}"))?;
    let mut input = child.stdin.take().ok_or("Claude's input isn't piped")?;
    let mut output =
        BufReader::new(child.stdout.take().ok_or("Claude's output isn't piped")?).lines();
    let request = json!({"type": "control_request", "request_id": "devora-models", "request": {"subtype": "initialize", "hooks": {}}});
    let reply = timeout(PROBE_TIMEOUT, async {
        input
            .write_all(format!("{request}\n").as_bytes())
            .await
            .map_err(|error| format!("couldn't ask Claude for its models: {error}"))?;
        while let Some(line) = output
            .next_line()
            .await
            .map_err(|error| error.to_string())?
        {
            let Ok(message) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if message["type"] == "control_response"
                && message["response"]["request_id"] == "devora-models"
            {
                if message["response"]["subtype"] != "success" {
                    return Err(format!(
                        "Claude refused to list its models: {}",
                        message["response"]["error"]
                    ));
                }
                let reply: InitializeResponse =
                    serde_json::from_value(message["response"]["response"].clone())
                        .map_err(|error| format!("invalid initialize response: {error}"))?;
                return Ok(reply);
            }
        }
        Err("Claude exited before listing its models".to_owned())
    })
    .await
    .map_err(|_| "Claude took too long to list its models".to_owned())??;
    // Closing its input ends it, and it cleans up after itself.
    drop(input);
    if timeout(PROBE_TIMEOUT, child.wait()).await.is_err() {
        let _ = child.kill().await;
    }
    Ok(decode(&reply))
}

/// The version the CLI reports (`claude --version`, as in "2.1.326 (Claude Code)"), with the session's environment.
pub async fn cli_version(options: &SessionOptions) -> Result<String, String> {
    let output = timeout(
        PROBE_TIMEOUT,
        tokio::process::Command::new(&options.program)
            .arg("--version")
            .env_clear()
            .envs(&options.environment)
            .current_dir(&options.working_directory)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .output(),
    )
    .await
    .map_err(|_| "Claude took too long to report its version".to_owned())?
    .map_err(|error| format!("couldn't ask Claude for its version: {error}"))?;
    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .next()
        .map(str::to_owned)
        .filter(|_| output.status.success())
        .ok_or_else(|| format!("Claude's --version exited with {}", output.status))
}

/// Whether two model IDs name the same model: equal, or equal but for a snapshot date at the end. The API answers a
/// request for an undated ID with its dated snapshot (`claude-sonnet-4-5` with `claude-sonnet-4-5-20250929`).
pub fn same_model(a: &str, b: &str) -> bool {
    a == b || undated(a) == undated(b)
}

/// `id` without a snapshot date at its end (`-20250929`).
fn undated(id: &str) -> &str {
    match id.rsplit_once('-') {
        Some((base, date)) if date.len() == 8 && date.bytes().all(|b| b.is_ascii_digit()) => base,
        _ => id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reply(models: Value) -> InitializeResponse {
        serde_json::from_value(
            json!({"commands": [], "models": models, "claude_code_version": "2.1.326"}),
        )
        .unwrap()
    }

    #[test]
    fn the_cli_2_1_326_reply_decodes_to_its_aliases_with_what_they_resolve_to() {
        let list = decode(&reply(json!([
            {"value": "default", "resolvedModel": "claude-opus-5-5", "displayName": "Default (recommended)", "supportsEffort": true, "supportedEffortLevels": ["low", "medium", "high", "xhigh", "max"]},
            {"value": "opus", "resolvedModel": "claude-opus-5-5", "displayName": "Opus", "description": "For complex work", "supportsEffort": true, "supportedEffortLevels": ["low", "medium", "high", "xhigh", "max"]},
            {"value": "haiku", "resolvedModel": "claude-haiku-5-5", "displayName": "Haiku", "supportsEffort": false, "supportedEffortLevels": ["low"]},
        ])));
        assert_eq!(list.cli_version.as_deref(), Some("2.1.326"));
        assert_eq!(
            list.models
                .iter()
                .map(|m| m.value.as_str())
                .collect::<Vec<_>>(),
            ["default", "opus", "haiku"]
        );
        let opus = &list.models[1];
        assert_eq!(opus.resolved_model.as_deref(), Some("claude-opus-5-5"));
        assert_eq!(opus.display_name, "Opus");
        assert_eq!(opus.description.as_deref(), Some("For complex work"));
        assert_eq!(
            opus.effort_levels,
            ["low", "medium", "high", "xhigh", "max"]
        );
        assert!(
            list.models[2].effort_levels.is_empty(),
            "no effort when it says it takes none"
        );
    }

    /// A real login's list (recorded with CLI 2.1.312): the aliases, then full IDs of other models, older ones included.
    #[test]
    fn a_recorded_session_lists_the_aliases_then_full_model_ids() {
        let line = include_str!("../tests/fixtures/claude-2.1.312/basic.jsonl")
            .lines()
            .nth(1)
            .unwrap();
        let message: Value = serde_json::from_str(line).unwrap();
        let reply: InitializeResponse =
            serde_json::from_value(message["msg"]["response"]["response"].clone()).unwrap();
        let list = decode(&reply);
        assert_eq!(list.cli_version.as_deref(), Some("2.1.312"));
        let values: Vec<_> = list.models.iter().map(|m| m.value.as_str()).collect();
        assert_eq!(
            &values[..5],
            ["default", "opus", "sonnet", "fable", "haiku"]
        );
        assert!(values.contains(&"claude-opus-4-8"), "{values:?}");
        let sonnet = &list.models[2];
        assert_eq!(
            (
                sonnet.resolved_model.as_deref(),
                sonnet.display_name.as_str()
            ),
            (Some("claude-sonnet-5-5"), "Sonnet 5.5")
        );
    }

    fn option(value: &str, resolved: &str, name: &str) -> ModelOption {
        ModelOption {
            value: value.into(),
            resolved_model: Some(resolved.into()),
            display_name: name.into(),
            description: None,
            effort_levels: vec![],
        }
    }

    fn tiers(opus: &str) -> ModelList {
        ModelList {
            cli_version: Some("2.1.326".into()),
            models: vec![
                option("default", opus, "Default (recommended)"),
                option("opus", opus, "Opus 5.5"),
                option("sonnet", "claude-sonnet-5-5", "Sonnet 5.5"),
                option("claude-opus-4-8", "claude-opus-4-8", "Opus 4.8"),
                option("claude-sonnet-4-5", "claude-sonnet-4-5", "Sonnet 4.5"),
            ],
        }
    }

    #[test]
    fn a_reply_is_expected_from_what_the_setting_resolves_to_under_the_profiles_tiers() {
        let list = tiers("claude-opus-5-5");
        assert!(list.expects("opus", "claude-opus-5-5"));
        assert!(!list.expects("opus", "claude-sonnet-5-5"), "a fallback");
        assert!(list.expects("claude-opus-4-8", "claude-opus-4-8"));
        assert!(!list.expects("claude-opus-4-8", "claude-opus-5-5"));
        // Asked for an undated ID, the API answers with its dated snapshot.
        assert!(list.expects("claude-sonnet-4-5", "claude-sonnet-4-5-20250929"));
        assert!(!list.expects("claude-sonnet-4-5", "claude-sonnet-5-5"));
        // The profile pins the Opus tier to an older model: a reply from it is what the agent runs.
        let pinned = tiers("claude-opus-4-8");
        assert!(pinned.expects("opus", "claude-opus-4-8"));
        assert!(!pinned.expects("opus", "claude-opus-5-5"));
    }

    #[test]
    fn opusplan_expects_its_opus_and_its_sonnet_and_an_unknown_alias_expects_anything() {
        let list = tiers("claude-opus-5-5");
        assert!(list.expects("opusplan", "claude-opus-5-5"));
        assert!(list.expects("opusplan", "claude-sonnet-5-5"));
        assert!(!list.expects("opusplan", "claude-haiku-5-5"));
        assert!(
            list.expects("some-future-alias", "claude-haiku-5-5"),
            "no guess"
        );
    }

    #[test]
    fn a_model_is_named_as_the_cli_names_it_a_dated_snapshot_included() {
        let list = tiers("claude-opus-5-5");
        assert_eq!(
            list.name_of("claude-opus-5-5"),
            Some("Opus 5.5"),
            "not Claude Code's default"
        );
        assert_eq!(list.name_of("claude-opus-4-8"), Some("Opus 4.8"));
        assert_eq!(
            list.name_of("claude-sonnet-4-5-20250929"),
            Some("Sonnet 4.5"),
            "a dated snapshot is its undated model"
        );
        assert_eq!(list.name_of("claude-haiku-4-5-20251001"), None);
        assert!(same_model(
            "claude-sonnet-4-5",
            "claude-sonnet-4-5-20250929"
        ));
        assert!(!same_model("claude-opus-4-8", "claude-opus-4-7"));
    }

    /// A subscription's list, as Claude Code 2.1.312 gave it: its aliases, then other models' full IDs.
    fn subscription() -> ModelList {
        ModelList {
            cli_version: Some("2.1.312".into()),
            models: vec![
                option("default", "claude-opus-5-5", "Default (recommended)"),
                option("opus", "claude-opus-5-5", "Opus 5.5"),
                option("sonnet", "claude-sonnet-5-5", "Sonnet 5.5"),
                option("haiku", "claude-haiku-5-5", "Haiku 5.5"),
                option(
                    "claude-haiku-4-5-20251001",
                    "claude-haiku-4-5-20251001",
                    "Haiku 4.5",
                ),
                option("claude-opus-4-8", "claude-opus-4-8", "Opus 4.8"),
                option("claude-sonnet-4-6", "claude-sonnet-4-6", "Sonnet 4.6"),
            ],
        }
    }

    #[test]
    fn models_read_newest_generation_first_then_by_tier_then_the_rest_by_id() {
        let mut ids = vec![
            "claude-haiku-4-5-20251001",
            "zeta-model",
            "claude-opus-5",
            "claude-sonnet-5-5",
            "claude-opus-5-2",
            "claude-fable-5-5",
            "claude-mystery-6-0",
            "claude-opus-4-8",
            "claude-opus",
            "alpha-model",
            "claude-haiku-5-5",
            "claude-opus-5-5",
        ];
        ids.sort_by(|a, b| order(a, b));
        assert_eq!(
            ids,
            [
                "claude-fable-5-5",
                "claude-opus-5-5",
                "claude-sonnet-5-5",
                "claude-haiku-5-5",
                "claude-opus-5-2",
                "claude-opus-5",
                "claude-opus-4-8",
                "claude-haiku-4-5-20251001",
                // Outside the scheme, alphabetically: another naming, an unknown family, no number at all.
                "alpha-model",
                "claude-mystery-6-0",
                "claude-opus",
                "zeta-model",
            ]
        );
        assert!(precedes("claude-opus-5-5", "claude-opus-4-8"));
    }

    #[test]
    fn an_agent_is_offered_every_model_but_claude_codes_default() {
        let choices = subscription().agent_choices();
        assert_eq!(
            choices.iter().map(|c| c.value.as_str()).collect::<Vec<_>>(),
            [
                "opus",
                "sonnet",
                "haiku",
                "claude-opus-4-8",
                "claude-sonnet-4-6",
                "claude-haiku-4-5-20251001"
            ]
        );
        assert!(choices[0].is_alias() && !choices[3].is_alias());
    }

    #[test]
    fn a_tier_is_offered_each_full_id_once_named_by_its_model() {
        let ids = subscription().tier_choices();
        assert_eq!(
            ids.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            [
                "claude-opus-5-5",
                "claude-sonnet-5-5",
                "claude-haiku-5-5",
                "claude-opus-4-8",
                "claude-sonnet-4-6",
                "claude-haiku-4-5-20251001",
            ]
        );
        assert_eq!(
            ids[0].name, "Opus 5.5",
            "named by its alias, not by `default`"
        );
    }

    #[test]
    fn an_entry_without_a_value_is_skipped_and_the_rest_are_optional() {
        let list = decode(&reply(
            json!([{"displayName": "Nameless"}, {"value": "fable"}, {"value": "claude-next", "displayName": ""}]),
        ));
        assert_eq!(list.models.len(), 2);
        assert_eq!(
            list.models[1].display_name, "claude-next",
            "an empty name is no name"
        );
        let fable = &list.models[0];
        assert_eq!(
            fable.display_name, "fable",
            "the value stands in for a missing display name"
        );
        assert_eq!(fable.resolved_model, None);
        assert!(fable.effort_levels.is_empty());
    }
}

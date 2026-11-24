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
    /// Whether an agent may be set to `model`: what a row of its [`sections`](Self::sections) stores, a model ID dated or
    /// not, so never what [`IGNORED`] names.
    pub fn offers(&self, model: &str) -> bool {
        let sections = self.sections();
        sections
            .aliases
            .iter()
            .chain(&sections.model_ids)
            .any(|row| row.value == model || same_model(&row.value, model))
    }

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

    /// The effort levels an agent set to `setting` takes, lowest first: its entry's (an alias, or a full ID with or
    /// without its date), else the entry of the model the setting resolves to; empty when it takes none. None when the
    /// list doesn't know the setting, so nothing is claimed about it.
    pub fn effort_levels(&self, setting: &str) -> Option<&[String]> {
        let resolved = self.resolve(setting).unwrap_or(setting);
        self.models
            .iter()
            .find(|model| model.value == setting || same_model(&model.value, setting))
            .or_else(|| {
                self.models
                    .iter()
                    .find(|model| same_model(model.model_id(), resolved))
            })
            .map(|model| model.effort_levels.as_slice())
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

    /// The list as every model field shows it: the tiers Claude Code's aliases name, then every model by its version.
    ///
    /// - **Aliases**, picking one stores the alias, which follows Claude Code to newer models: those Devora doesn't know as
    ///   a tier first, alphabetically (a new tier is shown, not hidden), then Fable, Opus, Sonnet and Haiku. Each reads
    ///   "Opus (currently 5.5)": the tier, then what it resolves to today, without repeating the tier's word ("Haiku
    ///   (currently Sonnet 5.5)" when the profile points Haiku at Sonnet; the ID when Claude Code gives no name).
    /// - **Model IDs**, picking one pins that version: each model once, the ones the aliases resolve to included, by major
    ///   version (newest first), then tier, then version (newest first); an ID that doesn't read as Claude's model naming
    ///   goes last, alphabetically.
    ///
    /// Neither shows what [`IGNORED`] names.
    pub fn sections(&self) -> ModelSections {
        let shown = self
            .models
            .iter()
            .filter(|model| !IGNORED.contains(&model.value.as_str()));
        let mut aliases: Vec<&ModelOption> =
            shown.clone().filter(|model| model.is_alias()).collect();
        aliases.sort_by(|a, b| alias_order(&a.value, &b.value));
        let mut model_ids: Vec<ModelRow> = Vec::new();
        // A model's own entry names it best; an alias's name stands in for a model Claude Code lists only by its alias.
        for model in shown
            .clone()
            .filter(|model| !model.is_alias())
            .chain(aliases.iter().copied())
        {
            let id = model.model_id();
            if model_ids.iter().any(|row| row.model_id == id) {
                continue;
            }
            let name = model_name(model);
            model_ids.push(ModelRow {
                value: id.to_owned(),
                label: name.unwrap_or(id).to_owned(),
                model_id: id.to_owned(),
            });
        }
        model_ids.sort_by(|a, b| model_order(&a.model_id, &b.model_id));
        ModelSections {
            aliases: aliases
                .into_iter()
                .map(|model| ModelRow {
                    value: model.value.clone(),
                    label: alias_label(model),
                    model_id: model.model_id().to_owned(),
                })
                .collect(),
            model_ids,
        }
    }

    /// The aliases Claude Code lists that Devora doesn't know as a tier (and doesn't ignore): a new tier, shown first.
    pub fn unknown_aliases(&self) -> Vec<&str> {
        self.models
            .iter()
            .filter(|model| model.is_alias() && !IGNORED.contains(&model.value.as_str()))
            .map(|model| model.value.as_str())
            .filter(|alias| !Generation::TIERS.contains(alias))
            .collect()
    }
}

/// What Claude Code lists that no model field shows: its own `default`, a choice rather than a model, and `opusplan`,
/// which Devora's agents don't offer (`ccc`'s Default model setting still stores it).
pub const IGNORED: [&str; 2] = ["default", "opusplan"];

/// One row of a model field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelRow {
    /// What picking it stores: an alias for a tier, a full ID for a model.
    pub value: String,
    pub label: String,
    /// The model it runs today, a full ID.
    pub model_id: String,
}

/// A model list as every model field shows it ([`ModelList::sections`]).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ModelSections {
    pub aliases: Vec<ModelRow>,
    pub model_ids: Vec<ModelRow>,
}

impl ModelSections {
    /// The rows that match what's `typed` into a model field, so every field narrows alike: a tier by its own name ("son"
    /// finds Sonnet, not "Haiku (currently Sonnet 5.5)"), a model by its name or ID, ignoring case. A field holding exactly
    /// a value one row stores (as the New Agent field opens with `opus`) has typed nothing yet, so every row shows.
    pub fn narrowed(&self, typed: &str) -> ModelSections {
        let typed = typed.trim().to_lowercase();
        if typed.is_empty()
            || self
                .aliases
                .iter()
                .chain(&self.model_ids)
                .any(|row| row.value.to_lowercase() == typed)
        {
            return self.clone();
        }
        let has = |text: &str| text.to_lowercase().contains(&typed);
        ModelSections {
            aliases: self
                .aliases
                .iter()
                .filter(|row| has(&row.value))
                .cloned()
                .collect(),
            model_ids: self
                .model_ids
                .iter()
                .filter(|row| has(&row.label) || has(&row.model_id))
                .cloned()
                .collect(),
        }
    }
}

/// The name Claude Code gives `model`, unless it's only the tier's word (an API-key login names `opus` just "Opus").
fn model_name(model: &ModelOption) -> Option<&str> {
    let name = model.display_name.trim();
    (!name.is_empty() && !name.eq_ignore_ascii_case(&model.value)).then_some(name)
}

/// "Opus (currently 5.5)": the alias as a tier's name, then what it resolves to, without the tier's word again.
fn alias_label(model: &ModelOption) -> String {
    let tier = capitalized(&model.value);
    let current = match model_name(model) {
        Some(name) => name.strip_prefix(&format!("{tier} ")).unwrap_or(name),
        None => model.model_id(),
    };
    format!("{tier} (currently {current})")
}

fn capitalized(word: &str) -> String {
    let mut chars = word.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

/// Aliases Devora doesn't know as a tier first, alphabetically, then the tiers in their order.
fn alias_order(first: &str, second: &str) -> std::cmp::Ordering {
    let tier = |alias: &str| Generation::TIERS.iter().position(|tier| *tier == alias);
    match (tier(first), tier(second)) {
        (Some(a), Some(b)) => a.cmp(&b),
        (Some(_), None) => std::cmp::Ordering::Greater,
        (None, Some(_)) => std::cmp::Ordering::Less,
        (None, None) => first.cmp(second),
    }
}

/// Claude's models by major version (newest first), then tier, then version (newest first); every other ID after them,
/// alphabetically.
fn model_order(first: &str, second: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    match (Generation::of(first), Generation::of(second)) {
        (Some(a), Some(b)) => b
            .version
            .0
            .cmp(&a.version.0)
            .then(a.tier.cmp(&b.tier))
            .then(b.version.cmp(&a.version))
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
    fn the_effort_levels_a_setting_takes_are_its_entrys_or_its_resolved_models() {
        let levels = |levels: &[&str]| levels.iter().map(|l| l.to_string()).collect::<Vec<_>>();
        let mut list = tiers("claude-opus-5-5");
        for model in &mut list.models {
            model.effort_levels = match model.value.as_str() {
                "claude-sonnet-4-5" => vec![],
                "claude-opus-4-8" => levels(&["low", "medium", "high", "max"]),
                _ => levels(&["low", "medium", "high", "xhigh", "max"]),
            };
        }
        assert_eq!(
            list.effort_levels("opus").unwrap(),
            levels(&["low", "medium", "high", "xhigh", "max"])
        );
        assert_eq!(
            list.effort_levels("claude-opus-4-8").unwrap(),
            levels(&["low", "medium", "high", "max"])
        );
        assert_eq!(
            list.effort_levels("claude-opus-5-5").unwrap(),
            levels(&["low", "medium", "high", "xhigh", "max"]),
            "a full ID an alias resolves to"
        );
        assert!(
            list.effort_levels("claude-sonnet-4-5-20250929")
                .unwrap()
                .is_empty(),
            "a dated ID, of a model that takes none"
        );
        assert_eq!(
            list.effort_levels("claude-made-up-9"),
            None,
            "nothing claimed"
        );
    }

    #[test]
    fn an_agent_may_be_set_only_to_what_the_model_fields_offer() {
        let mut list = tiers("claude-opus-5-5");
        list.models
            .push(option("opusplan", "claude-opus-5-5", "Opus Plan Mode"));
        for offered in [
            "opus",
            "claude-opus-5-5",
            "claude-opus-4-8",
            "claude-sonnet-4-5-20250929",
        ] {
            assert!(list.offers(offered), "{offered}");
        }
        for refused in ["default", "opusplan", "haiku", "claude-made-up-9"] {
            assert!(!list.offers(refused), "{refused}");
        }
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
    fn the_aliases_read_as_tiers_new_ones_first_and_the_ignored_values_appear_nowhere() {
        let list = ModelList {
            cli_version: Some("2.1.329".into()),
            models: vec![
                option("default", "claude-opus-5-5", "Default (recommended)"),
                option("opusplan", "claude-opus-5-5", "Opus Plan"),
                option("haiku", "claude-sonnet-5-5", "Sonnet 5.5"),
                option("sonnet", "claude-sonnet-5-5", "Sonnet 5.5"),
                option("opus", "claude-opus-5-5", "Opus"),
                option("fable", "claude-fable-5-2", "Fable 5.2"),
                option("zest", "claude-zest-1-9", "Zest 1.9"),
                option("ace", "claude-ace-2-2", "Ace 2.2"),
            ],
        };
        let sections = list.sections();
        let labels: Vec<&str> = sections
            .aliases
            .iter()
            .map(|row| row.label.as_str())
            .collect();
        assert_eq!(
            labels,
            [
                "Ace (currently 2.2)",
                "Zest (currently 1.9)",
                "Fable (currently 5.2)",
                "Opus (currently claude-opus-5-5)",
                "Sonnet (currently 5.5)",
                "Haiku (currently Sonnet 5.5)",
            ],
            "new tiers first; a name that's only the tier's word shows the ID"
        );
        assert_eq!(
            sections.aliases[5].value, "haiku",
            "picking a tier stores its alias"
        );
        let all: Vec<&str> = sections
            .aliases
            .iter()
            .chain(&sections.model_ids)
            .map(|row| row.value.as_str())
            .collect();
        assert!(
            !all.contains(&"default") && !all.contains(&"opusplan"),
            "{all:?}"
        );
        assert_eq!(list.unknown_aliases(), ["zest", "ace"]);
    }

    #[test]
    fn typing_narrows_a_tier_by_its_own_name_and_a_model_by_its_name_or_id() {
        let row = |value: &str, label: &str, id: &str| ModelRow {
            value: value.into(),
            label: label.into(),
            model_id: id.into(),
        };
        let sections = ModelSections {
            aliases: vec![
                row("sonnet", "Sonnet (currently 5.5)", "claude-sonnet-5-5"),
                row("haiku", "Haiku (currently Sonnet 5.5)", "claude-sonnet-5-5"),
            ],
            model_ids: vec![
                row("claude-sonnet-5-5", "Sonnet 5.5", "claude-sonnet-5-5"),
                row("claude-opus-4-8", "Opus 4.8", "claude-opus-4-8"),
            ],
        };
        let values = |narrowed: ModelSections| -> Vec<String> {
            narrowed
                .aliases
                .into_iter()
                .chain(narrowed.model_ids)
                .map(|row| row.value)
                .collect()
        };
        assert_eq!(
            values(sections.narrowed("SON")),
            ["sonnet", "claude-sonnet-5-5"],
            "not Haiku, whose tier is another's name"
        );
        assert_eq!(
            values(sections.narrowed("4.8")),
            ["claude-opus-4-8"],
            "by name"
        );
        assert_eq!(
            values(sections.narrowed("opus-4")),
            ["claude-opus-4-8"],
            "by ID"
        );
        assert_eq!(
            values(sections.narrowed("haiku")).len(),
            4,
            "a picked value shows every row"
        );
        assert!(values(sections.narrowed("nothing-like-it")).is_empty());
    }

    #[test]
    fn every_model_appears_once_by_its_version_by_major_then_tier_then_newest() {
        let mut list = subscription();
        list.models.extend([
            option("fable", "claude-fable-5-2", "Fable 5.2"),
            option("claude-fable-5-1", "claude-fable-5-1", "Fable 5.1"),
            option("claude-opus-5", "claude-opus-5", "Opus 5"),
            option("claude-sonnet-5", "claude-sonnet-5", "Sonnet 5"),
            option("claude-opus-4-7", "claude-opus-4-7", "Opus 4.7"),
            option("zeta-model", "zeta-model", "Zeta"),
            option("alpha-model", "alpha-model", ""),
        ]);
        let rows = list.sections().model_ids;
        let labels: Vec<&str> = rows.iter().map(|row| row.label.as_str()).collect();
        assert_eq!(
            labels,
            [
                "Fable 5.2",
                "Fable 5.1",
                "Opus 5.5",
                "Opus 5",
                "Sonnet 5.5",
                "Sonnet 5",
                "Haiku 5.5",
                "Opus 4.8",
                "Opus 4.7",
                "Sonnet 4.6",
                "Haiku 4.5",
                "alpha-model",
                "Zeta",
            ],
            "the current models included, once; what doesn't read as Claude's naming last"
        );
        assert_eq!(
            rows[2].value, "claude-opus-5-5",
            "picking a model pins its version"
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

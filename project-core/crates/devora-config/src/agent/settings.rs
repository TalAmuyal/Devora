//! Agent settings at one editable scope, resolved by the creation resolver.
//! Input names are literal map keys, including dots, rather than config paths.
use super::{resolve, sections, Scope, DEFAULT_COMPACT_THRESHOLD, NAMED};
use crate::config::{read_json_file, write_config_setting_in, write_pretty_json};
use serde_json::{Map, Value};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Default,
    Global,
    Profile,
}

impl From<Scope> for Source {
    fn from(scope: Scope) -> Self {
        match scope {
            Scope::Global => Self::Global,
            Scope::Profile => Self::Profile,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub key: String,
    /// Absent inherits. JSON null is an explicit legacy clearing override.
    pub stored: Option<Value>,
    pub resolved: Option<Value>,
    pub source: Source,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub fields: Vec<Entry>,
    pub inputs: Vec<Entry>,
    /// A legacy null map clears all lower-scope free-form inputs.
    pub inputs_cleared: bool,
}

pub fn read(global: &Path, profile: Option<&Path>) -> Result<Settings, String> {
    let _write = crate::config::lock_config_writes();
    read_unlocked(global, profile)
}

fn read_unlocked(global: &Path, profile: Option<&Path>) -> Result<Settings, String> {
    let sections = sections(global, profile)?;
    let config = resolve(&sections)?;
    let own_scope = if profile.is_some() {
        Scope::Profile
    } else {
        Scope::Global
    };
    let own = sections
        .iter()
        .find(|(_, scope, _)| *scope == own_scope)
        .map(|(_, _, section)| section);
    let mut fields = Vec::new();
    for key in [
        "compact-threshold",
        "template-file",
        "role",
        "user-details",
        "custom-sections",
    ] {
        let definition = sections.iter().rev().find(|(_, _, section)| {
            section.contains_key(key) && (key != "template-file" || section[key].is_string())
        });
        let source = definition
            .map(|(_, scope, _)| Source::from(*scope))
            .unwrap_or(Source::Default);
        let resolved = match key {
            "compact-threshold" => Some(Value::from(
                config
                    .compact_threshold
                    .unwrap_or(DEFAULT_COMPACT_THRESHOLD),
            )),
            "template-file" => config
                .template_file
                .as_ref()
                .map(|file| Value::from(file.to_string_lossy().into_owned())),
            _ => {
                let placeholder = NAMED
                    .iter()
                    .find(|(name, _)| *name == key)
                    .expect("named setting")
                    .1;
                config
                    .inputs
                    .get(placeholder)
                    .map(|(text, _)| Value::from(text.clone()))
            }
        };
        // A null threshold explicitly selects the built-in default.
        let source = if key == "compact-threshold" && config.compact_threshold.is_none() {
            Source::Default
        } else {
            source
        };
        fields.push(Entry {
            key: key.into(),
            stored: own.and_then(|section| section.get(key)).cloned(),
            resolved,
            source,
        });
    }
    let mut keys = BTreeSet::new();
    for (_, _, section) in &sections {
        if let Some(Value::Object(inputs)) = section.get("inputs") {
            keys.extend(inputs.keys().filter(|key| !reserved(key)).cloned());
        }
    }
    let mut inputs = Vec::new();
    for key in keys {
        let definition = sections.iter().rev().find(|(_, _, section)| {
            section
                .get("inputs")
                .is_some_and(|inputs| inputs.is_null() || inputs.get(&key).is_some())
        });
        inputs.push(Entry {
            stored: own
                .and_then(|section| section.get("inputs"))
                .and_then(|inputs| inputs.get(&key))
                .cloned(),
            resolved: config
                .inputs
                .get(&key)
                .map(|(text, _)| Value::from(text.clone())),
            source: definition
                .map(|(_, scope, _)| Source::from(*scope))
                .unwrap_or(Source::Default),
            key,
        });
    }
    let generic = crate::config::read_config_settings_in(global, profile.and_then(Path::to_str))?;
    let source = sections
        .iter()
        .rev()
        .find(|(_, _, section)| section.get("enter-sends").is_some_and(Value::is_boolean))
        .map(|(_, scope, _)| Source::from(*scope))
        .unwrap_or(Source::Default);
    fields.push(Entry {
        key: "enter-sends".into(),
        stored: generic.stored.get("agent.enter-sends").cloned(),
        resolved: generic.resolved.get("agent.enter-sends").cloned(),
        source,
    });
    Ok(Settings {
        fields,
        inputs,
        inputs_cleared: own
            .and_then(|section| section.get("inputs"))
            .is_some_and(Value::is_null),
    })
}

pub fn write_setting(
    global: &Path,
    profile: Option<&Path>,
    key: &str,
    value: Option<&str>,
) -> Result<(), String> {
    if key == "enter-sends" {
        return write_config_setting_in(
            global,
            profile.and_then(Path::to_str),
            "agent.enter-sends",
            if value.is_some() { "value" } else { "default" },
            value,
        );
    }
    let _write = crate::config::lock_config_writes();
    let value = match (key, value) {
        ("compact-threshold", Some(raw)) => {
            let percent = raw
                .parse::<u8>()
                .ok()
                .filter(|n| (1..=100).contains(n))
                .ok_or("agent.compact-threshold must be a whole percentage from 1 to 100")?;
            Some(Value::from(percent))
        }
        ("template-file", Some(raw)) => {
            if raw.trim().is_empty() {
                return Err("agent.template-file must be a non-empty path".into());
            }
            Some(Value::from(raw))
        }
        ("role" | "user-details" | "custom-sections", Some(raw)) => Some(Value::from(raw)),
        (
            "compact-threshold" | "template-file" | "role" | "user-details" | "custom-sections"
            | "inputs",
            None,
        ) => None,
        _ => return Err(format!("unknown agent setting: {key}")),
    };
    let file = scope_file(global, profile);
    let mut document = document(&file)?;
    let section = agent_section(&mut document)?;
    match value {
        Some(value) => {
            section.insert(key.into(), value);
        }
        None => {
            section.remove(key);
        }
    }
    persist(&file, document)
}

/// Add an input, update its override, rename an existing local entry atomically,
/// or remove an override (Default). A rename cannot replace another input.
pub fn write_input(
    global: &Path,
    profile: Option<&Path>,
    previous_key: Option<&str>,
    key: &str,
    value: Option<&str>,
) -> Result<(), String> {
    let _write = crate::config::lock_config_writes();
    if value.is_none() && previous_key.is_some_and(|previous| previous != key) {
        return Err("removing an override cannot rename another entry".into());
    }
    if value.is_some() {
        validate_key(key)?;
    }
    if reserved(key) || previous_key.is_some_and(reserved) {
        return Err("named placeholders have their own Agent settings fields".into());
    }
    let file = scope_file(global, profile);
    let mut document = document(&file)?;
    let section = agent_section(&mut document)?;
    if section.get("inputs").is_some_and(Value::is_null) {
        return Err("select Default for the cleared inputs map before editing entries".into());
    }
    let inputs = section
        .entry("inputs")
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .ok_or("agent.inputs must be an object")?;
    if value.is_some() && previous_key != Some(key) {
        let effective = read_unlocked(global, profile)?;
        if inputs.contains_key(key)
            || effective
                .inputs
                .iter()
                .any(|entry| entry.key == key && entry.resolved.is_some())
        {
            return Err(format!("agent input already exists: {key}"));
        }
    }
    if let Some(previous) = previous_key {
        if previous != key {
            if !inputs.contains_key(previous) {
                return Err("only an input stored at this scope can be renamed".into());
            }
            inputs.remove(previous);
        }
    }
    match value {
        Some(value) => {
            inputs.insert(key.into(), Value::from(value));
        }
        None => {
            inputs.remove(key);
        }
    }
    if inputs.is_empty() {
        section.remove("inputs");
    }
    persist(&file, document)
}

fn reserved(key: &str) -> bool {
    NAMED.iter().any(|(_, placeholder)| *placeholder == key)
}

pub fn validate_key(key: &str) -> Result<(), String> {
    if !super::valid_placeholder_key(key) {
        return Err("input keys must contain only A–Z, a–z, 0–9, dots and underscores".into());
    }
    if reserved(key) {
        return Err("named placeholders have their own Agent settings fields".into());
    }
    Ok(())
}

fn scope_file(global: &Path, profile: Option<&Path>) -> PathBuf {
    profile
        .map(|path| path.join("config.json"))
        .unwrap_or_else(|| global.to_path_buf())
}

fn document(file: &Path) -> Result<Value, String> {
    let document = if file.exists() {
        read_json_file(file)?
    } else {
        Value::Object(Map::new())
    };
    if !document.is_object() {
        return Err("config is not a JSON object".into());
    }
    Ok(document)
}

fn agent_section(document: &mut Value) -> Result<&mut Map<String, Value>, String> {
    document
        .as_object_mut()
        .expect("validated config")
        .entry("agent")
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .ok_or("agent config must be an object".into())
}

fn persist(file: &Path, mut document: Value) -> Result<(), String> {
    if document
        .get("agent")
        .and_then(Value::as_object)
        .is_some_and(Map::is_empty)
    {
        document
            .as_object_mut()
            .expect("validated config")
            .remove("agent");
    }
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create {}: {e}", parent.display()))?;
    }
    write_pretty_json(file, &document)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;

    fn entry<'a>(entries: &'a [Entry], key: &str) -> &'a Entry {
        entries.iter().find(|entry| entry.key == key).unwrap()
    }

    #[test]
    fn resolves_each_value_and_source_through_the_creation_fold() {
        let tmp = tempfile::tempdir().unwrap();
        let global = tmp.path().join("global.json");
        let profile = tmp.path().join("profile");
        fs::create_dir(&profile).unwrap();
        fs::write(&global, json!({"other":true,"agent":{"compact-threshold":70,"template-file":"global.md","role":"global role","inputs":{"repo.notes":"global","team":"global"}}}).to_string()).unwrap();
        fs::write(profile.join("config.json"), json!({"agent":{"role":null,"user-details":"\n user \n","inputs":{"repo.notes":"profile","team":null}}}).to_string()).unwrap();
        let settings = read(&global, Some(&profile)).unwrap();
        assert_eq!(
            entry(&settings.fields, "compact-threshold").resolved,
            Some(json!(70))
        );
        assert_eq!(
            entry(&settings.fields, "compact-threshold").source,
            Source::Global
        );
        assert_eq!(
            entry(&settings.fields, "template-file").resolved,
            Some(json!(tmp.path().join("global.md").to_string_lossy()))
        );
        assert_eq!(entry(&settings.fields, "role").stored, Some(Value::Null));
        assert_eq!(entry(&settings.fields, "role").resolved, None);
        assert_eq!(entry(&settings.fields, "role").source, Source::Profile);
        assert_eq!(
            entry(&settings.fields, "user-details").resolved,
            Some(json!("\n user \n"))
        );
        assert_eq!(
            entry(&settings.inputs, "repo.notes").resolved,
            Some(json!("profile"))
        );
        assert_eq!(entry(&settings.inputs, "team").stored, Some(Value::Null));
        assert_eq!(entry(&settings.inputs, "team").resolved, None);
        let creation = super::super::read(&global, &profile).unwrap();
        assert_eq!(
            entry(&settings.inputs, "repo.notes")
                .resolved
                .as_ref()
                .unwrap()
                .as_str(),
            Some(creation.inputs["repo.notes"].0.as_str())
        );
        let global_settings = read(&global, None).unwrap();
        assert_eq!(
            entry(&global_settings.fields, "role").resolved,
            Some(json!("global role"))
        );
    }

    #[test]
    fn defaults_and_legacy_nulls_keep_the_existing_meanings() {
        let tmp = tempfile::tempdir().unwrap();
        let global = tmp.path().join("global.json");
        let profile = tmp.path().join("profile");
        fs::create_dir(&profile).unwrap();
        fs::write(&global, json!({"agent":{"compact-threshold":65,"template-file":"global.md","inputs":{"team":"core"}}}).to_string()).unwrap();
        fs::write(
            profile.join("config.json"),
            json!({"agent":{"compact-threshold":null,"template-file":null,"inputs":null}})
                .to_string(),
        )
        .unwrap();
        let settings = read(&global, Some(&profile)).unwrap();
        assert_eq!(
            entry(&settings.fields, "compact-threshold").resolved,
            Some(json!(DEFAULT_COMPACT_THRESHOLD))
        );
        assert_eq!(
            entry(&settings.fields, "compact-threshold").source,
            Source::Default
        );
        assert_eq!(
            entry(&settings.fields, "template-file").source,
            Source::Global
        );
        assert!(settings.inputs_cleared);
        assert_eq!(entry(&settings.inputs, "team").resolved, None);
        assert!(write_input(&global, Some(&profile), None, "new", Some("value")).is_err());
        write_setting(&global, Some(&profile), "inputs", None).unwrap();
        assert_eq!(
            entry(&read(&global, Some(&profile)).unwrap().inputs, "team").resolved,
            Some(json!("core"))
        );
    }

    #[test]
    fn writes_typed_percentages_and_preserves_empty_multiline_text_and_siblings() {
        let tmp = tempfile::tempdir().unwrap();
        let global = tmp.path().join("global.json");
        fs::write(
            &global,
            json!({"claude":{"model":"custom"},"agent":{"inputs":{"team":"core"}}}).to_string(),
        )
        .unwrap();
        for value in ["1", "100"] {
            write_setting(&global, None, "compact-threshold", Some(value)).unwrap();
            assert!(read_json_file(&global).unwrap()["agent"]["compact-threshold"].is_number());
        }
        for value in ["0", "101", "1.5", "-1", "junk", ""] {
            let before = fs::read(&global).unwrap();
            assert!(write_setting(&global, None, "compact-threshold", Some(value)).is_err());
            assert_eq!(fs::read(&global).unwrap(), before);
        }
        for value in ["", "\n role \n"] {
            write_setting(&global, None, "role", Some(value)).unwrap();
            assert_eq!(
                entry(&read(&global, None).unwrap().fields, "role").stored,
                Some(json!(value))
            );
        }
        write_setting(&global, None, "role", None).unwrap();
        let saved = read_json_file(&global).unwrap();
        assert_eq!(saved["claude"]["model"], "custom");
        assert_eq!(saved["agent"]["inputs"]["team"], "core");
        assert!(saved["agent"].get("role").is_none());
    }

    #[test]
    fn map_keys_are_literal_and_renames_are_validated_before_the_single_write() {
        let tmp = tempfile::tempdir().unwrap();
        let global = tmp.path().join("global.json");
        write_input(&global, None, None, "repo.notes", Some("\n notes \n")).unwrap();
        assert_eq!(
            read_json_file(&global).unwrap()["agent"]["inputs"]["repo.notes"],
            "\n notes \n"
        );
        write_input(
            &global,
            None,
            Some("repo.notes"),
            "team_notes",
            Some("renamed"),
        )
        .unwrap();
        let saved = read_json_file(&global).unwrap();
        assert!(saved["agent"]["inputs"].get("repo.notes").is_none());
        assert_eq!(saved["agent"]["inputs"]["team_notes"], "renamed");
        for key in [
            "",
            "bad-key",
            "bad key",
            "é",
            "agent.role",
            "user.details",
            "custom.sections",
            "team_notes",
        ] {
            let before = fs::read(&global).unwrap();
            assert!(write_input(&global, None, None, key, Some("bad")).is_err());
            assert_eq!(fs::read(&global).unwrap(), before);
        }
        write_input(&global, None, None, "other", Some("other")).unwrap();
        let before = fs::read(&global).unwrap();
        assert!(write_input(
            &global,
            None,
            Some("team_notes"),
            "other",
            Some("collision")
        )
        .is_err());
        assert_eq!(fs::read(&global).unwrap(), before);
        write_input(&global, None, Some("team_notes"), "team_notes", None).unwrap();
        assert_eq!(
            read_json_file(&global).unwrap()["agent"]["inputs"],
            json!({"other":"other"})
        );
    }

    #[test]
    fn a_profile_override_can_be_removed_without_removing_the_global_entry() {
        let tmp = tempfile::tempdir().unwrap();
        let global = tmp.path().join("global.json");
        let profile = tmp.path().join("profile");
        write_input(&global, None, None, "team", Some("global")).unwrap();
        write_input(&global, Some(&profile), Some("team"), "team", Some("")).unwrap();
        assert_eq!(
            entry(&read(&global, Some(&profile)).unwrap().inputs, "team").resolved,
            Some(json!(""))
        );
        write_input(&global, Some(&profile), Some("team"), "team", None).unwrap();
        assert_eq!(
            entry(&read(&global, Some(&profile)).unwrap().inputs, "team").resolved,
            Some(json!("global"))
        );
        assert_eq!(
            read_json_file(&global).unwrap()["agent"]["inputs"]["team"],
            "global"
        );
    }

    #[test]
    fn an_unreadable_config_path_is_left_untouched() {
        let tmp = tempfile::tempdir().unwrap();
        let global = tmp.path().join("global.json");
        fs::create_dir(&global).unwrap();
        assert!(write_setting(&global, None, "role", Some("role")).is_err());
        assert!(global.is_dir());
    }

    #[test]
    fn agent_generic_and_claude_mutations_preserve_each_other_across_threads() {
        let tmp = tempfile::tempdir().unwrap();
        let global = tmp.path().join("config.json");
        std::fs::write(&global, r#"{"profiles":[]}"#).unwrap();
        let barrier = std::sync::Barrier::new(3);
        std::thread::scope(|threads| {
            for kind in 0..3 {
                let global = &global;
                let barrier = &barrier;
                threads.spawn(move || {
                    for round in 0..20 {
                        barrier.wait();
                        let value = round.to_string();
                        match kind {
                            0 => write_input(
                                global,
                                None,
                                Some("repo.notes"),
                                "repo.notes",
                                Some(&value),
                            )
                            .unwrap(),
                            1 => crate::config::write_config_setting_in(
                                global,
                                None,
                                "prepare-command",
                                "value",
                                Some(&value),
                            )
                            .unwrap(),
                            _ => crate::config::write_claude_setting_in(
                                global,
                                None,
                                "opus-model",
                                "value",
                                Some(&value),
                            )
                            .unwrap(),
                        }
                    }
                });
            }
        });
        let document = read_json_file(&global).unwrap();
        assert_eq!(document["agent"]["inputs"]["repo.notes"], "19");
        assert_eq!(document["prepare-command"], "19");
        assert_eq!(document["claude"]["opus-model"], "19");
        assert_eq!(document["profiles"], serde_json::json!([]));
    }
}

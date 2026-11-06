//! Agent creation inputs, resolved per key from global then profile config.
//! Relative template-file paths are anchored at the config file defining them.
//!
//! Inputs are keyed by template placeholder. `role`, `user-details` and
//! `custom-sections` fill `agent.role`, `user.details` and `custom.sections`, and
//! win over the same placeholder in `inputs`; `inputs` holds any other placeholder.
use crate::config::{expand_tilde, read_json_file, write_pretty_json};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The named keys of the `agent` section and the placeholders they fill.
const NAMED: [(&str, &str); 3] = [
    ("role", "agent.role"),
    ("user-details", "user.details"),
    ("custom-sections", "custom.sections"),
];

/// Which config file an input came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Global,
    Profile,
}

#[derive(Debug, Clone, Default)]
pub struct AgentConfig {
    pub template_file: Option<PathBuf>,
    /// Template inputs by placeholder, each with the config that set it.
    pub inputs: BTreeMap<String, (String, Scope)>,
}

pub fn read(global: &Path, profile: &Path) -> Result<AgentConfig, String> {
    let mut config = AgentConfig::default();
    for (file, scope) in [
        (global.to_path_buf(), Scope::Global),
        (profile.join("config.json"), Scope::Profile),
    ] {
        let Some(section) = section(&file)? else {
            continue;
        };
        match section.get("inputs") {
            None => {}
            Some(Value::Null) => config
                .inputs
                .retain(|key, _| NAMED.iter().any(|(_, placeholder)| placeholder == key)),
            Some(Value::Object(inputs)) => {
                for (key, value) in inputs {
                    if NAMED.iter().any(|(_, placeholder)| placeholder == key) {
                        continue;
                    }
                    set(&mut config.inputs, key, value, scope)
                        .map_err(|e| format!("agent.inputs.{key} {e}"))?;
                }
            }
            Some(_) => return Err("agent.inputs must be an object".into()),
        }
        for (name, placeholder) in NAMED {
            if let Some(value) = section.get(name) {
                set(&mut config.inputs, placeholder, value, scope)
                    .map_err(|e| format!("agent.{name} {e}"))?;
            }
        }
        match section.get("template-file") {
            None | Some(Value::Null) => {}
            Some(Value::String(text)) => {
                let path = PathBuf::from(expand_tilde(text)?);
                config.template_file = Some(if path.is_absolute() {
                    path
                } else {
                    file.parent().unwrap_or(Path::new(".")).join(path)
                });
            }
            Some(_) => return Err("agent.template-file must be a path string".into()),
        }
    }
    Ok(config)
}

/// Saves one template input to a config file, as editing it in the Settings Hub would.
pub fn write(file: &Path, placeholder: &str, value: &str) -> Result<(), String> {
    let mut document = if file.exists() {
        read_json_file(file)?
    } else {
        Value::Object(Map::new())
    };
    let root = document
        .as_object_mut()
        .ok_or("config is not a JSON object")?;
    let section = root
        .entry("agent")
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .ok_or("agent config must be an object")?;
    let value = Value::String(value.into());
    match NAMED.iter().find(|(_, key)| *key == placeholder) {
        Some((name, _)) => {
            section.insert((*name).into(), value);
        }
        None => {
            section
                .entry("inputs")
                .or_insert_with(|| Value::Object(Map::new()))
                .as_object_mut()
                .ok_or("agent.inputs must be an object")?
                .insert(placeholder.into(), value);
        }
    }
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create {}: {e}", parent.display()))?;
    }
    write_pretty_json(file, &document)
}

fn section(file: &Path) -> Result<Option<Map<String, Value>>, String> {
    if !file.exists() {
        return Ok(None);
    }
    match read_json_file(file)?.get("agent") {
        None => Ok(None),
        Some(Value::Object(section)) => Ok(Some(section.clone())),
        Some(_) => Err("agent config must be an object".into()),
    }
}

/// A string sets the input and a JSON `null` clears what a lower scope set.
fn set(
    inputs: &mut BTreeMap<String, (String, Scope)>,
    key: &str,
    value: &Value,
    scope: Scope,
) -> Result<(), String> {
    match value {
        Value::Null => {
            inputs.remove(key);
        }
        Value::String(text) => {
            inputs.insert(key.into(), (text.clone(), scope));
        }
        _ => return Err("must be a string".into()),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn profile_wins_per_key_null_clears_and_saving_targets_the_named_key_or_inputs() {
        let root = tempfile::tempdir().unwrap();
        let global = root.path().join("global.json");
        let profile = root.path().join("profile");
        std::fs::create_dir(&profile).unwrap();
        std::fs::write(
            &global,
            json!({"agent":{"role":"global role","user-details":"global user","inputs":{"team":"core","agent.role":"ignored"}}}).to_string(),
        )
        .unwrap();
        std::fs::write(
            profile.join("config.json"),
            json!({"agent":{"user-details":null,"inputs":{"team":"apps"}}}).to_string(),
        )
        .unwrap();
        let config = read(&global, &profile).unwrap();
        assert_eq!(
            config.inputs,
            BTreeMap::from([
                ("agent.role".into(), ("global role".into(), Scope::Global)),
                ("team".into(), ("apps".into(), Scope::Profile)),
            ])
        );

        write(&profile.join("config.json"), "agent.role", "profile role").unwrap();
        write(&profile.join("config.json"), "repo.notes", "notes").unwrap();
        let saved = read_json_file(&profile.join("config.json")).unwrap();
        assert_eq!(saved["agent"]["role"], "profile role");
        assert_eq!(saved["agent"]["inputs"]["repo.notes"], "notes");
        assert_eq!(saved["agent"]["inputs"]["team"], "apps");
        let config = read(&global, &profile).unwrap();
        assert_eq!(
            config.inputs["agent.role"],
            ("profile role".into(), Scope::Profile)
        );
    }
}

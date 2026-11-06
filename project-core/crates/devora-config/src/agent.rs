//! Agent creation inputs, resolved per key from global then profile config.
//! Relative template-file paths are anchored at the config file defining them.
use crate::config::{expand_tilde, read_json_file};
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct AgentConfig {
    pub template_file: Option<PathBuf>,
    pub role: Option<String>,
    pub user_details: Option<String>,
    pub custom_sections: Option<String>,
    pub inputs: BTreeMap<String, String>,
}

pub fn read(global: &Path, profile: &Path) -> Result<AgentConfig, String> {
    let mut merged = serde_json::Map::new();
    let mut inputs = serde_json::Map::new();
    for file in [global.to_path_buf(), profile.join("config.json")] {
        if !file.exists() {
            continue;
        }
        let document = read_json_file(&file)?;
        let Some(section) = document.get("agent") else {
            continue;
        };
        let section = section
            .as_object()
            .ok_or("agent config must be an object")?;
        for (key, value) in section {
            if key == "inputs" {
                if value.is_null() {
                    inputs.clear();
                } else {
                    inputs.extend(
                        value
                            .as_object()
                            .ok_or("agent.inputs must be an object")?
                            .clone(),
                    );
                }
            } else if key == "template-file" && !value.is_null() {
                let text = value
                    .as_str()
                    .ok_or("agent.template-file must be a path string")?;
                let path = PathBuf::from(expand_tilde(text)?);
                let path = if path.is_absolute() {
                    path
                } else {
                    file.parent().unwrap_or(Path::new(".")).join(path)
                };
                merged.insert(
                    key.clone(),
                    serde_json::to_value(path).map_err(|e| e.to_string())?,
                );
            } else {
                merged.insert(key.clone(), value.clone());
            }
        }
    }
    merged.insert("inputs".into(), Value::Object(inputs));
    serde_json::from_value(Value::Object(merged)).map_err(|e| format!("invalid agent config: {e}"))
}

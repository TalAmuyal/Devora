//! Typed Agent card values and their defining config scope.
use crate::{CoreError, DevoraCore};
use devora_core::agent_config::settings;
use serde_json::Value;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum AgentSettingValue {
    Text {
        value: String,
    },
    Percent {
        value: u8,
    },
    Bool {
        value: bool,
    },
    /// A legacy explicit JSON null, distinct from an absent override.
    Cleared,
}

impl AgentSettingValue {
    fn from_json(value: Value) -> Result<Self, CoreError> {
        match value {
            Value::Null => Ok(Self::Cleared),
            Value::String(value) => Ok(Self::Text { value }),
            Value::Bool(value) => Ok(Self::Bool { value }),
            Value::Number(value) => value
                .as_u64()
                .and_then(|value| u8::try_from(value).ok())
                .map(|value| Self::Percent { value })
                .ok_or_else(|| "invalid agent percentage".to_string().into()),
            _ => Err("invalid agent setting value".to_string().into()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum AgentSettingSource {
    Default,
    Global,
    Profile,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct AgentSettingEntry {
    pub key: String,
    pub stored: Option<AgentSettingValue>,
    pub resolved: Option<AgentSettingValue>,
    pub source: AgentSettingSource,
}

impl TryFrom<settings::Entry> for AgentSettingEntry {
    type Error = CoreError;
    fn try_from(entry: settings::Entry) -> Result<Self, Self::Error> {
        Ok(Self {
            key: entry.key,
            stored: entry.stored.map(AgentSettingValue::from_json).transpose()?,
            resolved: entry
                .resolved
                .map(AgentSettingValue::from_json)
                .transpose()?,
            source: match entry.source {
                settings::Source::Default => AgentSettingSource::Default,
                settings::Source::Global => AgentSettingSource::Global,
                settings::Source::Profile => AgentSettingSource::Profile,
            },
        })
    }
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct AgentSettings {
    pub fields: Vec<AgentSettingEntry>,
    pub inputs: Vec<AgentSettingEntry>,
    pub inputs_cleared: bool,
}

#[uniffi::export]
impl DevoraCore {
    pub fn get_agent_settings(
        &self,
        profile_path: Option<String>,
    ) -> Result<AgentSettings, CoreError> {
        let settings = settings::read(&self.config_path, profile_path.as_deref().map(Path::new))?;
        Ok(AgentSettings {
            fields: settings
                .fields
                .into_iter()
                .map(AgentSettingEntry::try_from)
                .collect::<Result<_, _>>()?,
            inputs: settings
                .inputs
                .into_iter()
                .map(AgentSettingEntry::try_from)
                .collect::<Result<_, _>>()?,
            inputs_cleared: settings.inputs_cleared,
        })
    }

    /// None removes this scope's override. Empty multiline strings remain explicit values.
    pub fn set_agent_setting(
        &self,
        profile_path: Option<String>,
        key: String,
        value: Option<String>,
    ) -> Result<(), CoreError> {
        Ok(settings::write_setting(
            &self.config_path,
            profile_path.as_deref().map(Path::new),
            &key,
            value.as_deref(),
        )?)
    }

    /// A changed key renames one stored entry in one atomic file write.
    pub fn set_agent_input(
        &self,
        profile_path: Option<String>,
        previous_key: Option<String>,
        key: String,
        value: Option<String>,
    ) -> Result<(), CoreError> {
        Ok(settings::write_input(
            &self.config_path,
            profile_path.as_deref().map(Path::new),
            previous_key.as_deref(),
            &key,
            value.as_deref(),
        )?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn exports_percent_bool_clear_and_sources_and_keeps_literal_map_names() {
        let tmp = tempfile::tempdir().unwrap();
        let global = tmp.path().join("config.json");
        let profile = tmp.path().join("profile");
        fs::create_dir(&profile).unwrap();
        fs::write(
            &global,
            r#"{"agent":{"compact-threshold":65,"enter-sends":true,"role":"global"}}"#,
        )
        .unwrap();
        fs::write(profile.join("config.json"), r#"{"agent":{"role":null}}"#).unwrap();
        let core = DevoraCore::new(
            Some(global.to_string_lossy().into_owned()),
            None,
            crate::log::test_log(),
        )
        .unwrap();
        let path = Some(profile.to_string_lossy().into_owned());
        let settings = core.get_agent_settings(path.clone()).unwrap();
        let field = |key: &str| {
            settings
                .fields
                .iter()
                .find(|field| field.key == key)
                .unwrap()
        };
        assert_eq!(
            field("compact-threshold").resolved,
            Some(AgentSettingValue::Percent { value: 65 })
        );
        assert_eq!(
            field("enter-sends").resolved,
            Some(AgentSettingValue::Bool { value: true })
        );
        assert_eq!(field("enter-sends").source, AgentSettingSource::Global);
        assert_eq!(field("role").stored, Some(AgentSettingValue::Cleared));
        core.set_agent_input(
            path.clone(),
            None,
            "repo.notes".into(),
            Some("notes".into()),
        )
        .unwrap();
        core.set_agent_input(
            path.clone(),
            Some("repo.notes".into()),
            "repo.renamed".into(),
            Some("updated".into()),
        )
        .unwrap();
        let settings = core.get_agent_settings(path.clone()).unwrap();
        assert_eq!(settings.inputs.len(), 1);
        assert_eq!(settings.inputs[0].key, "repo.renamed");
        assert_eq!(settings.inputs[0].source, AgentSettingSource::Profile);
        core.set_agent_setting(path.clone(), "role".into(), None)
            .unwrap();
        let inherited = core.get_agent_settings(path).unwrap();
        assert_eq!(
            inherited
                .fields
                .iter()
                .find(|entry| entry.key == "role")
                .unwrap()
                .resolved,
            Some(AgentSettingValue::Text {
                value: "global".into()
            })
        );
    }
}

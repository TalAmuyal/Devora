//! Claude launch settings at User Defaults or profile scope, keeping explicit None distinct from inheritance.

use std::collections::HashMap;

use devora_core::config;
use serde_json::Value;

use crate::{CoreError, DevoraCore};

/// A stored setting is either a concrete value or explicit None; an absent map key inherits.
#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum ClaudeValue {
    Text { value: String },
    None,
}

#[derive(Debug, PartialEq, uniffi::Record)]
pub struct ClaudeSettings {
    pub stored: HashMap<String, ClaudeValue>,
    pub resolved: HashMap<String, ClaudeValue>,
}

/// Choices owned by the Rust configuration module; model and permission suggestions are not validation lists.
#[derive(Debug, PartialEq, uniffi::Record)]
pub struct ClaudeSettingChoices {
    /// Lowest to highest.
    pub effort_levels: Vec<String>,
    pub model_suggestions: Vec<String>,
    pub permission_mode_suggestions: Vec<String>,
}

fn values(map: serde_json::Map<String, Value>) -> HashMap<String, ClaudeValue> {
    map.into_iter()
        .map(|(key, value)| {
            let value = match value {
                Value::String(value) => ClaudeValue::Text { value },
                _ => ClaudeValue::None,
            };
            (key, value)
        })
        .collect()
}

#[uniffi::export]
impl DevoraCore {
    pub fn get_claude_settings(
        &self,
        profile_path: Option<String>,
    ) -> Result<ClaudeSettings, CoreError> {
        let settings = config::read_claude_settings_in(&self.config_path, profile_path.as_deref())?;
        Ok(ClaudeSettings {
            stored: values(settings.stored),
            resolved: values(settings.resolved),
        })
    }

    /// A missing value inherits; explicit None writes JSON null.
    pub fn set_claude_setting(
        &self,
        profile_path: Option<String>,
        key: String,
        value: Option<ClaudeValue>,
    ) -> Result<(), CoreError> {
        let (state, text) = match &value {
            None => ("default", None),
            Some(ClaudeValue::None) => ("none", None),
            Some(ClaudeValue::Text { value }) => ("value", Some(value.as_str())),
        };
        Ok(config::write_claude_setting_in(
            &self.config_path,
            profile_path.as_deref(),
            &key,
            state,
            text,
        )?)
    }

    pub fn get_claude_setting_choices(&self) -> Result<ClaudeSettingChoices, CoreError> {
        let choices = config::claude_setting_choices();
        Ok(ClaudeSettingChoices {
            effort_levels: choices.effort_levels,
            model_suggestions: choices.model_suggestions,
            permission_mode_suggestions: choices.permission_mode_suggestions,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn text(value: &str) -> ClaudeValue {
        ClaudeValue::Text {
            value: value.into(),
        }
    }

    #[test]
    fn explicit_none_stops_inheritance_and_default_restores_it() {
        let root = tempfile::tempdir().unwrap();
        let config = root.path().join("config.json");
        let profile = root.path().join("work");
        fs::create_dir(&profile).unwrap();
        fs::write(
            &config,
            r#"{"profiles":[],"terminal":{"git-shortcuts":false}}"#,
        )
        .unwrap();
        fs::write(profile.join("config.json"), r#"{"name":"Work"}"#).unwrap();
        let core = DevoraCore::new(
            Some(config.to_string_lossy().into_owned()),
            None,
            crate::log::test_log(),
        )
        .unwrap();
        let scope = Some(profile.to_string_lossy().into_owned());
        core.set_claude_setting(None, "opus-model".into(), Some(text("new-model")))
            .unwrap();
        core.set_claude_setting(scope.clone(), "opus-model".into(), Some(ClaudeValue::None))
            .unwrap();
        let explicit = core.get_claude_settings(scope.clone()).unwrap();
        assert_eq!(explicit.stored["opus-model"], ClaudeValue::None);
        assert_eq!(explicit.resolved["opus-model"], ClaudeValue::None);
        core.set_claude_setting(scope.clone(), "opus-model".into(), None)
            .unwrap();
        let inherited = core.get_claude_settings(scope.clone()).unwrap();
        assert!(!inherited.stored.contains_key("opus-model"));
        assert_eq!(inherited.resolved["opus-model"], text("new-model"));
        core.set_claude_setting(
            scope.clone(),
            "permission-mode".into(),
            Some(text("future-mode")),
        )
        .unwrap();
        assert_eq!(
            core.get_claude_settings(scope.clone()).unwrap().resolved["permission-mode"],
            text("future-mode")
        );
        assert!(core
            .set_claude_setting(scope, "effort".into(), Some(text("invalid")))
            .is_err());
        let user: Value = serde_json::from_str(&fs::read_to_string(config).unwrap()).unwrap();
        assert_eq!(user["terminal"]["git-shortcuts"], false);
        let profile: Value =
            serde_json::from_str(&fs::read_to_string(profile.join("config.json")).unwrap())
                .unwrap();
        assert_eq!(profile["name"], "Work");
        assert_eq!(
            profile["claude"],
            serde_json::json!({"permission-mode":"future-mode"})
        );
        let choices = core.get_claude_setting_choices().unwrap();
        assert_eq!(choices.effort_levels, config::CLAUDE_EFFORT_LEVELS);
        assert_eq!(choices.model_suggestions, config::CLAUDE_MODEL_SUGGESTIONS);
    }
}

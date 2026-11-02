//! The Settings Hub's config settings (`devora_core::config`, `CONFIG_FIELDS`) across the boundary, at either scope: the User Defaults (the global config) or a profile's.

use std::collections::HashMap;

use devora_core::config;
use serde_json::Value;

use crate::{CoreError, DevoraCore};

/// A config setting's value: text, or a boolean for the On/Off settings.
#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum ConfigValue {
    Text { value: String },
    Bool { value: bool },
}

impl ConfigValue {
    fn from_json(value: Value) -> Option<Self> {
        match value {
            Value::String(value) => Some(Self::Text { value }),
            Value::Bool(value) => Some(Self::Bool { value }),
            _ => None,
        }
    }
}

/// One scope's config settings, by dot-path key (`terminal.default-app`, `task-tracker.asana.project-id`, …).
#[derive(Debug, PartialEq, uniffi::Record)]
pub struct ConfigSettings {
    /// The values this scope stores; a key it doesn't store inherits ("Default").
    pub stored: HashMap<String, ConfigValue>,
    /// Every key's effective value, profile → User Defaults → Devora's built-in; nil when none applies, as for a default Debi owns.
    pub resolved: HashMap<String, Option<ConfigValue>>,
}

#[uniffi::export]
impl DevoraCore {
    /// The config settings of a profile, or of the User Defaults without one.
    pub fn get_config_settings(
        &self,
        profile_path: Option<String>,
    ) -> Result<ConfigSettings, CoreError> {
        let response = config::read_config_settings_in(&self.config_path, profile_path.as_deref())?;
        Ok(ConfigSettings {
            stored: response
                .stored
                .into_iter()
                .filter_map(|(key, value)| Some((key, ConfigValue::from_json(value)?)))
                .collect(),
            resolved: response
                .resolved
                .into_iter()
                .map(|(key, value)| (key, ConfigValue::from_json(value)))
                .collect(),
        })
    }

    /// Stores `value` for `key` in a profile's config, or in the User Defaults without one, keeping the other keys; a nil `value` removes the key, so that the scope inherits it.
    /// A boolean setting takes `"true"` or `"false"`; a text setting can't be blank, while the task tracker's provider takes `""` for none.
    pub fn set_config_setting(
        &self,
        profile_path: Option<String>,
        key: String,
        value: Option<String>,
    ) -> Result<(), CoreError> {
        let state = if value.is_some() { "value" } else { "default" };
        Ok(config::write_config_setting_in(
            &self.config_path,
            profile_path.as_deref(),
            &key,
            state,
            value.as_deref(),
        )?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    use std::sync::Arc;

    fn text(value: &str) -> ConfigValue {
        ConfigValue::Text {
            value: value.into(),
        }
    }

    /// A global config and a profile, `work`, under `root`.
    fn core(root: &Path) -> (Arc<DevoraCore>, String) {
        let config = root.join("config.json");
        fs::write(&config, r#"{"profiles": []}"#).unwrap();
        let profile = root.join("work");
        fs::create_dir(&profile).unwrap();
        fs::write(profile.join("config.json"), r#"{"name": "work"}"#).unwrap();
        let core = DevoraCore::new(
            Some(config.to_string_lossy().into_owned()),
            None,
            crate::log::test_log(),
        )
        .unwrap();
        (core, profile.to_string_lossy().into_owned())
    }

    fn read(path: &Path) -> Value {
        serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
    }

    #[test]
    fn a_profile_value_overrides_the_user_default_and_removing_it_inherits_again() {
        let tmp = tempfile::tempdir().unwrap();
        let (core, profile) = core(tmp.path());
        let key = "terminal.default-app".to_string();

        core.set_config_setting(None, key.clone(), Some("nvim".into()))
            .unwrap();
        core.set_config_setting(Some(profile.clone()), key.clone(), Some("hx".into()))
            .unwrap();
        let overridden = core.get_config_settings(Some(profile.clone())).unwrap();
        core.set_config_setting(Some(profile.clone()), key.clone(), None)
            .unwrap();
        let inherited = core.get_config_settings(Some(profile.clone())).unwrap();

        assert_eq!(overridden.stored.get(&key), Some(&text("hx")));
        assert_eq!(overridden.resolved.get(&key), Some(&Some(text("hx"))));
        assert_eq!(inherited.stored.get(&key), None);
        assert_eq!(inherited.resolved.get(&key), Some(&Some(text("nvim"))));
        assert_eq!(
            read(&tmp.path().join("work/config.json")),
            serde_json::json!({"name": "work"})
        );
        assert_eq!(
            read(&tmp.path().join("config.json")),
            serde_json::json!({"profiles": [], "terminal": {"default-app": "nvim"}})
        );
    }

    #[test]
    fn reads_booleans_built_in_defaults_and_unresolved_keys() {
        let tmp = tempfile::tempdir().unwrap();
        let (core, _) = core(tmp.path());

        core.set_config_setting(None, "pr.auto-merge".into(), Some("false".into()))
            .unwrap();
        let settings = core.get_config_settings(None).unwrap();

        assert_eq!(
            settings.stored.get("pr.auto-merge"),
            Some(&ConfigValue::Bool { value: false })
        );
        assert_eq!(
            settings.resolved.get("terminal.git-shortcuts"),
            Some(&Some(ConfigValue::Bool { value: true }))
        );
        assert_eq!(settings.resolved.get("prepare-command"), Some(&None));
    }

    #[test]
    fn a_fragment_keeps_its_whitespace_and_the_provider_takes_none() {
        let tmp = tempfile::tempdir().unwrap();
        let (core, profile) = core(tmp.path());

        core.set_config_setting(
            Some(profile.clone()),
            "pr.title-prefix".into(),
            Some("[ENG] ".into()),
        )
        .unwrap();
        core.set_config_setting(
            Some(profile.clone()),
            "task-tracker.provider".into(),
            Some(String::new()),
        )
        .unwrap();
        let settings = core.get_config_settings(Some(profile)).unwrap();

        assert_eq!(
            settings.stored.get("pr.title-prefix"),
            Some(&text("[ENG] "))
        );
        assert_eq!(
            settings.stored.get("task-tracker.provider"),
            Some(&text(""))
        );
    }

    #[test]
    fn rejects_a_blank_text_value_and_an_unknown_key() {
        let tmp = tempfile::tempdir().unwrap();
        let (core, _) = core(tmp.path());

        let blank = core.set_config_setting(None, "prepare-command".into(), Some("  ".into()));
        let unknown = core.set_config_setting(None, "no.such-key".into(), Some("x".into()));

        assert!(blank.is_err());
        assert_eq!(
            unknown.unwrap_err().to_string(),
            "unknown config key: no.such-key"
        );
    }
}

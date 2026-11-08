//! User Defaults for the periodic parent-clone updater.
use crate::config;
use std::path::Path;

pub const INTERVAL_MINUTES: u32 = 60;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppKind {
    Sapphire,
    Ember,
}

impl AppKind {
    pub fn enabled_by_default(self) -> bool {
        self == Self::Sapphire
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    pub enabled: bool,
    pub interval_minutes: u32,
}

pub fn is_key(key: &str) -> bool {
    matches!(key, "repo-update.enabled" | "repo-update.interval-minutes")
}

impl Settings {
    pub fn read(config_path: &Path, app: AppKind) -> Result<Self, String> {
        let settings = config::read_config_settings_in(config_path, None)?;
        Ok(Self {
            enabled: settings
                .resolved
                .get("repo-update.enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(app.enabled_by_default()),
            interval_minutes: settings
                .resolved
                .get("repo-update.interval-minutes")
                .and_then(|v| v.as_u64())
                .and_then(|v| u32::try_from(v).ok())
                .filter(|v| *v > 0)
                .unwrap_or(INTERVAL_MINUTES),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn defaults_are_app_specific_and_global_values_are_validated() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("config.json");
        assert!(Settings::read(&path, AppKind::Sapphire).unwrap().enabled);
        assert!(!Settings::read(&path, AppKind::Ember).unwrap().enabled);
        for value in ["0", "-1", "1.5", "bad", "4294967296"] {
            assert!(config::write_config_setting_in(
                &path,
                None,
                "repo-update.interval-minutes",
                "value",
                Some(value)
            )
            .is_err());
        }
        config::write_config_setting_in(
            &path,
            None,
            "repo-update.interval-minutes",
            "value",
            Some("15"),
        )
        .unwrap();
        config::write_config_setting_in(&path, None, "repo-update.enabled", "value", Some("false"))
            .unwrap();
        let settings = Settings::read(&path, AppKind::Sapphire).unwrap();
        assert_eq!(
            settings,
            Settings {
                enabled: false,
                interval_minutes: 15
            }
        );
        let json: Value = config::read_json_file(&path).unwrap();
        assert_eq!(json["repo-update"]["interval-minutes"], 15);
        assert!(config::write_config_setting_in(
            &path,
            Some(root.path().to_str().unwrap()),
            "repo-update.enabled",
            "value",
            Some("true")
        )
        .is_err());
        let profile =
            config::read_config_settings_in(&path, Some(root.path().to_str().unwrap())).unwrap();
        assert!(!profile.resolved.contains_key("repo-update.enabled"));
    }
}

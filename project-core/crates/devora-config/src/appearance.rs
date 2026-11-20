//! The app's Appearance, a User Defaults setting (ADR-010): the system's (Automatic, the default), Light or Dark.
use crate::config;
use std::path::Path;

pub const KEY: &str = "appearance";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Appearance {
    Automatic,
    Light,
    Dark,
}

impl Appearance {
    /// The User Defaults' choice. A value that isn't `light` or `dark` (hand-edited, or from a later version) is Automatic, so it never fails a launch.
    pub fn read(config_path: &Path) -> Result<Self, String> {
        let settings = config::read_config_settings_in(config_path, None)?;
        Ok(
            match settings.resolved.get(KEY).and_then(|value| value.as_str()) {
                Some("light") => Self::Light,
                Some("dark") => Self::Dark,
                _ => Self::Automatic,
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn write(path: &Path, config: serde_json::Value) {
        std::fs::write(path, config.to_string()).unwrap();
    }

    #[test]
    fn reads_the_user_defaults_choice_and_anything_else_as_automatic() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("config.json");
        assert_eq!(Appearance::read(&path).unwrap(), Appearance::Automatic);
        for (stored, expected) in [
            (json!("light"), Appearance::Light),
            (json!("dark"), Appearance::Dark),
            (json!("sepia"), Appearance::Automatic),
            (json!(""), Appearance::Automatic),
            (json!(true), Appearance::Automatic),
        ] {
            write(&path, json!({ KEY: stored }));
            assert_eq!(Appearance::read(&path).unwrap(), expected, "{stored}");
        }
    }

    #[test]
    fn it_is_set_only_in_the_user_defaults_to_light_or_dark() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("config.json");
        let profile = root.path().join("Work");
        std::fs::create_dir_all(&profile).unwrap();
        let profile = profile.to_str().unwrap();
        let set = |profile: Option<&str>, value: &str| {
            config::write_config_setting_in(&path, profile, KEY, "value", Some(value))
        };

        set(None, "dark").unwrap();
        assert_eq!(Appearance::read(&path).unwrap(), Appearance::Dark);
        let stored: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(stored[KEY], "dark");
        for invalid in ["", "automatic", "sepia"] {
            assert!(set(None, invalid).is_err(), "{invalid:?}");
        }
        assert!(set(Some(profile), "light").is_err());
        let profile_settings = config::read_config_settings_in(&path, Some(profile)).unwrap();
        assert!(!profile_settings.resolved.contains_key(KEY));

        config::write_config_setting_in(&path, None, KEY, "default", None).unwrap();
        assert_eq!(Appearance::read(&path).unwrap(), Appearance::Automatic);
    }
}

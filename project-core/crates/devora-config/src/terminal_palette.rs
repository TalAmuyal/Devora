//! The terminals' palette, a User Defaults setting (ADR-010, amended 2026-11-21): Selenized (the default), Catppuccin
//! or Gruvbox, each a light and a dark variant that the terminals switch between with the app's appearance.
use crate::config;
use std::path::Path;

pub const KEY: &str = "terminal.palette";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminalPalette {
    Selenized,
    Catppuccin,
    Gruvbox,
}

impl TerminalPalette {
    /// The User Defaults' choice. A value it doesn't know (hand-edited, or from a later version) is Selenized, so it
    /// never fails a launch.
    pub fn read(config_path: &Path) -> Result<Self, String> {
        let settings = config::read_config_settings_in(config_path, None)?;
        Ok(
            match settings.resolved.get(KEY).and_then(|value| value.as_str()) {
                Some("catppuccin") => Self::Catppuccin,
                Some("gruvbox") => Self::Gruvbox,
                _ => Self::Selenized,
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_the_user_defaults_choice_and_anything_else_as_selenized() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("config.json");
        assert_eq!(
            TerminalPalette::read(&path).unwrap(),
            TerminalPalette::Selenized
        );
        for (stored, expected) in [
            (json!("catppuccin"), TerminalPalette::Catppuccin),
            (json!("gruvbox"), TerminalPalette::Gruvbox),
            (json!("solarized"), TerminalPalette::Selenized),
            (json!(""), TerminalPalette::Selenized),
            (json!(1), TerminalPalette::Selenized),
        ] {
            std::fs::write(
                &path,
                json!({ "terminal": { "palette": stored } }).to_string(),
            )
            .unwrap();
            assert_eq!(TerminalPalette::read(&path).unwrap(), expected, "{stored}");
        }
    }

    #[test]
    fn it_is_set_only_in_the_user_defaults() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("config.json");
        let profile = root.path().join("Work");
        std::fs::create_dir_all(&profile).unwrap();
        let profile = profile.to_str().unwrap();

        config::write_config_setting_in(&path, None, KEY, "value", Some("gruvbox")).unwrap();
        assert_eq!(
            TerminalPalette::read(&path).unwrap(),
            TerminalPalette::Gruvbox
        );
        for invalid in ["", "selenized", "solarized"] {
            assert!(
                config::write_config_setting_in(&path, None, KEY, "value", Some(invalid)).is_err(),
                "{invalid:?}"
            );
        }
        assert!(config::write_config_setting_in(
            &path,
            Some(profile),
            KEY,
            "value",
            Some("catppuccin")
        )
        .is_err());
        let profile_settings = config::read_config_settings_in(&path, Some(profile)).unwrap();
        assert!(!profile_settings.resolved.contains_key(KEY));
    }
}

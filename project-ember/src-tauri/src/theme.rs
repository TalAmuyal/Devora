use std::collections::HashMap;
use std::fs;

/// Default path to the Kitty theme file, resolved at compile time relative to the repo layout.
const DEFAULT_THEME_PATH: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../kitty-configs/current-theme.conf");

/// Returns the list of CSS custom property names that a Kitty config key maps to.
fn css_properties_for(kitty_key: &str) -> &'static [&'static str] {
    match kitty_key {
        "foreground" => &["--color-terminal-fg", "--color-text", "--color-fg"],
        "background" => &["--color-terminal-bg", "--color-bg", "--color-base"],
        "selection_foreground" => &["--color-terminal-selection-fg"],
        "selection_background" => &["--color-terminal-selection-bg"],
        "cursor" => &["--color-terminal-cursor"],
        "cursor_text_color" => &[],
        "active_tab_foreground" => &["--color-tab-active-fg"],
        "active_tab_background" => &["--color-tab-active-bg", "--color-accent"],
        "inactive_tab_foreground" => &["--color-tab-inactive-fg"],
        "inactive_tab_background" => &[
            "--color-tab-inactive-bg",
            "--color-bg-secondary",
            "--color-mantle",
        ],
        "tab_bar_background" => &["--color-tab-bar-bg", "--color-crust"],
        "color0" => &["--color-ansi-black", "--color-surface1", "--color-border"],
        "color1" => &["--color-ansi-red", "--color-red", "--color-status-error"],
        "color2" => &["--color-ansi-green", "--color-green", "--color-status-clean"],
        "color3" => &["--color-ansi-yellow", "--color-yellow", "--color-status-modified"],
        "color4" => &["--color-ansi-blue", "--color-blue", "--color-status-info"],
        "color5" => &["--color-ansi-magenta", "--color-pink"],
        "color6" => &["--color-ansi-cyan", "--color-teal"],
        "color7" => &["--color-ansi-white", "--color-subtext1"],
        "color8" => &["--color-ansi-bright-black", "--color-surface2"],
        "color9" => &["--color-ansi-bright-red"],
        "color10" => &["--color-ansi-bright-green"],
        "color11" => &["--color-ansi-bright-yellow"],
        "color12" => &["--color-ansi-bright-blue"],
        "color13" => &["--color-ansi-bright-magenta"],
        "color14" => &["--color-ansi-bright-cyan"],
        "color15" => &[
            "--color-ansi-bright-white",
            "--color-subtext0",
            "--color-fg-secondary",
        ],
        _ => &[],
    }
}

/// Parse a Kitty theme config file and return a map of CSS custom property names to color values.
///
/// Lines starting with `#` are treated as comments. Blank lines are skipped.
/// Each data line is expected to be `key  value` (separated by whitespace).
/// The special key `macos_titlebar_color` is mapped to `--color-crust` unless its value is "system".
pub fn load_theme(theme_path: &str) -> Result<HashMap<String, String>, String> {
    let content =
        fs::read_to_string(theme_path).map_err(|e| format!("failed to read theme file: {e}"))?;

    let mut result = HashMap::new();

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let mut parts = trimmed.splitn(2, char::is_whitespace);
        let key = match parts.next() {
            Some(k) => k,
            None => continue,
        };
        let value = match parts.next() {
            Some(v) => v.trim(),
            None => continue,
        };

        // Special case: macos_titlebar_color maps to --color-crust only when not "system"
        if key == "macos_titlebar_color" {
            if value != "system" {
                result.insert("--color-crust".to_string(), value.to_string());
            }
            continue;
        }

        for css_prop in css_properties_for(key) {
            result.insert(css_prop.to_string(), value.to_string());
        }
    }

    Ok(result)
}

/// Tauri command that loads the theme from the given path, or from the default location.
#[tauri::command]
pub fn get_theme(theme_path: Option<String>) -> Result<HashMap<String, String>, String> {
    let path = theme_path.unwrap_or_else(|| DEFAULT_THEME_PATH.to_string());
    load_theme(&path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_actual_theme_file() {
        let theme = load_theme(DEFAULT_THEME_PATH)
            .expect("should be able to parse the actual current-theme.conf");

        // Verify a selection of key mappings exist with correct values from the file
        assert_eq!(theme.get("--color-terminal-fg"), Some(&"#CAD3F5".to_string()));
        assert_eq!(theme.get("--color-text"), Some(&"#CAD3F5".to_string()));
        assert_eq!(theme.get("--color-fg"), Some(&"#CAD3F5".to_string()));

        assert_eq!(theme.get("--color-terminal-bg"), Some(&"#24273A".to_string()));
        assert_eq!(theme.get("--color-bg"), Some(&"#24273A".to_string()));
        assert_eq!(theme.get("--color-base"), Some(&"#24273A".to_string()));

        assert_eq!(theme.get("--color-terminal-selection-fg"), Some(&"#24273A".to_string()));
        assert_eq!(theme.get("--color-terminal-selection-bg"), Some(&"#F4DBD6".to_string()));
        assert_eq!(theme.get("--color-terminal-cursor"), Some(&"#F4DBD6".to_string()));

        assert_eq!(theme.get("--color-tab-active-fg"), Some(&"#181926".to_string()));
        assert_eq!(theme.get("--color-tab-active-bg"), Some(&"#C6A0F6".to_string()));
        assert_eq!(theme.get("--color-accent"), Some(&"#C6A0F6".to_string()));

        assert_eq!(theme.get("--color-tab-inactive-fg"), Some(&"#CAD3F5".to_string()));
        assert_eq!(theme.get("--color-tab-inactive-bg"), Some(&"#1E2030".to_string()));
        assert_eq!(theme.get("--color-bg-secondary"), Some(&"#1E2030".to_string()));
        assert_eq!(theme.get("--color-mantle"), Some(&"#1E2030".to_string()));

        assert_eq!(theme.get("--color-tab-bar-bg"), Some(&"#181926".to_string()));

        // macos_titlebar_color should also map to --color-crust (value is #181926, not "system")
        // But tab_bar_background also maps to --color-crust. Since both map #181926, the
        // final value depends on parse order. Both write #181926, so the result is the same.
        assert_eq!(theme.get("--color-crust"), Some(&"#181926".to_string()));

        // ANSI colors
        assert_eq!(theme.get("--color-ansi-black"), Some(&"#494D64".to_string()));
        assert_eq!(theme.get("--color-surface1"), Some(&"#494D64".to_string()));
        assert_eq!(theme.get("--color-border"), Some(&"#494D64".to_string()));

        assert_eq!(theme.get("--color-ansi-red"), Some(&"#ED8796".to_string()));
        assert_eq!(theme.get("--color-red"), Some(&"#ED8796".to_string()));
        assert_eq!(theme.get("--color-status-error"), Some(&"#ED8796".to_string()));

        assert_eq!(theme.get("--color-ansi-green"), Some(&"#A6DA95".to_string()));
        assert_eq!(theme.get("--color-green"), Some(&"#A6DA95".to_string()));
        assert_eq!(theme.get("--color-status-clean"), Some(&"#A6DA95".to_string()));

        assert_eq!(theme.get("--color-ansi-yellow"), Some(&"#EED49F".to_string()));
        assert_eq!(theme.get("--color-yellow"), Some(&"#EED49F".to_string()));
        assert_eq!(theme.get("--color-status-modified"), Some(&"#EED49F".to_string()));

        assert_eq!(theme.get("--color-ansi-blue"), Some(&"#8AADF4".to_string()));
        assert_eq!(theme.get("--color-blue"), Some(&"#8AADF4".to_string()));
        assert_eq!(theme.get("--color-status-info"), Some(&"#8AADF4".to_string()));

        assert_eq!(theme.get("--color-ansi-magenta"), Some(&"#F5BDE6".to_string()));
        assert_eq!(theme.get("--color-pink"), Some(&"#F5BDE6".to_string()));

        assert_eq!(theme.get("--color-ansi-cyan"), Some(&"#8BD5CA".to_string()));
        assert_eq!(theme.get("--color-teal"), Some(&"#8BD5CA".to_string()));

        assert_eq!(theme.get("--color-ansi-white"), Some(&"#B8C0E0".to_string()));
        assert_eq!(theme.get("--color-subtext1"), Some(&"#B8C0E0".to_string()));

        assert_eq!(theme.get("--color-ansi-bright-black"), Some(&"#5B6078".to_string()));
        assert_eq!(theme.get("--color-surface2"), Some(&"#5B6078".to_string()));

        assert_eq!(theme.get("--color-ansi-bright-white"), Some(&"#A5ADCB".to_string()));
        assert_eq!(theme.get("--color-subtext0"), Some(&"#A5ADCB".to_string()));
        assert_eq!(theme.get("--color-fg-secondary"), Some(&"#A5ADCB".to_string()));
    }

    #[test]
    fn skip_comments_and_blank_lines() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test-theme.conf");
        std::fs::write(
            &path,
            "# comment line\n\n  \nforeground #AABBCC\n# another comment\n",
        )
        .unwrap();

        let theme = load_theme(path.to_str().unwrap()).unwrap();

        assert_eq!(theme.get("--color-terminal-fg"), Some(&"#AABBCC".to_string()));
        assert_eq!(theme.get("--color-text"), Some(&"#AABBCC".to_string()));
        assert_eq!(theme.get("--color-fg"), Some(&"#AABBCC".to_string()));
        assert_eq!(theme.len(), 3);
    }

    #[test]
    fn macos_titlebar_system_is_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test-theme.conf");
        std::fs::write(&path, "macos_titlebar_color system\n").unwrap();

        let theme = load_theme(path.to_str().unwrap()).unwrap();
        assert!(theme.is_empty());
    }

    #[test]
    fn macos_titlebar_hex_maps_to_crust() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test-theme.conf");
        std::fs::write(&path, "macos_titlebar_color #112233\n").unwrap();

        let theme = load_theme(path.to_str().unwrap()).unwrap();
        assert_eq!(theme.get("--color-crust"), Some(&"#112233".to_string()));
        assert_eq!(theme.len(), 1);
    }

    #[test]
    fn unknown_keys_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test-theme.conf");
        std::fs::write(&path, "url_color #FF0000\nactive_border_color #00FF00\n").unwrap();

        let theme = load_theme(path.to_str().unwrap()).unwrap();
        assert!(theme.is_empty());
    }

    #[test]
    fn missing_file_returns_error() {
        let result = load_theme("/nonexistent/path/theme.conf");
        assert!(result.is_err());
    }
}

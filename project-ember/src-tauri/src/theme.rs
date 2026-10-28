use std::collections::HashMap;
use tauri::Manager;

use devora_core::theme::load_theme;

/// Where `populate-app-resources.sh` bundles the theme, relative to the app's resource directory.
const BUNDLED_THEME_PATH: &str = "themes/catppuccin-macchiato.conf";

/// Tauri command that loads the theme bundled in the app.
#[tauri::command]
pub fn get_theme(app: tauri::AppHandle) -> Result<HashMap<String, String>, String> {
    let resource_dir = app
        .path()
        .resource_dir()
        .map_err(|e| format!("failed to resolve resource directory: {e}"))?;
    load_theme(&resource_dir.join(BUNDLED_THEME_PATH))
}

use std::collections::HashMap;
use tauri::Manager;

use devora_core::theme::{bundled_theme_path, load_theme};

/// Tauri command that loads the theme bundled in the app.
#[tauri::command]
pub fn get_theme(app: tauri::AppHandle) -> Result<HashMap<String, String>, String> {
    let resource_dir = app
        .path()
        .resource_dir()
        .map_err(|e| format!("failed to resolve resource directory: {e}"))?;
    load_theme(&bundled_theme_path(&resource_dir))
}

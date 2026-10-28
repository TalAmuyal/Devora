//! Profile registry operations: creating/registering profile root directories and maintaining the "profiles" list in the global config.
//!
//! All functions take the global config path explicitly so tests never need to mutate process-wide environment variables.

use std::fs;
use std::path::Path;

use serde::Serialize;
use serde_json::Value;

use crate::workspace;
use crate::workspace::write_pretty_json;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisteredProfile {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfilePathValidation {
    pub kind: String, // "new" | "existing_profile" | "invalid"
    pub name: Option<String>,
    pub error: Option<String>,
    pub expanded_path: String,
}

impl ProfilePathValidation {
    fn new_profile(expanded_path: String) -> Self {
        Self {
            kind: "new".to_string(),
            name: None,
            error: None,
            expanded_path,
        }
    }

    fn existing_profile(expanded_path: String, name: String) -> Self {
        Self {
            kind: "existing_profile".to_string(),
            name: Some(name),
            error: None,
            expanded_path,
        }
    }

    fn invalid(expanded_path: String, error: &str) -> Self {
        Self {
            kind: "invalid".to_string(),
            name: None,
            error: Some(error.to_string()),
            expanded_path,
        }
    }
}

/// Classifies a user-entered profile root path for the new-profile form.
pub fn validate_profile_path(path: &str) -> Result<ProfilePathValidation, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Ok(ProfilePathValidation::invalid(
            String::new(),
            "Please enter a path",
        ));
    }

    let expanded = workspace::expand_tilde(trimmed)?;
    let dir = Path::new(&expanded);

    if !dir.is_absolute() {
        return Ok(ProfilePathValidation::invalid(
            expanded.clone(),
            "Path must be absolute or start with ~",
        ));
    }

    if dir.exists() {
        if !dir.is_dir() {
            return Ok(ProfilePathValidation::invalid(
                expanded.clone(),
                "Path exists but is not a directory",
            ));
        }
        let config_path = dir.join("config.json");
        if !config_path.exists() {
            return Ok(ProfilePathValidation::new_profile(expanded));
        }
        return Ok(match workspace::read_json_file(&config_path) {
            Ok(cfg) => match cfg.get("name").and_then(|v| v.as_str()) {
                Some(name) => ProfilePathValidation::existing_profile(expanded, name.to_string()),
                None => ProfilePathValidation::invalid(
                    expanded,
                    "Directory contains a config.json without a \"name\"",
                ),
            },
            Err(_) => ProfilePathValidation::invalid(
                expanded,
                "Directory contains an unrecognized config.json",
            ),
        });
    }

    match dir.parent() {
        Some(parent) if parent.is_dir() => Ok(ProfilePathValidation::new_profile(expanded)),
        _ => Ok(ProfilePathValidation::invalid(
            expanded,
            "Parent directory does not exist",
        )),
    }
}

/// Registers a profile in the global config.
/// For a directory that is already an initialized profile, the on-disk name wins and the directory is left untouched; otherwise the profile structure (config.json, repos/, workspaces/) is created with the given name.
/// Idempotent for paths that are already registered.
pub fn register_profile(
    config_path: &Path,
    root_path: &str,
    name: &str,
) -> Result<RegisteredProfile, String> {
    let validation = validate_profile_path(root_path)?;
    let expanded = validation.expanded_path.clone();
    let profile_dir = Path::new(&expanded);

    // Load the registry before touching the filesystem so a corrupt global config fails the whole operation without leaving a half-created profile.
    let mut global = load_global_config(config_path)?;

    let final_name = match validation.kind.as_str() {
        "existing_profile" => validation.name.unwrap_or_default(),
        "new" => {
            let name = name.trim();
            if name.is_empty() {
                return Err("Please enter a profile name".to_string());
            }
            for sub in ["repos", "workspaces"] {
                let dir = profile_dir.join(sub);
                fs::create_dir_all(&dir)
                    .map_err(|e| format!("failed to create {}: {e}", dir.display()))?;
            }
            write_pretty_json(
                &profile_dir.join("config.json"),
                &serde_json::json!({ "name": name }),
            )?;
            name.to_string()
        }
        _ => {
            return Err(validation
                .error
                .unwrap_or_else(|| "Invalid profile path".to_string()))
        }
    };

    let obj = global
        .as_object_mut()
        .ok_or("global config is not a JSON object")?;
    let profiles = obj
        .entry("profiles")
        .or_insert_with(|| Value::Array(Vec::new()));
    let list = profiles
        .as_array_mut()
        .ok_or("\"profiles\" is not an array in global config")?;
    if !list.iter().any(|v| v.as_str() == Some(expanded.as_str())) {
        list.push(Value::String(expanded.clone()));
    }
    write_global_config(config_path, &global)?;

    Ok(RegisteredProfile {
        name: final_name,
        path: expanded,
    })
}

/// Removes a profile from the global config registry.
/// The profile directory and its contents are left untouched.
/// Removing a path that is not registered is a no-op success.
pub fn unregister_profile(config_path: &Path, root_path: &str) -> Result<(), String> {
    let expanded = workspace::expand_tilde(root_path.trim())?;

    let mut global = load_global_config(config_path)?;
    let obj = global
        .as_object_mut()
        .ok_or("global config is not a JSON object")?;
    let remaining: Vec<Value> = obj
        .get("profiles")
        .and_then(|v| v.as_array())
        .map(|list| {
            list.iter()
                .filter(|v| v.as_str() != Some(expanded.as_str()))
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    obj.insert("profiles".to_string(), Value::Array(remaining));

    write_global_config(config_path, &global)
}

/// Reads the global config, treating a missing file as `{}` but a corrupt one as an error (so a registration can never clobber unrelated settings).
fn load_global_config(config_path: &Path) -> Result<Value, String> {
    if !config_path.exists() {
        return Ok(Value::Object(serde_json::Map::new()));
    }
    workspace::read_json_file(config_path)
}

fn write_global_config(config_path: &Path, config: &Value) -> Result<(), String> {
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create {}: {e}", parent.display()))?;
    }
    write_pretty_json(config_path, config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn config_in(dir: &Path) -> std::path::PathBuf {
        dir.join("config-dir").join("config.json")
    }

    fn read_profiles_list(config_path: &Path) -> Vec<String> {
        let config = workspace::read_json_file(config_path).unwrap();
        config["profiles"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect()
    }

    #[test]
    fn register_profile_creates_structure() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = config_in(tmp.path());
        let root = tmp.path().join("my-profile");

        let result =
            register_profile(&config_path, &root.to_string_lossy(), "Work").unwrap();

        assert_eq!(result.name, "Work");
        assert_eq!(result.path, root.to_string_lossy());
        assert!(root.join("repos").is_dir());
        assert!(root.join("workspaces").is_dir());
        let profile_config = workspace::read_json_file(&root.join("config.json")).unwrap();
        assert_eq!(profile_config["name"], "Work");
        assert_eq!(
            read_profiles_list(&config_path),
            vec![root.to_string_lossy().to_string()]
        );
    }

    #[test]
    fn register_profile_creates_global_config_and_parents_when_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join("deeply/nested/config.json");
        let root = tmp.path().join("p");

        register_profile(&config_path, &root.to_string_lossy(), "X").unwrap();

        assert!(config_path.exists());
        assert_eq!(read_profiles_list(&config_path).len(), 1);
    }

    #[test]
    fn register_profile_reuses_existing_profile_name() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = config_in(tmp.path());
        let root = tmp.path().join("existing");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("config.json"), r#"{"name": "Legacy", "repos": []}"#).unwrap();

        let result =
            register_profile(&config_path, &root.to_string_lossy(), "Ignored").unwrap();

        assert_eq!(result.name, "Legacy");
        // config.json untouched (repos key still present)
        let profile_config = workspace::read_json_file(&root.join("config.json")).unwrap();
        assert!(profile_config.get("repos").is_some());
    }

    #[test]
    fn register_profile_idempotent_for_already_registered_path() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = config_in(tmp.path());
        let root = tmp.path().join("p");

        register_profile(&config_path, &root.to_string_lossy(), "A").unwrap();
        register_profile(&config_path, &root.to_string_lossy(), "B").unwrap();

        assert_eq!(read_profiles_list(&config_path).len(), 1);
    }

    #[test]
    fn register_profile_preserves_unrelated_global_config_keys() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = config_in(tmp.path());
        fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        fs::write(&config_path, r#"{"terminal": {"default-app": "nvim"}}"#).unwrap();
        let root = tmp.path().join("p");

        register_profile(&config_path, &root.to_string_lossy(), "A").unwrap();

        let config = workspace::read_json_file(&config_path).unwrap();
        assert_eq!(config["terminal"]["default-app"], "nvim");
        assert_eq!(read_profiles_list(&config_path).len(), 1);
    }

    #[test]
    fn register_profile_errors_on_corrupt_global_config() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = config_in(tmp.path());
        fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        fs::write(&config_path, "{not json").unwrap();
        let root = tmp.path().join("p");

        let result = register_profile(&config_path, &root.to_string_lossy(), "A");

        assert!(result.is_err());
        assert!(!root.exists(), "validation error must not create the profile dir");
    }

    #[test]
    fn register_profile_refuses_dir_with_invalid_config_json() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = config_in(tmp.path());
        let root = tmp.path().join("broken");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("config.json"), r#"{"no-name": true}"#).unwrap();

        let result = register_profile(&config_path, &root.to_string_lossy(), "A");

        assert!(result.is_err());
        // config.json untouched
        let profile_config = workspace::read_json_file(&root.join("config.json")).unwrap();
        assert!(profile_config.get("no-name").is_some());
    }

    #[test]
    fn register_profile_requires_name_for_new_profile() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = config_in(tmp.path());
        let root = tmp.path().join("p");

        let result = register_profile(&config_path, &root.to_string_lossy(), "  ");

        assert!(result.is_err());
        assert!(!root.exists());
    }

    #[test]
    fn unregister_profile_removes_entry() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = config_in(tmp.path());
        let root_a = tmp.path().join("a");
        let root_b = tmp.path().join("b");
        register_profile(&config_path, &root_a.to_string_lossy(), "A").unwrap();
        register_profile(&config_path, &root_b.to_string_lossy(), "B").unwrap();

        unregister_profile(&config_path, &root_a.to_string_lossy()).unwrap();

        assert_eq!(
            read_profiles_list(&config_path),
            vec![root_b.to_string_lossy().to_string()]
        );
        assert!(root_a.join("config.json").exists(), "directory must remain on disk");
    }

    #[test]
    fn unregister_profile_missing_entry_is_noop() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = config_in(tmp.path());
        let root = tmp.path().join("a");
        register_profile(&config_path, &root.to_string_lossy(), "A").unwrap();

        unregister_profile(&config_path, "/not/registered").unwrap();

        assert_eq!(read_profiles_list(&config_path).len(), 1);
    }

    #[test]
    fn unregister_last_profile_writes_empty_array() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = config_in(tmp.path());
        let root = tmp.path().join("a");
        register_profile(&config_path, &root.to_string_lossy(), "A").unwrap();

        unregister_profile(&config_path, &root.to_string_lossy()).unwrap();

        assert!(read_profiles_list(&config_path).is_empty());
    }

    #[test]
    fn validate_new_dir_with_existing_parent() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("brand-new");

        let v = validate_profile_path(&path.to_string_lossy()).unwrap();

        assert_eq!(v.kind, "new");
        assert_eq!(v.expanded_path, path.to_string_lossy());
    }

    #[test]
    fn validate_existing_empty_dir_is_new() {
        let tmp = tempfile::tempdir().unwrap();

        let v = validate_profile_path(&tmp.path().to_string_lossy()).unwrap();

        assert_eq!(v.kind, "new");
    }

    #[test]
    fn validate_missing_parent_is_invalid() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("nope").join("profile");

        let v = validate_profile_path(&path.to_string_lossy()).unwrap();

        assert_eq!(v.kind, "invalid");
        assert!(v.error.unwrap().contains("Parent directory"));
    }

    #[test]
    fn validate_file_is_invalid() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("a-file");
        fs::write(&file, "x").unwrap();

        let v = validate_profile_path(&file.to_string_lossy()).unwrap();

        assert_eq!(v.kind, "invalid");
        assert!(v.error.unwrap().contains("not a directory"));
    }

    #[test]
    fn validate_initialized_profile_returns_name() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("config.json"), r#"{"name": "Work"}"#).unwrap();

        let v = validate_profile_path(&tmp.path().to_string_lossy()).unwrap();

        assert_eq!(v.kind, "existing_profile");
        assert_eq!(v.name.unwrap(), "Work");
    }

    #[test]
    fn validate_config_without_name_is_invalid() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("config.json"), r#"{"repos": []}"#).unwrap();

        let v = validate_profile_path(&tmp.path().to_string_lossy()).unwrap();

        assert_eq!(v.kind, "invalid");
    }

    #[test]
    fn validate_empty_path_is_invalid() {
        let v = validate_profile_path("   ").unwrap();
        assert_eq!(v.kind, "invalid");
    }

    #[test]
    fn validate_relative_path_is_invalid() {
        let v = validate_profile_path("relative/dir").unwrap();
        assert_eq!(v.kind, "invalid");
        assert!(v.error.unwrap().contains("absolute"));
    }

    #[test]
    fn global_config_written_with_four_space_indent() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = config_in(tmp.path());
        let root = tmp.path().join("p");

        register_profile(&config_path, &root.to_string_lossy(), "A").unwrap();

        let raw = fs::read_to_string(&config_path).unwrap();
        assert!(raw.contains("\n    \"profiles\""), "expected 4-space indent, got: {raw}");
    }
}

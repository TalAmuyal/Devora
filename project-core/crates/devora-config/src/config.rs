//! Devora's configuration: the global config file (`~/.config/devora/config.json`, or `DEVORA_CONFIG_PATH`), per-profile config, and the settings resolved from them (Claude settings, config fields, the prepare-command, git shortcuts).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;

fn home_dir() -> Result<PathBuf, String> {
    std::env::var("HOME")
        .map(PathBuf::from)
        .map_err(|_| "HOME environment variable not set".to_string())
}

pub fn expand_tilde(path: &str) -> Result<String, String> {
    if path == "~" {
        return Ok(home_dir()?.to_string_lossy().into_owned());
    }
    if let Some(rest) = path.strip_prefix("~/") {
        let home = home_dir()?;
        Ok(home.join(rest).to_string_lossy().into_owned())
    } else {
        Ok(path.to_string())
    }
}

pub fn global_config_path() -> Result<PathBuf, String> {
    if let Ok(override_path) = std::env::var("DEVORA_CONFIG_PATH") {
        return Ok(PathBuf::from(override_path));
    }
    let home = home_dir()?;
    Ok(home.join(".config/devora/config.json"))
}

pub fn read_json_file(path: &Path) -> Result<Value, String> {
    let content =
        fs::read_to_string(path).map_err(|e| format!("failed to read {}: {e}", path.display()))?;
    serde_json::from_str(&content)
        .map_err(|e| format!("failed to parse {}: {e}", path.display()))
}

pub fn write_pretty_json(path: &Path, value: &Value) -> Result<(), String> {
    let mut buf = Vec::new();
    let formatter = serde_json::ser::PrettyFormatter::with_indent(b"    ");
    let mut serializer = serde_json::Serializer::with_formatter(&mut buf, formatter);
    value
        .serialize(&mut serializer)
        .map_err(|e| format!("failed to serialize {}: {e}", path.display()))?;
    fs::write(path, buf).map_err(|e| format!("failed to write {}: {e}", path.display()))
}

fn get_value_by_dot_path<'a>(root: &'a Value, dot_path: &str) -> Option<&'a Value> {
    let mut current = root;
    for key in dot_path.split('.') {
        current = current.get(key)?;
    }
    Some(current)
}

pub fn get_config_value(key: &str) -> Result<Option<Value>, String> {
    let config_path = global_config_path()?;
    if !config_path.exists() {
        return Ok(None);
    }

    let config = read_json_file(&config_path)?;
    // The config uses kebab-case keys, so convert dots to navigate
    Ok(get_value_by_dot_path(&config, key).cloned())
}

/// Resolves the session terminal app command (`terminal.default-app`, profile → user).
/// Returns `None` when unset at both scopes, which the caller treats as a bare login shell.
/// Routes through the same resolver as the Settings Hub so the displayed and the launched value cannot disagree.
pub fn get_default_app(profile_path: &str) -> Result<Option<String>, String> {
    let user_cfg = read_config_opt(&global_config_path()?)?;
    let profile_cfg = read_config_opt(&profile_config_path(profile_path))?;
    let spec = config_field("terminal.default-app")
        .expect("terminal.default-app is a registered config field");
    Ok(resolve_config(profile_cfg.as_ref(), user_cfg.as_ref(), spec)
        .and_then(|v| v.as_str().map(str::to_string)))
}

/// Resolves the configured prepare-command (`prepare-command`, profile → user).
/// Returns `None` when unset at both scopes.
/// A malformed/unreadable config degrades to `None` (prepare is skipped) rather than failing workspace creation.
pub fn get_prepare_command(profile_path: Option<&str>) -> Option<String> {
    let user_cfg = global_config_path().ok().and_then(|p| read_config_lenient(&p));
    let profile_cfg = profile_path.and_then(|p| read_config_lenient(&profile_config_path(p)));
    let spec = config_field("prepare-command")?;
    resolve_config(profile_cfg.as_ref(), user_cfg.as_ref(), spec)
        .and_then(|v| v.as_str().map(str::to_string))
}

/// Devora's built-in default for `terminal.git-shortcuts`.
/// Shared with the Settings Hub registry so the displayed default and the PTY consumer below cannot drift.
pub const DEFAULT_GIT_SHORTCUTS: bool = true;

/// Resolves whether Debi git-shortcut command shims should be placed on the PATH of session shells.
/// Profile-level `terminal.git-shortcuts` wins, then global config, then defaults to `true` (feature enabled).
/// A malformed or unreadable config degrades to the default rather than failing PTY creation.
pub fn get_git_shortcuts_enabled(profile_path: Option<&str>) -> bool {
    if let Some(profile_path) = profile_path {
        let profile_config_path = Path::new(profile_path).join("config.json");
        if profile_config_path.exists() {
            if let Ok(profile_config) = read_json_file(&profile_config_path) {
                if let Some(val) = get_value_by_dot_path(&profile_config, "terminal.git-shortcuts") {
                    if let Some(b) = val.as_bool() {
                        return b;
                    }
                }
            }
        }
    }

    match get_config_value("terminal.git-shortcuts") {
        Ok(Some(val)) => val.as_bool().unwrap_or(DEFAULT_GIT_SHORTCUTS),
        _ => DEFAULT_GIT_SHORTCUTS,
    }
}

// ── Claude Code launch configuration (default model, model tiers, etc.) ──
//
// `ccc` launches Claude Code with various configurations.
// Each is resolved per key with the precedence profile → user → Devora default, where a level may hold an explicit string (a value), JSON `null` (= "None": impose nothing, so Claude Code falls back to its own default), or omit the key (fall through to the next level).
// The resolved values are injected as environment variables by the PTY layer (see commands.rs): the `ANTHROPIC_*` vars are read by Claude Code natively, while `DEVORA_CCC_*` vars are turned into CLI flags by `ccc.sh`.

/// One configurable Claude launch setting: its kebab-case config key (under the top-level `claude` object), the environment variable the PTY exports, and the Devora default value.
struct ClaudeSettingSpec {
    key: &'static str,
    env_var: &'static str,
    default: &'static str,
}

const CLAUDE_SETTINGS: [ClaudeSettingSpec; 6] = [
    ClaudeSettingSpec {
        key: "opus-model",
        env_var: "ANTHROPIC_DEFAULT_OPUS_MODEL",
        default: "claude-opus-4-8",
    },
    ClaudeSettingSpec {
        key: "sonnet-model",
        env_var: "ANTHROPIC_DEFAULT_SONNET_MODEL",
        default: "claude-opus-4-8",
    },
    ClaudeSettingSpec {
        key: "haiku-model",
        env_var: "ANTHROPIC_DEFAULT_HAIKU_MODEL",
        default: "claude-sonnet-5",
    },
    ClaudeSettingSpec {
        key: "effort",
        env_var: "DEVORA_CCC_EFFORT",
        default: "xhigh",
    },
    ClaudeSettingSpec {
        key: "default-model",
        env_var: "ANTHROPIC_MODEL",
        default: "opusplan",
    },
    ClaudeSettingSpec {
        key: "permission-mode",
        env_var: "DEVORA_CCC_PERMISSION_MODE",
        default: "plan",
    },
];

/// Valid Claude Code effort levels (low → max).
/// Mirrored in the Ember UI Effort control (`src/ui/components/ClaudeConfigCard.ts`) — keep the two lists in sync.
pub const CLAUDE_EFFORT_LEVELS: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];

/// One resolved setting: either a concrete value to apply, or None (impose nothing).
enum ResolvedSetting {
    Value(String),
    None,
}

/// Result of `read_claude_settings`: the raw values stored at one scope plus the effective values after full precedence resolution.
/// Both maps are keyed by the kebab config key.
#[derive(Serialize)]
pub struct ClaudeSettingsResponse {
    /// Per key: the raw stored value at this scope — a string, JSON `null`, or the key is omitted entirely (meaning "Default" / not set at this scope).
    stored: serde_json::Map<String, Value>,
    /// Per key (all settings always present): the effective resolved value — a string, or JSON `null` meaning None (no override; Claude Code uses its own default).
    resolved: serde_json::Map<String, Value>,
}

/// Reads a config file, returning `Ok(None)` when it does not exist.
fn read_config_opt(path: &Path) -> Result<Option<Value>, String> {
    if path.exists() {
        Ok(Some(read_json_file(path)?))
    } else {
        Ok(None)
    }
}

/// Best-effort read used on the PTY hot path: a missing or unreadable config degrades to `None` (treated as absent) rather than failing session creation.
fn read_config_lenient(path: &Path) -> Option<Value> {
    if path.exists() {
        read_json_file(path).ok()
    } else {
        None
    }
}

fn profile_config_path(profile_path: &str) -> PathBuf {
    Path::new(profile_path).join("config.json")
}

/// Resolves one setting across profile → user → Devora default.
/// A present non-empty string wins as a value; a present JSON `null` wins as None and stops the chain; an absent key, an empty/whitespace string, or any other malformed type falls through to the next level.
fn resolve_one(
    profile_cfg: Option<&Value>,
    user_cfg: Option<&Value>,
    spec: &ClaudeSettingSpec,
) -> ResolvedSetting {
    let dot_path = format!("claude.{}", spec.key);
    for cfg in [profile_cfg, user_cfg].into_iter().flatten() {
        match get_value_by_dot_path(cfg, &dot_path) {
            Some(Value::String(s)) if !s.trim().is_empty() => {
                return ResolvedSetting::Value(s.clone());
            }
            Some(Value::Null) => return ResolvedSetting::None,
            _ => {} // absent / empty / malformed → fall through
        }
    }
    ResolvedSetting::Value(spec.default.to_string())
}

/// Resolves the Claude launch settings for a session and returns the environment variables the PTY should export.
/// Keys resolved to None are omitted (Claude Code uses its default).
pub fn claude_launch_env(profile_path: Option<&str>) -> HashMap<String, String> {
    let user_cfg = global_config_path()
        .ok()
        .and_then(|p| read_config_lenient(&p));
    let profile_cfg = profile_path.and_then(|p| read_config_lenient(&profile_config_path(p)));

    let mut env = HashMap::new();
    for spec in &CLAUDE_SETTINGS {
        if let ResolvedSetting::Value(v) = resolve_one(profile_cfg.as_ref(), user_cfg.as_ref(), spec)
        {
            env.insert(spec.env_var.to_string(), v);
        }
    }
    env
}

/// Reads the Claude settings for the Settings Hub UI at the given scope (a profile path, or `None` for the user-level/global scope): the raw stored values at that scope plus the effective values after full resolution.
/// A corrupt config surfaces as an error.
pub fn read_claude_settings(profile_path: Option<&str>) -> Result<ClaudeSettingsResponse, String> {
    let user_cfg = read_config_opt(&global_config_path()?)?;
    let profile_cfg = match profile_path {
        Some(p) => read_config_opt(&profile_config_path(p))?,
        None => None,
    };

    // The scope's own config holds the raw "stored" values.
    let own_cfg = if profile_path.is_some() {
        profile_cfg.as_ref()
    } else {
        user_cfg.as_ref()
    };

    let mut stored = serde_json::Map::new();
    if let Some(claude) = own_cfg.and_then(|c| c.get("claude")).and_then(|v| v.as_object()) {
        for spec in &CLAUDE_SETTINGS {
            // Preserve string and null verbatim; ignore malformed types.
            if let Some(val) = claude.get(spec.key).filter(|v| v.is_string() || v.is_null()) {
                stored.insert(spec.key.to_string(), val.clone());
            }
        }
    }

    let mut resolved = serde_json::Map::new();
    for spec in &CLAUDE_SETTINGS {
        let value = match resolve_one(profile_cfg.as_ref(), user_cfg.as_ref(), spec) {
            ResolvedSetting::Value(s) => Value::String(s),
            ResolvedSetting::None => Value::Null,
        };
        resolved.insert(spec.key.to_string(), value);
    }

    Ok(ClaudeSettingsResponse { stored, resolved })
}

/// Writes one Claude setting at the given scope, preserving sibling keys.
/// `state`: `"value"` (store `value` as a string), `"none"` (store JSON null), `"default"` (remove the key so it falls through).
/// Empty/whitespace values are rejected; effort values are validated against `CLAUDE_EFFORT_LEVELS`.
pub fn write_claude_setting(
    profile_path: Option<&str>,
    key: &str,
    state: &str,
    value: Option<&str>,
) -> Result<(), String> {
    let spec = CLAUDE_SETTINGS
        .iter()
        .find(|s| s.key == key)
        .ok_or_else(|| format!("unknown claude setting key: {key}"))?;

    let config_path = match profile_path {
        Some(p) => profile_config_path(p),
        None => global_config_path()?,
    };

    let mut config = read_config_opt(&config_path)?
        .unwrap_or_else(|| Value::Object(serde_json::Map::new()));
    let obj = config
        .as_object_mut()
        .ok_or("config is not a JSON object")?;

    let claude_empty = {
        let claude = obj
            .entry("claude")
            .or_insert_with(|| Value::Object(serde_json::Map::new()));
        let claude_obj = claude
            .as_object_mut()
            .ok_or("\"claude\" is not an object in config")?;

        match state {
            "value" => {
                let v = value
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| format!("a non-empty value is required to set {key}"))?;
                if spec.key == "effort" && !CLAUDE_EFFORT_LEVELS.contains(&v) {
                    return Err(format!(
                        "invalid effort level: {v} (expected one of {})",
                        CLAUDE_EFFORT_LEVELS.join(", ")
                    ));
                }
                claude_obj.insert(key.to_string(), Value::String(v.to_string()));
            }
            "none" => {
                claude_obj.insert(key.to_string(), Value::Null);
            }
            "default" => {
                claude_obj.remove(key);
            }
            other => {
                return Err(format!(
                    "unknown state: {other} (expected value, none, or default)"
                ))
            }
        }

        claude_obj.is_empty()
    };

    // Keep configs tidy: drop the `claude` object once it holds no overrides.
    if claude_empty {
        obj.remove("claude");
    }

    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create {}: {e}", parent.display()))?;
    }
    write_pretty_json(&config_path, &config)
}

// ── Generic config settings (Settings Hub) ──
//
// The config keys (besides Claude) that the Settings Hub edits, resolved per key with the precedence profile → user → Devora built-in.
// Unlike Claude settings, these have no `null`/"None" state: a key is either stored at a scope or absent (inherit).
// Debi (Go) reads the same keys from the same files, so each value must keep its JSON type (a bool stays a bool, not the string "true").

/// A configurable field's value type, which drives validation and JSON encoding.
#[derive(Clone, Copy)]
enum ConfigFieldKind {
    Text,
    /// Text concatenated verbatim onto other text (e.g. a PR title prefix), so surrounding whitespace is significant and kept.
    Fragment,
    Bool,
    Enum(&'static [&'static str]),
}

/// Devora's built-in default for a field, shown as the resolved "Default" hint when nothing is stored at any scope.
/// `None` for keys whose real default lives in Debi (Go) - advertising a value here would duplicate Go's default and risk drift, so the UI shows a generic label instead.
enum BuiltinDefault {
    None,
    Bool(bool),
}

struct ConfigFieldSpec {
    /// Dot-path under the config root (e.g. `terminal.default-app`, `task-tracker.asana.project-id`).
    key: &'static str,
    kind: ConfigFieldKind,
    builtin_default: BuiltinDefault,
}

const CONFIG_FIELDS: &[ConfigFieldSpec] = &[
    ConfigFieldSpec {
        key: "terminal.default-app",
        kind: ConfigFieldKind::Text,
        builtin_default: BuiltinDefault::None, // unset = bare login shell
    },
    ConfigFieldSpec {
        key: "terminal.git-shortcuts",
        kind: ConfigFieldKind::Bool,
        builtin_default: BuiltinDefault::Bool(DEFAULT_GIT_SHORTCUTS),
    },
    ConfigFieldSpec {
        key: "prepare-command",
        kind: ConfigFieldKind::Text,
        builtin_default: BuiltinDefault::None,
    },
    ConfigFieldSpec {
        key: "pr.auto-merge",
        kind: ConfigFieldKind::Bool,
        builtin_default: BuiltinDefault::None, // Debi owns the default (and a per-repo tier exists)
    },
    ConfigFieldSpec {
        key: "pr.title-prefix",
        kind: ConfigFieldKind::Fragment,
        builtin_default: BuiltinDefault::None, // Debi owns the default (no prefix)
    },
    ConfigFieldSpec {
        key: "feature.branch-prefix",
        kind: ConfigFieldKind::Text,
        builtin_default: BuiltinDefault::None,
    },
    ConfigFieldSpec {
        key: "task-tracker.provider",
        kind: ConfigFieldKind::Enum(&["asana"]),
        builtin_default: BuiltinDefault::None,
    },
    ConfigFieldSpec {
        key: "task-tracker.asana.workspace-id",
        kind: ConfigFieldKind::Text,
        builtin_default: BuiltinDefault::None,
    },
    ConfigFieldSpec {
        key: "task-tracker.asana.project-id",
        kind: ConfigFieldKind::Text,
        builtin_default: BuiltinDefault::None,
    },
    ConfigFieldSpec {
        key: "task-tracker.asana.cli-tag",
        kind: ConfigFieldKind::Text,
        builtin_default: BuiltinDefault::None,
    },
    ConfigFieldSpec {
        key: "task-tracker.asana.section-id",
        kind: ConfigFieldKind::Text,
        builtin_default: BuiltinDefault::None,
    },
];

fn config_field(key: &str) -> Option<&'static ConfigFieldSpec> {
    CONFIG_FIELDS.iter().find(|s| s.key == key)
}

fn builtin_default_value(d: &BuiltinDefault) -> Option<Value> {
    match d {
        BuiltinDefault::None => None,
        BuiltinDefault::Bool(b) => Some(Value::Bool(*b)),
    }
}

/// The raw value of `spec.key` stored in one scope's config, coerced to the field's expected JSON type.
/// Returns `None` when the key is absent or stored with a mismatched type.
fn stored_config_value(cfg: Option<&Value>, spec: &ConfigFieldSpec) -> Option<Value> {
    let val = get_value_by_dot_path(cfg?, spec.key)?;
    match spec.kind {
        ConfigFieldKind::Bool => val.as_bool().map(Value::Bool),
        ConfigFieldKind::Text | ConfigFieldKind::Fragment | ConfigFieldKind::Enum(_) => {
            val.as_str().map(|s| Value::String(s.to_string()))
        }
    }
}

/// Resolves `spec.key` across profile → user → Devora built-in.
/// Returns `None` only when the key is unset everywhere and there is no built-in default (a Debi-owned key).
fn resolve_config(
    profile_cfg: Option<&Value>,
    user_cfg: Option<&Value>,
    spec: &ConfigFieldSpec,
) -> Option<Value> {
    stored_config_value(profile_cfg, spec)
        .or_else(|| stored_config_value(user_cfg, spec))
        .or_else(|| builtin_default_value(&spec.builtin_default))
}

/// Result of `read_config_settings`: the raw values stored at one scope plus the effective values after full precedence resolution.
/// Both maps are keyed by the dot-path config key.
#[derive(Serialize)]
pub struct ConfigSettingsResponse {
    /// Per key present at this scope: the raw stored value (a string or bool). Absent keys are omitted, which the UI reads as "Default" (inherit) for that scope.
    stored: serde_json::Map<String, Value>,
    /// Per key (all present): the effective value after profile → user → built-in, or JSON `null` when nothing is set and Devora has no built-in to advertise (a Debi-owned default).
    resolved: serde_json::Map<String, Value>,
}

/// Reads the generic config settings for the Settings Hub at the given scope (a profile path, or `None` for the user-level/global scope): the raw stored values at that scope plus the effective values after full resolution. A corrupt config surfaces as an error.
pub fn read_config_settings(profile_path: Option<&str>) -> Result<ConfigSettingsResponse, String> {
    let user_cfg = read_config_opt(&global_config_path()?)?;
    let profile_cfg = match profile_path {
        Some(p) => read_config_opt(&profile_config_path(p))?,
        None => None,
    };
    let own_cfg = if profile_path.is_some() {
        profile_cfg.as_ref()
    } else {
        user_cfg.as_ref()
    };

    let mut stored = serde_json::Map::new();
    let mut resolved = serde_json::Map::new();
    for spec in CONFIG_FIELDS {
        if let Some(v) = stored_config_value(own_cfg, spec) {
            stored.insert(spec.key.to_string(), v);
        }
        let effective =
            resolve_config(profile_cfg.as_ref(), user_cfg.as_ref(), spec).unwrap_or(Value::Null);
        resolved.insert(spec.key.to_string(), effective);
    }

    Ok(ConfigSettingsResponse { stored, resolved })
}

/// Coerces a UI-supplied string into the field's JSON value, validating the field kind.
/// An empty (or whitespace-only) value is rejected for `Text` and `Fragment` (clear via the `default` state instead) but accepted for `Enum` as the explicit "none" choice (e.g. disabling a tracker inherited from a higher scope).
fn coerce_config_value(spec: &ConfigFieldSpec, value: Option<&str>) -> Result<Value, String> {
    let raw = value.unwrap_or("");
    match spec.kind {
        ConfigFieldKind::Bool => match raw {
            "true" => Ok(Value::Bool(true)),
            "false" => Ok(Value::Bool(false)),
            other => Err(format!("invalid boolean for {}: {other}", spec.key)),
        },
        ConfigFieldKind::Text | ConfigFieldKind::Fragment => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return Err(format!("a non-empty value is required to set {}", spec.key));
            }
            let stored = match spec.kind {
                ConfigFieldKind::Fragment => raw,
                _ => trimmed,
            };
            Ok(Value::String(stored.to_string()))
        }
        ConfigFieldKind::Enum(allowed) => {
            if raw.is_empty() {
                return Ok(Value::String(String::new()));
            }
            if !allowed.contains(&raw) {
                return Err(format!(
                    "invalid value for {}: {raw} (expected one of {})",
                    spec.key,
                    allowed.join(", ")
                ));
            }
            Ok(Value::String(raw.to_string()))
        }
    }
}

/// Writes one generic config setting at the given scope, preserving sibling keys.
/// `state`: `"value"` (store the coerced `value`), `"default"` (remove the key so it falls through).
pub fn write_config_setting(
    profile_path: Option<&str>,
    key: &str,
    state: &str,
    value: Option<&str>,
) -> Result<(), String> {
    let spec = config_field(key).ok_or_else(|| format!("unknown config key: {key}"))?;

    let config_path = match profile_path {
        Some(p) => profile_config_path(p),
        None => global_config_path()?,
    };
    let mut config =
        read_config_opt(&config_path)?.unwrap_or_else(|| Value::Object(serde_json::Map::new()));
    if !config.is_object() {
        return Err("config is not a JSON object".to_string());
    }

    let new_value = match state {
        "value" => Some(coerce_config_value(spec, value)?),
        "default" => None,
        other => {
            return Err(format!(
                "unknown state: {other} (expected value or default)"
            ))
        }
    };
    write_dot_path(&mut config, key, new_value)?;

    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create {}: {e}", parent.display()))?;
    }
    write_pretty_json(&config_path, &config)
}

/// Sets (`Some`) or removes (`None`) a dot-path key in a JSON object, creating intermediate objects on set and pruning intermediate objects left empty after a remove.
fn write_dot_path(root: &mut Value, dot_path: &str, value: Option<Value>) -> Result<(), String> {
    let keys: Vec<&str> = dot_path.split('.').collect();
    set_or_remove_path(root, &keys, value)
}

fn set_or_remove_path(node: &mut Value, keys: &[&str], value: Option<Value>) -> Result<(), String> {
    let obj = node.as_object_mut().ok_or("config is not a JSON object")?;
    let (first, rest) = keys
        .split_first()
        .expect("a dot-path always has at least one segment");

    if rest.is_empty() {
        match value {
            Some(v) => {
                obj.insert(first.to_string(), v);
            }
            None => {
                obj.remove(*first);
            }
        }
        return Ok(());
    }

    match value {
        Some(v) => {
            let child = obj
                .entry(first.to_string())
                .or_insert_with(|| Value::Object(serde_json::Map::new()));
            if !child.is_object() {
                return Err(format!("\"{first}\" is not an object in config"));
            }
            set_or_remove_path(child, rest, Some(v))
        }
        None => {
            // Remove: descend only if the intermediate exists and is an object, then prune it if empty.
            if let Some(child) = obj.get_mut(*first) {
                if child.is_object() {
                    set_or_remove_path(child, rest, None)?;
                    if child.as_object().is_some_and(serde_json::Map::is_empty) {
                        obj.remove(*first);
                    }
                }
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    // Serializes tests that mutate process-global env vars (HOME / DEVORA_CONFIG_PATH) read by `global_config_path`; concurrent mutation would make config reads non-deterministic.
    // `into_inner` recovers from a poisoned lock so one failing test doesn't cascade.
    static ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn test_expand_tilde_with_home() {
        let result = expand_tilde("~/some/path").unwrap();
        let home = std::env::var("HOME").unwrap();
        assert_eq!(result, format!("{home}/some/path"));
    }

    #[test]
    fn test_expand_tilde_without_tilde() {
        let result = expand_tilde("/absolute/path").unwrap();
        assert_eq!(result, "/absolute/path");
    }

    #[test]
    fn test_expand_tilde_bare() {
        let result = expand_tilde("~").unwrap();
        let home = std::env::var("HOME").unwrap();
        assert_eq!(result, home);
    }

    #[test]
    fn test_get_value_by_dot_path() {
        let json: Value = serde_json::json!({
            "terminal": {
                "default-app": "nvim"
            }
        });
        let val = get_value_by_dot_path(&json, "terminal.default-app");
        assert_eq!(val.unwrap().as_str().unwrap(), "nvim");
    }

    #[test]
    fn test_get_value_by_dot_path_missing() {
        let json: Value = serde_json::json!({"a": 1});
        let val = get_value_by_dot_path(&json, "b.c");
        assert!(val.is_none());
    }

    #[test]
    fn test_global_config_path_override() {
        // Serialize with other tests that read/write DEVORA_CONFIG_PATH - this test mutates that shared env var, so without the lock it can clobber a concurrent test's config path.
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let original = std::env::var("DEVORA_CONFIG_PATH").ok();
        std::env::set_var("DEVORA_CONFIG_PATH", "/tmp/test-config.json");
        let path = global_config_path().unwrap();
        assert_eq!(path, PathBuf::from("/tmp/test-config.json"));
        match original {
            Some(v) => std::env::set_var("DEVORA_CONFIG_PATH", v),
            None => std::env::remove_var("DEVORA_CONFIG_PATH"),
        }
    }

    #[test]
    fn test_get_git_shortcuts_enabled() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let original = std::env::var("DEVORA_CONFIG_PATH").ok();

        // Point the global config at a guaranteed-absent file so global resolution is deterministic (returns None -> default true).
        let tmp = tempfile::tempdir().unwrap();
        let global_config = tmp.path().join("global-config.json");
        std::env::set_var("DEVORA_CONFIG_PATH", &global_config);

        // No profile, no global config file -> default true.
        assert!(get_git_shortcuts_enabled(None));

        // Global config disables it (no profile override present).
        fs::write(&global_config, r#"{"terminal":{"git-shortcuts":false}}"#).unwrap();
        assert!(!get_git_shortcuts_enabled(None));

        // Profile override wins over global.
        let profile_dir = tmp.path().join("profile");
        fs::create_dir_all(&profile_dir).unwrap();
        fs::write(
            profile_dir.join("config.json"),
            r#"{"terminal":{"git-shortcuts":true}}"#,
        )
        .unwrap();
        let profile_path = profile_dir.to_string_lossy().to_string();
        assert!(get_git_shortcuts_enabled(Some(&profile_path)));

        // Profile with the key absent falls through to global (false here).
        fs::write(profile_dir.join("config.json"), r#"{"name":"p"}"#).unwrap();
        assert!(!get_git_shortcuts_enabled(Some(&profile_path)));

        match original {
            Some(v) => std::env::set_var("DEVORA_CONFIG_PATH", v),
            None => std::env::remove_var("DEVORA_CONFIG_PATH"),
        }
    }

    #[test]
    fn test_claude_effort_levels_are_canonical() {
        assert_eq!(CLAUDE_EFFORT_LEVELS, ["low", "medium", "high", "xhigh", "max"]);
    }

    #[test]
    fn test_claude_settings_resolution_and_write() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let original = std::env::var("DEVORA_CONFIG_PATH").ok();
        let tmp = tempfile::tempdir().unwrap();
        let global_config = tmp.path().join("global-config.json");
        std::env::set_var("DEVORA_CONFIG_PATH", &global_config);

        let opus = "ANTHROPIC_DEFAULT_OPUS_MODEL";
        let sonnet = "ANTHROPIC_DEFAULT_SONNET_MODEL";
        let haiku = "ANTHROPIC_DEFAULT_HAIKU_MODEL";
        let effort = "DEVORA_CCC_EFFORT";
        let default_model = "ANTHROPIC_MODEL";
        let permission_mode = "DEVORA_CCC_PERMISSION_MODE";
        let get = |env: &HashMap<String, String>, k: &str| env.get(k).cloned();

        // 1. No config anywhere -> all Devora defaults are set.
        let env = claude_launch_env(None);
        assert_eq!(get(&env, opus).as_deref(), Some("claude-opus-4-8"));
        assert_eq!(get(&env, sonnet).as_deref(), Some("claude-opus-4-8"));
        assert_eq!(get(&env, haiku).as_deref(), Some("claude-sonnet-5"));
        assert_eq!(get(&env, effort).as_deref(), Some("xhigh"));
        assert_eq!(get(&env, default_model).as_deref(), Some("opusplan"));
        assert_eq!(get(&env, permission_mode).as_deref(), Some("plan"));

        // 1b. The default model (None -> omitted) and permission mode (any free-text value, unvalidated) resolve like the other keys.
        fs::write(
            &global_config,
            r#"{"claude":{"default-model":null,"permission-mode":"acceptEdits"}}"#,
        )
        .unwrap();
        let env = claude_launch_env(None);
        assert!(!env.contains_key(default_model)); // null -> None -> omitted
        assert_eq!(get(&env, permission_mode).as_deref(), Some("acceptEdits"));

        // 2. User-level: a value wins; null = None (env var omitted); absent = default.
        fs::write(
            &global_config,
            r#"{"claude":{"opus-model":"claude-fable-5","haiku-model":null}}"#,
        )
        .unwrap();
        let env = claude_launch_env(None);
        assert_eq!(get(&env, opus).as_deref(), Some("claude-fable-5"));
        assert_eq!(get(&env, sonnet).as_deref(), Some("claude-opus-4-8"));
        assert!(!env.contains_key(haiku)); // null -> None -> omitted
        assert_eq!(get(&env, effort).as_deref(), Some("xhigh"));

        // 3. Profile overrides user per key; profile-absent keys fall through to user.
        let profile_dir = tmp.path().join("profile");
        fs::create_dir_all(&profile_dir).unwrap();
        let profile_path = profile_dir.to_string_lossy().to_string();
        fs::write(
            profile_dir.join("config.json"),
            r#"{"name":"p","claude":{"opus-model":null,"effort":"max"}}"#,
        )
        .unwrap();
        let env = claude_launch_env(Some(&profile_path));
        assert!(!env.contains_key(opus)); // profile null wins -> None
        assert_eq!(get(&env, effort).as_deref(), Some("max"));
        assert!(!env.contains_key(haiku)); // profile absent -> user null -> None
        assert_eq!(get(&env, sonnet).as_deref(), Some("claude-opus-4-8")); // both absent -> default

        // 4. Malformed values (wrong type, blank string) fall through to the default.
        fs::write(
            &global_config,
            r#"{"claude":{"opus-model":42,"effort":"   "}}"#,
        )
        .unwrap();
        let env = claude_launch_env(None);
        assert_eq!(get(&env, opus).as_deref(), Some("claude-opus-4-8"));
        assert_eq!(get(&env, effort).as_deref(), Some("xhigh"));

        // 5. read_claude_settings: `stored` distinguishes value / null / absent; `resolved` gives the effective value (string or null=None).
        fs::write(
            &global_config,
            r#"{"profiles":[],"claude":{"opus-model":"claude-fable-5","haiku-model":null}}"#,
        )
        .unwrap();
        let settings = read_claude_settings(None).unwrap();
        assert_eq!(
            settings.stored.get("opus-model"),
            Some(&Value::String("claude-fable-5".into()))
        );
        assert_eq!(settings.stored.get("haiku-model"), Some(&Value::Null));
        assert!(!settings.stored.contains_key("sonnet-model")); // absent
        assert_eq!(
            settings.resolved.get("opus-model"),
            Some(&Value::String("claude-fable-5".into()))
        );
        assert_eq!(
            settings.resolved.get("sonnet-model"),
            Some(&Value::String("claude-opus-4-8".into())) // default
        );
        assert_eq!(settings.resolved.get("haiku-model"), Some(&Value::Null)); // None

        // 6. write_claude_setting round-trips value/none/default and preserves siblings.
        write_claude_setting(None, "sonnet-model", "value", Some("claude-opus-4-7")).unwrap();
        let cfg = read_json_file(&global_config).unwrap();
        assert_eq!(cfg["claude"]["sonnet-model"], Value::String("claude-opus-4-7".into()));
        assert_eq!(cfg["claude"]["opus-model"], Value::String("claude-fable-5".into())); // sibling kept
        assert_eq!(cfg["profiles"], serde_json::json!([])); // unrelated key kept

        write_claude_setting(None, "opus-model", "none", None).unwrap();
        assert_eq!(read_json_file(&global_config).unwrap()["claude"]["opus-model"], Value::Null);

        write_claude_setting(None, "sonnet-model", "default", None).unwrap();
        assert!(read_json_file(&global_config).unwrap()["claude"]
            .get("sonnet-model")
            .is_none());

        // 7. Validation: blank value, bad effort, and unknown key are rejected.
        assert!(write_claude_setting(None, "opus-model", "value", Some("   ")).is_err());
        assert!(write_claude_setting(None, "effort", "value", Some("turbo")).is_err());
        assert!(write_claude_setting(None, "effort", "value", Some("max")).is_ok());
        assert!(write_claude_setting(None, "unknown-key", "value", Some("x")).is_err());

        // 8. Permission mode and default model are free-text (only effort is a closed set): any non-blank value is accepted, and `none` round-trips.
        write_claude_setting(None, "permission-mode", "value", Some("bypassPermissions")).unwrap();
        assert_eq!(
            read_json_file(&global_config).unwrap()["claude"]["permission-mode"],
            Value::String("bypassPermissions".into())
        );
        write_claude_setting(None, "default-model", "none", None).unwrap();
        assert_eq!(
            read_json_file(&global_config).unwrap()["claude"]["default-model"],
            Value::Null
        );

        match original {
            Some(v) => std::env::set_var("DEVORA_CONFIG_PATH", v),
            None => std::env::remove_var("DEVORA_CONFIG_PATH"),
        }
    }

    #[test]
    fn test_write_dot_path_set_remove_prune() {
        let mut cfg = serde_json::json!({ "name": "p" });

        // Set a deeply-nested leaf, creating intermediate objects; siblings untouched.
        write_dot_path(
            &mut cfg,
            "task-tracker.asana.project-id",
            Some(Value::String("99".into())),
        )
        .unwrap();
        assert_eq!(cfg["task-tracker"]["asana"]["project-id"], Value::String("99".into()));
        assert_eq!(cfg["name"], Value::String("p".into()));

        // A second leaf under the same branch + a top-level bool branch.
        write_dot_path(&mut cfg, "task-tracker.provider", Some(Value::String("asana".into()))).unwrap();
        write_dot_path(&mut cfg, "pr.auto-merge", Some(Value::Bool(false))).unwrap();
        assert_eq!(cfg["pr"]["auto-merge"], Value::Bool(false));

        // Removing the deep leaf prunes the now-empty `asana` object but keeps `task-tracker` (still has provider).
        write_dot_path(&mut cfg, "task-tracker.asana.project-id", None).unwrap();
        assert!(cfg["task-tracker"].get("asana").is_none());
        assert_eq!(cfg["task-tracker"]["provider"], Value::String("asana".into()));

        // Removing the last leaf prunes the whole branch.
        write_dot_path(&mut cfg, "task-tracker.provider", None).unwrap();
        assert!(cfg.get("task-tracker").is_none());

        // Removing an absent key is a no-op (no empty branch created).
        write_dot_path(&mut cfg, "feature.branch-prefix", None).unwrap();
        assert!(cfg.get("feature").is_none());
    }

    #[test]
    fn test_config_settings_resolution_and_write() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let original = std::env::var("DEVORA_CONFIG_PATH").ok();
        let tmp = tempfile::tempdir().unwrap();
        let global_config = tmp.path().join("global-config.json");
        std::env::set_var("DEVORA_CONFIG_PATH", &global_config);

        // 1. Nothing stored: resolved uses built-ins (git-shortcuts true), null for Debi-owned keys.
        let s = read_config_settings(None).unwrap();
        assert!(s.stored.is_empty());
        assert_eq!(s.resolved.get("terminal.git-shortcuts"), Some(&Value::Bool(true)));
        assert_eq!(s.resolved.get("terminal.default-app"), Some(&Value::Null));
        assert_eq!(s.resolved.get("pr.auto-merge"), Some(&Value::Null));
        assert_eq!(s.resolved.get("feature.branch-prefix"), Some(&Value::Null));

        // 2. Write at the user scope: a bool stays a JSON bool, a string stays a string.
        write_config_setting(None, "terminal.git-shortcuts", "value", Some("false")).unwrap();
        write_config_setting(None, "feature.branch-prefix", "value", Some("feat")).unwrap();
        write_config_setting(None, "terminal.default-app", "value", Some("nvim")).unwrap();
        let cfg = read_json_file(&global_config).unwrap();
        assert_eq!(cfg["terminal"]["git-shortcuts"], Value::Bool(false));
        assert_eq!(cfg["feature"]["branch-prefix"], Value::String("feat".into()));
        assert_eq!(cfg["terminal"]["default-app"], Value::String("nvim".into()));
        let s = read_config_settings(None).unwrap();
        assert_eq!(s.stored.get("terminal.git-shortcuts"), Some(&Value::Bool(false)));
        assert_eq!(s.resolved.get("terminal.git-shortcuts"), Some(&Value::Bool(false)));

        // A text value is trimmed; a fragment keeps its whitespace.
        write_config_setting(None, "terminal.default-app", "value", Some(" nvim ")).unwrap();
        write_config_setting(None, "pr.title-prefix", "value", Some("[ENG] ")).unwrap();
        let cfg = read_json_file(&global_config).unwrap();
        assert_eq!(cfg["terminal"]["default-app"], Value::String("nvim".into()));
        assert_eq!(cfg["pr"]["title-prefix"], Value::String("[ENG] ".into()));

        // 3. Profile overrides user per key; profile-absent keys fall through to user.
        let profile_dir = tmp.path().join("profile");
        fs::create_dir_all(&profile_dir).unwrap();
        let profile_path = profile_dir.to_string_lossy().to_string();
        fs::write(profile_dir.join("config.json"), r#"{"name":"p"}"#).unwrap();
        write_config_setting(Some(&profile_path), "feature.branch-prefix", "value", Some("hotfix")).unwrap();
        let s = read_config_settings(Some(&profile_path)).unwrap();
        assert_eq!(s.stored.get("feature.branch-prefix"), Some(&Value::String("hotfix".into())));
        assert_eq!(s.resolved.get("feature.branch-prefix"), Some(&Value::String("hotfix".into())));
        assert!(!s.stored.contains_key("terminal.git-shortcuts")); // absent at profile
        assert_eq!(s.resolved.get("terminal.git-shortcuts"), Some(&Value::Bool(false))); // from user
        // get_default_app routes through the same resolver: profile unset → user "nvim".
        assert_eq!(get_default_app(&profile_path).unwrap().as_deref(), Some("nvim"));

        // 4. Enum: empty value = explicit "none"; a valid value accepted; an invalid one rejected.
        write_config_setting(Some(&profile_path), "task-tracker.provider", "value", Some("asana")).unwrap();
        assert_eq!(
            read_json_file(&profile_dir.join("config.json")).unwrap()["task-tracker"]["provider"],
            Value::String("asana".into())
        );
        write_config_setting(Some(&profile_path), "task-tracker.provider", "value", Some("")).unwrap();
        assert_eq!(
            read_json_file(&profile_dir.join("config.json")).unwrap()["task-tracker"]["provider"],
            Value::String(String::new())
        );
        assert!(write_config_setting(Some(&profile_path), "task-tracker.provider", "value", Some("jira")).is_err());

        // 5. `default` removes the key and prunes empties; bad inputs are rejected.
        write_config_setting(Some(&profile_path), "task-tracker.provider", "default", None).unwrap();
        assert!(read_json_file(&profile_dir.join("config.json")).unwrap().get("task-tracker").is_none());
        assert!(write_config_setting(None, "feature.branch-prefix", "value", Some("   ")).is_err());
        assert!(write_config_setting(None, "pr.title-prefix", "value", Some("   ")).is_err());
        assert!(write_config_setting(None, "unknown.key", "value", Some("x")).is_err());
        assert!(write_config_setting(None, "pr.auto-merge", "bogus", None).is_err());
        assert!(write_config_setting(None, "pr.auto-merge", "value", Some("yes")).is_err());

        // 6. Bool write round-trips and reads back typed.
        write_config_setting(None, "pr.auto-merge", "value", Some("true")).unwrap();
        assert_eq!(
            read_config_settings(None).unwrap().resolved.get("pr.auto-merge"),
            Some(&Value::Bool(true))
        );

        match original {
            Some(v) => std::env::set_var("DEVORA_CONFIG_PATH", v),
            None => std::env::remove_var("DEVORA_CONFIG_PATH"),
        }
    }
}

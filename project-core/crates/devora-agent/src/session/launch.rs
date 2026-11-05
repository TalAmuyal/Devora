use crate::LaunchSpec;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Inline instructions chosen by the caller. Normal Claude settings and memory
/// still load in every form, as ADR-012 requires.
#[derive(Clone, Debug)]
pub enum Instructions {
    Append(String),
    Replace(String),
    Agent { name: String, definition: Value },
}

/// One explicitly selected plugin. Naming it lets the runtime leave Judgess out
/// while loading the other caller-selected plugins.
#[derive(Clone, Debug)]
pub struct Plugin {
    pub name: String,
    pub directory: PathBuf,
}

/// The caller's saved session identity and transcript state. A session paused
/// before its first result reuses its ID with --session-id instead of --resume.
#[derive(Clone, Debug)]
pub struct Resume {
    pub session_id: String,
    pub has_conversation: bool,
}

/// Everything needed to start or resume a session. The runtime reads no process
/// environment. Claude also loads its normal settings and memory sources.
///
/// Put the space's configured tool environment in `settings.env`. Inline settings
/// give those values priority over user/project settings. Put platform, auth and
/// proxy transport values (for example HOME, PATH, ANTHROPIC_API_KEY,
/// CLAUDE_CODE_OAUTH_TOKEN and HTTPS_PROXY) in `environment`. They replace the
/// inherited process environment and stay out of argv. Claude may override these
/// transport values from the user's own settings.env, which is intentional.
#[derive(Clone, Debug)]
pub struct SessionOptions {
    pub program: PathBuf,
    pub working_directory: PathBuf,
    pub instructions: Instructions,
    pub model: String,
    pub effort: String,
    pub permission_mode: String,
    pub plugins: Vec<Plugin>,
    /// Explicit behavioral settings, including authoritative configured tool env.
    pub settings: Value,
    /// Scrubbed platform/auth/proxy transport env, deliberately kept out of argv.
    pub environment: BTreeMap<String, String>,
    pub resume: Option<Resume>,
}

impl SessionOptions {
    /// Resolve the persistent-process command without reading environment or
    /// configuration files. The resulting environment replaces the inherited one.
    pub(crate) fn launch(
        &self,
        session_id: &str,
        resume: bool,
        model: &str,
        permission_mode: &str,
    ) -> Result<LaunchSpec, String> {
        uuid::Uuid::parse_str(session_id)
            .map_err(|error| format!("invalid session ID: {error}"))?;
        if model.is_empty() || self.effort.is_empty() || permission_mode.is_empty() {
            return Err("model, effort and permission mode must be supplied by the caller".into());
        }
        let mut settings = self
            .settings
            .as_object()
            .cloned()
            .ok_or("settings must be a JSON object")?;
        let configured = settings.entry("env").or_insert_with(|| json!({}));
        let configured = configured
            .as_object_mut()
            .ok_or("settings.env must be a JSON object")?;
        // Both are explicit session policy, and inline settings win over user/project values.
        configured.insert("DISABLE_AUTO_COMPACT".into(), json!("1"));
        let mut environment = self.environment.clone();
        environment.insert("DISABLE_AUTO_COMPACT".into(), "1".into());
        for key in ["CLAUDE_CODE_SESSION_ID", "CLAUDE_CODE_PARENT_SESSION_ID"] {
            if environment.contains_key(key) || configured.contains_key(key) {
                return Err(format!(
                    "{key} is parent session identity, not session configuration"
                ));
            }
        }
        let mut args = [
            "-p",
            "--input-format",
            "stream-json",
            "--output-format",
            "stream-json",
            "--verbose",
            "--include-partial-messages",
            "--permission-prompt-tool",
            "stdio",
            "--permission-mode",
            permission_mode,
            "--model",
            model,
            "--effort",
            &self.effort,
        ]
        .into_iter()
        .map(Into::into)
        .collect::<Vec<_>>();
        args.extend([
            "--settings".into(),
            serde_json::to_string(&settings)
                .map_err(|error| error.to_string())?
                .into(),
        ]);
        args.extend([
            if resume { "--resume" } else { "--session-id" }.into(),
            session_id.into(),
        ]);
        match &self.instructions {
            Instructions::Append(text) => {
                args.extend(["--append-system-prompt".into(), text.into()])
            }
            Instructions::Replace(text) => args.extend(["--system-prompt".into(), text.into()]),
            Instructions::Agent { name, definition } => {
                if name.is_empty() || !definition.is_object() {
                    return Err(
                        "an agent instruction needs a name and a JSON object definition".into(),
                    );
                }
                let definitions = Map::from_iter([(name.clone(), definition.clone())]);
                args.extend([
                    "--agents".into(),
                    serde_json::to_string(&definitions)
                        .map_err(|error| error.to_string())?
                        .into(),
                    "--agent".into(),
                    name.into(),
                ]);
            }
        }
        for plugin in self
            .plugins
            .iter()
            .filter(|plugin| plugin.name != "judgess")
        {
            args.extend(["--plugin-dir".into(), plugin.directory.as_os_str().into()]);
        }
        Ok(LaunchSpec {
            args,
            env: environment.into_iter().collect(),
        })
    }
}

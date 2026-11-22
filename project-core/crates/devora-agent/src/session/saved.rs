use super::SessionOptions;
use serde_json::Value;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

/// Whether Claude's saved session holds the user turn sent with `id`: the CLI
/// keeps it as that entry's `uuid`. This is the only evidence that the CLI
/// accepted a turn, since it writes the session a moment after starting it and
/// nothing on its output marks the write.
pub fn saved_turn(options: &SessionOptions, session_id: &str, id: &str) -> Result<bool, String> {
    let mut found = false;
    visit(options, session_id, |entry| {
        found = entry["type"] == "user" && entry["uuid"] == id;
        found
    })?;
    Ok(found)
}

/// The model of the latest reply in the agent's own conversation, as Claude's
/// saved session keeps it: the model that answered, which no control request
/// reports (they give the one set). A subagent's replies are saved apart, and
/// marked as a sidechain. None when the session or a reply in it isn't saved.
pub fn saved_model(options: &SessionOptions, session_id: &str) -> Result<Option<String>, String> {
    let mut model = None;
    visit(options, session_id, |entry| {
        if entry["type"] == "assistant" && entry["isSidechain"] != true {
            if let Some(answered) = entry["message"]["model"]
                .as_str()
                .filter(|m| crate::protocol::is_model(m))
            {
                model = Some(answered.to_owned());
            }
        }
        false
    })?;
    Ok(model)
}

/// Shows `each` the entries of Claude's saved session `session_id` in order, until it returns true. The session lives
/// under the CLI's config directory (`CLAUDE_CONFIG_DIR`, else `HOME/.claude`) in its launch environment, in a project
/// directory named after the working directory, which is found by the session's file rather than re-derived. A missing
/// session shows nothing.
fn visit(
    options: &SessionOptions,
    session_id: &str,
    mut each: impl FnMut(&Value) -> bool,
) -> Result<(), String> {
    let config = match options.environment.get("CLAUDE_CONFIG_DIR") {
        Some(directory) => PathBuf::from(directory),
        None => PathBuf::from(
            options
                .environment
                .get("HOME")
                .ok_or("the launch environment has no HOME")?,
        )
        .join(".claude"),
    };
    let projects = match std::fs::read_dir(config.join("projects")) {
        Ok(projects) => projects,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("Claude's saved sessions: {error}")),
    };
    for project in projects {
        let path = project
            .map_err(|error| format!("Claude's saved sessions: {error}"))?
            .path()
            .join(format!("{session_id}.jsonl"));
        let file = match std::fs::File::open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(format!("Claude's saved session: {error}")),
        };
        for line in BufReader::new(file).lines() {
            let line = line.map_err(|error| format!("Claude's saved session: {error}"))?;
            // A line cut short by a crash mid-write holds nothing accepted.
            let Ok(entry) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if each(&entry) {
                return Ok(());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Instructions;
    use serde_json::json;
    use std::collections::BTreeMap;

    /// A launch whose config directory holds `lines` as session `s`'s saved file.
    fn saved(lines: &[Value]) -> (tempfile::TempDir, SessionOptions) {
        let config = tempfile::tempdir().unwrap();
        let project = config.path().join("projects/-work");
        std::fs::create_dir_all(&project).unwrap();
        let text: Vec<String> = lines.iter().map(Value::to_string).collect();
        std::fs::write(project.join("s.jsonl"), text.join("\n") + "\n").unwrap();
        let options = SessionOptions {
            program: "claude".into(),
            working_directory: "/work".into(),
            instructions: Instructions::Append(String::new()),
            model: "opus".into(),
            effort: None,
            permission_mode: "default".into(),
            plugins: vec![],
            settings: json!({}),
            environment: BTreeMap::from([(
                "CLAUDE_CONFIG_DIR".into(),
                config.path().display().to_string(),
            )]),
            resume: None,
        };
        (config, options)
    }

    #[test]
    fn the_saved_model_is_the_latest_reply_that_a_model_wrote_in_the_agents_own_conversation() {
        let (_config, options) = saved(&[
            json!({"type":"user","uuid":"u1","message":{"role":"user","content":"hi"}}),
            json!({"type":"assistant","isSidechain":false,"message":{"model":"claude-opus-5-5"}}),
            json!({"type":"assistant","isSidechain":false,"message":{"model":"claude-sonnet-5-5"}}),
            json!({"type":"assistant","isSidechain":true,"message":{"model":"claude-haiku-5-5"}}),
            json!({"type":"assistant","isSidechain":false,"message":{"model":"<synthetic>"}}),
        ]);
        assert_eq!(
            saved_model(&options, "s").unwrap().as_deref(),
            Some("claude-sonnet-5-5")
        );
        assert!(saved_turn(&options, "s", "u1").unwrap());
        assert!(!saved_turn(&options, "s", "u2").unwrap());
    }

    #[test]
    fn a_session_with_no_saved_file_or_reply_has_no_saved_model() {
        let (_config, options) = saved(&[json!({"type":"user","uuid":"u1"})]);
        assert_eq!(saved_model(&options, "s").unwrap(), None);
        assert_eq!(saved_model(&options, "other").unwrap(), None);
    }
}

use super::SessionOptions;
use serde_json::Value;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

/// Whether Claude's saved session holds the user turn sent with `id`: the CLI
/// keeps it as that entry's `uuid`. This is the only evidence that the CLI
/// accepted a turn, since it writes the session a moment after starting it and
/// nothing on its output marks the write. The session lives under the CLI's
/// config directory (`CLAUDE_CONFIG_DIR`, else `HOME/.claude`) in its launch
/// environment, in a project directory named after the working directory,
/// which is found by the session's file rather than re-derived.
pub fn saved_turn(options: &SessionOptions, session_id: &str, id: &str) -> Result<bool, String> {
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
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
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
            if entry["type"] == "user" && entry["uuid"] == id {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

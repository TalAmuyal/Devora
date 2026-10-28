use serde_json::Value;

use crate::bash;
use crate::model::{Request, RuleId, Verdict};
use crate::rules;

/// Pure: the verdict depends only on the request.
pub fn decide(request: &Request) -> Verdict {
    let input = &request.tool_input;
    match request.tool_name.as_str() {
        "Bash" => match string_field(input, "command") {
            Some(command) => bash::decide(command),
            None => Verdict::unknown("Bash request without a command"),
        },
        "WebFetch" => web_fetch(input),
        "WebSearch" => Verdict::Approve,
        "Read" => read_only_file_tool(input, "file_path"),
        "Grep" | "Glob" => read_only_file_tool(input, "path"),
        "Edit" => Verdict::known(RuleId::ToolEdit),
        "Write" => Verdict::known(RuleId::ToolWrite),
        "AskUserQuestion" => Verdict::known(RuleId::ToolAskUserQuestion),
        other => Verdict::unknown(format!("unhandled tool {other}")),
    }
}

fn string_field<'a>(input: &'a Value, name: &str) -> Option<&'a str> {
    input.get(name)?.as_str()
}

/// Claude Code asks only for paths outside the workspace; of those, the temp directories are approved.
fn read_only_file_tool(input: &Value, field: &str) -> Verdict {
    let Some(path) = string_field(input, field) else {
        return Verdict::unknown(format!("read-only file tool without a {field}"));
    };
    let in_temp_dir = rules::TEMP_DIRS.iter().any(|dir| path.starts_with(dir));
    if in_temp_dir && !path.split('/').any(|segment| segment == "..") {
        Verdict::Approve
    } else {
        Verdict::unknown("read outside the temp directories")
    }
}

fn web_fetch(input: &Value) -> Verdict {
    let Some(url) = string_field(input, "url") else {
        return Verdict::unknown("WebFetch without a url");
    };
    let scheme = url
        .split_once(':')
        .map_or("", |(scheme, _)| scheme)
        .to_ascii_lowercase();
    match scheme.as_str() {
        "file" => Verdict::Deny {
            rule: RuleId::WebfetchFileUseRead,
        },
        "http" | "https" => Verdict::known(RuleId::WebfetchHttp),
        _ => Verdict::unknown(format!("WebFetch with scheme '{scheme}'")),
    }
}

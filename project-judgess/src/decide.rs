use serde_json::Value;

use crate::bash;
use crate::model::{Request, RuleId, Verdict};

/// Pure: the verdict depends only on the request.
pub fn decide(request: &Request) -> Verdict {
    let input = &request.tool_input;
    match request.tool_name.as_str() {
        "Bash" => match string_field(input, "command") {
            Some(command) => bash::decide(command),
            None => Verdict::unknown("Bash request without a command"),
        },
        "Read" => read(input, request.home.as_deref()),
        "WebFetch" => web_fetch(input),
        "Edit" => Verdict::known(RuleId::ToolEdit),
        "Write" => Verdict::known(RuleId::ToolWrite),
        "AskUserQuestion" => Verdict::known(RuleId::ToolAskUserQuestion),
        other => Verdict::unknown(format!("unhandled tool {other}")),
    }
}

fn string_field<'a>(input: &'a Value, name: &str) -> Option<&'a str> {
    input.get(name)?.as_str()
}

/// Crit's plan and review files are written by Devora's own tooling for the agent to read.
fn read(input: &Value, home: Option<&str>) -> Verdict {
    let (Some(path), Some(home)) = (string_field(input, "file_path"), home) else {
        return Verdict::unknown("Read without a path or home directory");
    };
    let in_crit_dir = ["plans", "reviews"]
        .iter()
        .any(|dir| path.starts_with(&format!("{home}/.crit/{dir}/")));
    if in_crit_dir && !path.split('/').any(|segment| segment == "..") {
        Verdict::Approve
    } else {
        Verdict::unknown("Read outside the crit directories")
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

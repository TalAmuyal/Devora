//! `judgess classify`: label the requests the hook could not decide, so they can later become rules and cases.

use std::collections::HashSet;
use std::io::{self, BufRead, Write};

use serde_json::{Value, json};

use crate::cases::decision_input;
use crate::decide;
use crate::model::{Request, Verdict, VerdictKind};
use crate::store::{self, Store};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Label {
    Approve,
    Deny,
    Defer,
    Abstain,
    /// Not worth a rule; hide it from the queue for good.
    Delete,
}

impl Label {
    pub fn as_str(self) -> &'static str {
        match self {
            Label::Approve => "approve",
            Label::Deny => "deny",
            Label::Defer => "defer",
            Label::Abstain => "abstain",
            Label::Delete => "delete",
        }
    }

    pub fn parse(text: &str) -> Option<Label> {
        [
            Label::Approve,
            Label::Deny,
            Label::Defer,
            Label::Abstain,
            Label::Delete,
        ]
        .into_iter()
        .find(|label| label.as_str() == text)
    }

    /// The verdict kind that satisfies this label.
    pub fn expected_kind(self) -> Option<VerdictKind> {
        match self {
            Label::Approve => Some(VerdictKind::Approve),
            Label::Deny => Some(VerdictKind::Deny),
            Label::Defer => Some(VerdictKind::Defer),
            Label::Abstain => Some(VerdictKind::AbstainKnown),
            Label::Delete => None,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Answer {
    Label(Label),
    Skip,
    Quit,
    Invalid,
}

pub fn parse_answer(line: &str) -> Answer {
    match line.trim().to_ascii_lowercase().as_str() {
        "approve" | "a" => Answer::Label(Label::Approve),
        "deny" | "d" => Answer::Label(Label::Deny),
        "defer" | "f" => Answer::Label(Label::Defer),
        "abstain" | "b" => Answer::Label(Label::Abstain),
        "delete" | "x" => Answer::Label(Label::Delete),
        "skip" | "s" | "" => Answer::Skip,
        "quit" | "q" => Answer::Quit,
        _ => Answer::Invalid,
    }
}

/// Identity of a request for deduplication: the tool and what its verdict can depend on.
pub fn case_key(raw: &Value) -> String {
    let tool = raw
        .get("tool_name")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let input = raw.get("tool_input").unwrap_or(&Value::Null);
    json!([tool, decision_input(tool, input)]).to_string()
}

/// Queued requests that still need a label: deduplicated, not yet labelled, and still not decidable.
pub fn pending(store: &Store, home: Option<&str>) -> Vec<(Value, Verdict)> {
    let mut seen: HashSet<String> = store
        .read(store::LABELLED)
        .iter()
        .filter_map(|entry| entry.get("request"))
        .map(case_key)
        .collect();
    let mut pending = Vec::new();
    for raw in store.read(store::UNKNOWN) {
        let Some(request) = Request::from_raw(&raw, home) else {
            continue;
        };
        if !seen.insert(case_key(&raw)) {
            continue;
        }
        let verdict = decide(&request);
        if verdict.kind() == VerdictKind::AbstainUnknown {
            pending.push((raw, verdict));
        }
    }
    pending
}

pub fn describe(raw: &Value) -> String {
    let input = raw.get("tool_input").unwrap_or(&Value::Null);
    let main = input
        .get("command")
        .or_else(|| input.get("file_path"))
        .or_else(|| input.get("url"))
        .and_then(Value::as_str)
        .map_or_else(|| input.to_string(), str::to_string);
    format!(
        "tool: {}\ncwd:  {}\n{}",
        raw.get("tool_name").and_then(Value::as_str).unwrap_or("?"),
        raw.get("cwd").and_then(Value::as_str).unwrap_or("?"),
        main
    )
}

pub fn run(
    store: &Store,
    home: Option<&str>,
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> io::Result<()> {
    let queue = pending(store, home);
    if queue.is_empty() {
        writeln!(output, "Nothing to classify.")?;
        return Ok(());
    }
    let total = queue.len();
    for (index, (raw, verdict)) in queue.into_iter().enumerate() {
        writeln!(
            output,
            "\n[{}/{total}] {}\nverdict: {verdict:?}",
            index + 1,
            describe(&raw)
        )?;
        let label = loop {
            write!(
                output,
                "approve/deny/defer/abstain/delete/skip/quit (a/d/f/b/x/s/q)? "
            )?;
            output.flush()?;
            let mut line = String::new();
            if input.read_line(&mut line)? == 0 {
                return Ok(());
            }
            match parse_answer(&line) {
                Answer::Label(label) => break Some(label),
                Answer::Skip => break None,
                Answer::Quit => return Ok(()),
                Answer::Invalid => writeln!(output, "?")?,
            }
        };
        let Some(label) = label else { continue };
        let mut note = String::new();
        if label != Label::Delete {
            write!(output, "note (optional): ")?;
            output.flush()?;
            input.read_line(&mut note)?;
        }
        let mut entry = json!({ "request": raw, "label": label.as_str() });
        if !note.trim().is_empty() {
            entry["note"] = note.trim().into();
        }
        store.append(store::LABELLED, &entry)?;
    }
    Ok(())
}

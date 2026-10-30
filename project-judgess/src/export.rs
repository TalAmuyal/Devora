//! `judgess export`: move labelled cases into the repo's cases file, redacted.

use std::io::{self, Write};
use std::path::Path;

use serde_json::Value;

use crate::cases::{self, Case, Expected};
use crate::classify::Label;
use crate::decide;
use crate::model::Request;
use crate::redact::Redactor;
use crate::store::{self, Store};

pub fn run(
    store: &Store,
    home: Option<&str>,
    cases_file: &Path,
    output: &mut impl Write,
) -> io::Result<()> {
    let labelled = store.read(store::LABELLED);
    let mut cases = cases::load(cases_file).map_err(io::Error::other)?;
    let redactor = Redactor::new(store.redaction_key()?, home.map(str::to_string));

    let mut remaining = Vec::new();
    let mut exported = 0;
    for entry in labelled {
        match export_one(&entry, &redactor) {
            Ok(Some(case)) => {
                if !cases.iter().any(|existing| {
                    existing.input.tool_name == case.input.tool_name
                        && existing.input.tool_input == case.input.tool_input
                        && existing.expected == case.expected
                }) {
                    cases.push(case);
                }
                exported += 1;
            }
            // Deleted entries stay so the request is not queued again.
            Ok(None) => remaining.push(entry),
            Err(reason) => {
                writeln!(
                    output,
                    "kept local: {}\n  {reason}",
                    entry
                        .get("request")
                        .map_or_else(String::new, crate::classify::describe)
                )?;
                remaining.push(entry);
            }
        }
    }
    cases::save(cases_file, &cases)?;
    store.replace(store::LABELLED, &remaining)?;
    writeln!(
        output,
        "Exported {exported} case(s) to {}.",
        cases_file.display()
    )
}

/// `Ok(None)` for entries that are not cases (deleted ones).
fn export_one(entry: &Value, redactor: &Redactor) -> Result<Option<Case>, String> {
    let label = entry
        .get("label")
        .and_then(Value::as_str)
        .and_then(Label::parse)
        .ok_or("unreadable label")?;
    let Some(wanted) = label.expected_kind() else {
        return Ok(None);
    };
    let raw = entry.get("request").ok_or("missing request")?;
    let request = Request::from_raw(raw).ok_or("unreadable request")?;

    let verdict = decide(&request);
    if verdict.kind() != wanted {
        return Err(format!(
            "labelled {}, but the current verdict is {verdict:?}; implement the rule first",
            label.as_str()
        ));
    }
    let expected = Expected::of(&verdict).ok_or("the verdict is a gap")?;

    let input = redactor.input(
        &request.tool_name,
        request.cwd.as_deref(),
        &request.tool_input,
    );
    let redacted_text = serde_json::to_string(&input).map_err(|error| error.to_string())?;
    if redactor.leaks(&redacted_text) {
        return Err("redaction left the home directory or username in place".into());
    }
    let after = decide(&input.request());
    if Expected::of(&after).as_ref() != Some(&expected) {
        return Err(format!(
            "redaction changed the verdict ({verdict:?} became {after:?}); extend the vocabulary or the rule"
        ));
    }
    let note = entry
        .get("note")
        .and_then(Value::as_str)
        .map(str::to_string);
    Ok(Some(Case {
        input,
        expected,
        note,
    }))
}

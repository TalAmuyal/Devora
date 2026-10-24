//! The regression suite: every case in `cases.json` must keep getting the verdict it was recorded with.

use std::path::Path;

use judgess::cases;
use judgess::classify::pending;
use judgess::decide;
use judgess::store::{LABELLED, Store};

#[test]
fn recorded_cases_keep_their_verdicts() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("cases.json");
    let cases = cases::load(&path).unwrap();
    assert!(!cases.is_empty());

    let failures: Vec<String> = cases
        .iter()
        .filter_map(|case| {
            case.check()
                .err()
                .map(|error| format!("{error}\n  {}", case.input.tool_input))
        })
        .collect();
    assert!(
        failures.is_empty(),
        "{} of {} cases failed:\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n")
    );
}

/// The developing agent's work list: reported, never failing, since it depends on one machine's usage.
#[test]
fn report_local_labelled_cases() {
    let Ok(home) = std::env::var("HOME") else {
        return;
    };
    let store = Store::in_home(&home);
    let labelled = store.read(LABELLED);
    if labelled.is_empty() {
        return;
    }
    let todo: Vec<_> = labelled
        .iter()
        .filter_map(|entry| {
            let raw = entry.get("request")?;
            let label = entry.get("label")?.as_str()?;
            let verdict = decide(&judgess::Request::from_raw(raw, Some(&home))?);
            let wanted = judgess::classify::Label::parse(label)?.expected_kind()?;
            (verdict.kind() != wanted).then(|| {
                format!(
                    "  labelled {label}, now {:?}: {}",
                    verdict.kind(),
                    judgess::classify::describe(raw)
                )
            })
        })
        .collect();
    let queued = pending(&store, Some(&home)).len();
    eprintln!(
        "labelled cases still to implement: {}\n{}\nunclassified requests: {queued}",
        todo.len(),
        todo.join("\n")
    );
}

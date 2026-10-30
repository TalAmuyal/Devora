//! classify -> export, end to end, against a throwaway store.

use std::io::Cursor;
use std::path::PathBuf;

use judgess::store::{LABELLED, Store, UNKNOWN};
use judgess::{cases, classify, export};
use serde_json::{Value, json};

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("judgess-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn bash(command: &str) -> Value {
    json!({"tool_name": "Bash", "cwd": "/Users/jane/secret-project", "tool_input": {"command": command}, "session_id": "s"})
}

#[test]
fn classified_requests_are_exported_redacted() {
    let dir = temp_dir("pipeline");
    let store = Store::at(&dir);
    let home = Some("/Users/jane");

    for request in [
        bash("ssh example.com"),
        bash("ssh example.com"),
        bash("ls /Users/jane/secret-project"),
    ] {
        store.append(UNKNOWN, &request).unwrap();
    }
    let mut output = Vec::new();
    classify::run(&store, &mut Cursor::new("a\nneeds network\n"), &mut output).unwrap();
    let labelled = store.read(LABELLED);
    assert_eq!(
        labelled.len(),
        1,
        "duplicates and already-decidable requests are not asked about"
    );
    assert_eq!(labelled[0]["label"], "approve");
    assert_eq!(labelled[0]["note"], "needs network");

    // One case the rules already agree with, one they do not (yet).
    store
        .append(LABELLED, &json!({"request": bash("git -C /Users/jane/secret-project status"), "label": "approve"}))
        .unwrap();
    // An option with an attached value must survive redaction with its verdict.
    store
        .append(
            LABELLED,
            &json!({"request": bash("sed -i.bak s/a/b/ notes.txt"), "label": "abstain"}),
        )
        .unwrap();
    let cases_file = dir.join("cases.json");
    std::fs::write(&cases_file, "[]").unwrap();
    export::run(&store, home, &cases_file, &mut Vec::new()).unwrap();

    let exported = cases::load(&cases_file).unwrap();
    assert_eq!(exported.len(), 2);
    let text = std::fs::read_to_string(&cases_file).unwrap();
    assert!(
        !text.contains("jane") && !text.contains("secret-project"),
        "{text}"
    );
    assert!(exported.iter().all(|case| case.check().is_ok()));

    let remaining = store.read(LABELLED);
    assert_eq!(
        remaining.len(),
        1,
        "only the unimplemented case stays local"
    );
    assert_eq!(remaining[0]["label"], "approve");

    std::fs::remove_dir_all(&dir).unwrap();
}

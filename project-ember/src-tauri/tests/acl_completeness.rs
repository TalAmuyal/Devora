//! Asserts that every Tauri command is registered in all three places the ACL system depends on: the invoke handler (src/lib.rs), the build manifest (build.rs), and the capability permissions (capabilities/main.json).
//! A command missing from any one of them compiles fine but is denied (or left with stale permissions) at runtime, with no visible error.

use std::collections::BTreeSet;
use std::path::Path;

fn read(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()))
}

fn slice_between<'a>(
    source: &'a str,
    start_marker: &str,
    end_marker: &str,
    context: &str,
) -> &'a str {
    let start = source
        .find(start_marker)
        .unwrap_or_else(|| panic!("{context}: marker {start_marker:?} not found"));
    let rest = &source[start + start_marker.len()..];
    let end = rest
        .find(end_marker)
        .unwrap_or_else(|| panic!("{context}: {start_marker:?} not closed by {end_marker:?}"));
    &rest[..end]
}

fn invoke_handler_commands() -> BTreeSet<String> {
    let source = read("src/lib.rs");
    let body = slice_between(&source, "tauri::generate_handler![", "]", "src/lib.rs");
    body.split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(|entry| entry.rsplit("::").next().unwrap().to_string())
        .collect()
}

fn build_manifest_commands() -> BTreeSet<String> {
    let source = read("build.rs");
    let body = slice_between(&source, ".commands(&[", "]", "build.rs");
    body.split('"').skip(1).step_by(2).map(str::to_string).collect()
}

fn capability_commands() -> BTreeSet<String> {
    let source = read("capabilities/main.json");
    let json: serde_json::Value =
        serde_json::from_str(&source).expect("capabilities/main.json: invalid JSON");
    json["permissions"]
        .as_array()
        .expect("capabilities/main.json: permissions must be an array")
        .iter()
        .map(|p| p.as_str().expect("capabilities/main.json: permissions must be strings"))
        .filter_map(|p| p.strip_prefix("allow-"))
        .map(|name| name.replace('-', "_"))
        .collect()
}

fn assert_sets_match(
    left_name: &str,
    left: &BTreeSet<String>,
    right_name: &str,
    right: &BTreeSet<String>,
) {
    let missing_from_right: Vec<_> = left.difference(right).collect();
    let missing_from_left: Vec<_> = right.difference(left).collect();
    assert!(
        missing_from_right.is_empty() && missing_from_left.is_empty(),
        "command lists differ:\n  in {left_name} but missing from {right_name}: {missing_from_right:?}\n  in {right_name} but missing from {left_name}: {missing_from_left:?}",
    );
}

#[test]
fn all_commands_registered_in_handler_manifest_and_capabilities() {
    let handler = invoke_handler_commands();
    let manifest = build_manifest_commands();
    let capabilities = capability_commands();

    assert!(!handler.is_empty(), "no commands parsed from src/lib.rs — marker drift?");
    assert!(!manifest.is_empty(), "no commands parsed from build.rs — marker drift?");
    assert!(
        !capabilities.is_empty(),
        "no permissions parsed from capabilities/main.json — marker drift?"
    );

    assert_sets_match("lib.rs invoke_handler", &handler, "build.rs manifest", &manifest);
    assert_sets_match("lib.rs invoke_handler", &handler, "capabilities/main.json", &capabilities);
}

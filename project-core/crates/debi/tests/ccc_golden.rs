//! `ccc` must launch Claude exactly as the `ccc.sh` script it replaced did.
//! `fixtures/ccc_golden.json` was recorded from `ccc.sh` (arguments and environment a fake `claude` received, and the exit code) before the script was removed.
//! Each case runs the real `debi` binary, through a `ccc` symlink in a fake app bundle, against the same fake `claude`.

use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::Path;
use std::process::Command;

use serde_json::{json, Value};

const ENV_KEYS: [&str; 3] = [
    "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC",
    "CLAUDE_CODE_ENABLE_TELEMETRY",
    "CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS",
];

/// A fake `claude` that prints the arguments and the relevant environment it received as JSON.
fn write_fake_claude(dir: &Path) {
    let script = format!(
        "#!/usr/bin/env python3\nimport json, os, sys\nkeys = {keys:?}\nprint(json.dumps({{\"args\": sys.argv[1:], \"env\": {{k: os.environ.get(k) for k in keys}}}}))\nsys.exit(int(os.environ.get(\"FAKE_CLAUDE_EXIT\", \"0\")))\n",
        keys = ENV_KEYS
    );
    let path = dir.join("claude");
    std::fs::write(&path, script).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// `<root>/bundled-apps/{debi, ccc -> debi}` and, unless `no_plugins`, three plugin directories and a stray file in `<root>/cc-plugins`.
fn fake_bundle(root: &Path, no_plugins: bool) {
    let apps = root.join("bundled-apps");
    std::fs::create_dir_all(&apps).unwrap();
    std::fs::copy(env!("CARGO_BIN_EXE_debi"), apps.join("debi")).unwrap();
    symlink("debi", apps.join("ccc")).unwrap();
    if !no_plugins {
        for plugin in ["judgess", "detached-flow", "code-cleanup"] {
            std::fs::create_dir_all(root.join("cc-plugins").join(plugin)).unwrap();
        }
        std::fs::write(root.join("cc-plugins").join("not-a-dir.txt"), "").unwrap();
    }
}

fn run_case(case: &Value, via_symlink: bool) -> Value {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().canonicalize().unwrap();
    fake_bundle(&root, case["no_plugins"].as_bool().unwrap());
    let bin = root.join("bin");
    std::fs::create_dir(&bin).unwrap();
    write_fake_claude(&bin);

    let user_args: Vec<&str> = case["args"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a.as_str().unwrap())
        .collect();
    let mut command = if via_symlink {
        Command::new(root.join("bundled-apps/ccc"))
    } else {
        let mut command = Command::new(root.join("bundled-apps/debi"));
        command.arg("ccc");
        command
    };
    command
        .args(&user_args)
        .env_clear()
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .env("HOME", &root);
    for (key, value) in case["env"].as_object().unwrap() {
        command.env(key, value.as_str().unwrap());
    }
    let output = command.output().unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let claude = stdout.lines().last().map_or(Value::Null, |line| {
        let mut received: Value = serde_json::from_str(line).unwrap();
        let root = root.to_string_lossy().into_owned();
        for arg in received["args"].as_array_mut().unwrap() {
            *arg = Value::String(arg.as_str().unwrap().replace(&root, "<root>"));
        }
        received
    });
    json!({ "exit_code": output.status.code().unwrap(), "claude": claude })
}

#[test]
fn ccc_matches_the_recorded_ccc_sh_behavior() {
    let fixtures: Vec<Value> =
        serde_json::from_str(include_str!("fixtures/ccc_golden.json")).expect("fixtures parse");
    assert!(!fixtures.is_empty());
    for case in &fixtures {
        for via_symlink in [true, false] {
            assert_eq!(
                run_case(case, via_symlink),
                case["expected"],
                "case {} (via {})",
                case["name"],
                if via_symlink {
                    "ccc symlink"
                } else {
                    "debi ccc"
                }
            );
        }
    }
}

#[test]
fn other_commands_are_delegated_to_debi_go() {
    let tmp = tempfile::tempdir().unwrap();
    let apps = tmp.path().join("bundled-apps");
    std::fs::create_dir_all(&apps).unwrap();
    std::fs::copy(env!("CARGO_BIN_EXE_debi"), apps.join("debi")).unwrap();
    let debi_go = apps.join("debi-go");
    std::fs::write(&debi_go, "#!/bin/sh\nprintf '%s|' \"$@\"\ncat\nexit 5\n").unwrap();
    std::fs::set_permissions(&debi_go, std::fs::Permissions::from_mode(0o755)).unwrap();

    let mut child = Command::new(apps.join("debi"))
        .args(["health", "--json", "a b"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    use std::io::Write;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"from-stdin")
        .unwrap();
    let output = child.wait_with_output().unwrap();

    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "health|--json|a b|from-stdin"
    );
    assert_eq!(output.status.code(), Some(5));
}

#[test]
fn a_missing_debi_go_is_reported() {
    let tmp = tempfile::tempdir().unwrap();
    let debi = tmp.path().join("debi");
    std::fs::copy(env!("CARGO_BIN_EXE_debi"), &debi).unwrap();
    let output = Command::new(&debi)
        .arg("health")
        .env("PATH", tmp.path())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(127));
    assert!(String::from_utf8_lossy(&output.stderr).contains("cannot run debi-go"));
}

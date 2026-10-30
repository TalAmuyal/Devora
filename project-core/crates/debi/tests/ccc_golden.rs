//! `ccc` must launch Claude exactly as the `ccc.sh` script it replaced did.
//! `fixtures/ccc_golden.json` was recorded from `ccc.sh` (arguments and environment a fake `claude` received, and the exit code) before the script was removed.
//! Each case runs the real `debi` binary, through a `ccc` symlink in a fake app bundle, against the same fake `claude`.

use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::Path;
use std::process::{Child, Command, Output, Stdio};
use std::sync::{PoisonError, RwLock};

use serde_json::{json, Value};

const ENV_KEYS: [&str; 3] = [
    "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC",
    "CLAUDE_CODE_ENABLE_TELEMETRY",
    "CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS",
];

/// Taken for writing by every write of an executable and for reading by every spawn, like Go's `syscall.ForkLock`.
/// A child forked while another thread still has an executable open for writing inherits that descriptor until its own exec, and exec'ing the executable meanwhile fails on Linux with "Text file busy".
static EXEC_LOCK: RwLock<()> = RwLock::new(());

fn write_script(path: &Path, script: &str) {
    let _guard = EXEC_LOCK.write().unwrap_or_else(PoisonError::into_inner);
    std::fs::write(path, script).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

fn copy_debi(dest: &Path) {
    let _guard = EXEC_LOCK.write().unwrap_or_else(PoisonError::into_inner);
    std::fs::copy(env!("CARGO_BIN_EXE_debi"), dest).unwrap();
}

/// `spawn` returns only once the child has exec'd, by which point it no longer holds any inherited descriptor.
fn spawn(command: &mut Command) -> Child {
    let _guard = EXEC_LOCK.read().unwrap_or_else(PoisonError::into_inner);
    command.spawn().unwrap()
}

/// Like [`Command::output`], but spawned through [`spawn`].
fn output(command: &mut Command) -> Output {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    spawn(command).wait_with_output().unwrap()
}

/// A fake `claude` that prints the arguments and the relevant environment it received as JSON.
fn write_fake_claude(dir: &Path) {
    let script = format!(
        "#!/usr/bin/env python3\nimport json, os, sys\nkeys = {keys:?}\nprint(json.dumps({{\"args\": sys.argv[1:], \"env\": {{k: os.environ.get(k) for k in keys}}}}))\nsys.exit(int(os.environ.get(\"FAKE_CLAUDE_EXIT\", \"0\")))\n",
        keys = ENV_KEYS
    );
    write_script(&dir.join("claude"), &script);
}

/// `<root>/bundled-apps/{debi, ccc -> debi}` and, unless `no_plugins`, three plugin directories and a stray file in `<root>/cc-plugins`.
fn fake_bundle(root: &Path, no_plugins: bool) {
    let apps = root.join("bundled-apps");
    std::fs::create_dir_all(&apps).unwrap();
    copy_debi(&apps.join("debi"));
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
    let output = output(&mut command);

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
    copy_debi(&apps.join("debi"));
    write_script(
        &apps.join("debi-go"),
        "#!/bin/sh\nprintf '%s|' \"$@\"\ncat\nexit 5\n",
    );

    let mut child = spawn(
        Command::new(apps.join("debi"))
            .args(["health", "--json", "a b"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped()),
    );
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
    copy_debi(&debi);
    let output = output(Command::new(&debi).arg("health").env("PATH", tmp.path()));
    assert_eq!(output.status.code(), Some(127));
    assert!(String::from_utf8_lossy(&output.stderr).contains("cannot run debi-go"));
}

//! The hook binary must never fail the agent: whatever it is fed, stdout stays empty and the exit code is 0.

use std::io::Write;
use std::process::{Command, Stdio};

fn run_hook(input: &str) -> std::process::Output {
    let home = std::env::temp_dir().join(format!("judgess-hook-{}", std::process::id()));
    std::fs::create_dir_all(&home).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_judgess"))
        .arg("hook")
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    std::fs::remove_dir_all(&home).unwrap();
    output
}

fn bash(command: &str) -> String {
    serde_json::json!({"tool_name": "Bash", "tool_input": {"command": command}}).to_string()
}

#[test]
fn input_the_hook_cannot_handle_produces_no_output_and_succeeds() {
    let deep =
        |open: &str, close: &str| format!("{}ls{}", open.repeat(10_000), close.repeat(10_000));
    for input in [
        "not json".to_string(),
        "[]".to_string(),
        "{}".to_string(),
        bash(&deep("(", ")")),
        bash(&deep("{ ", " }")),
        bash(&deep("${a:-", "}")),
        bash(&deep("if true; then ", " fi;")),
        bash(&deep("case x in x) ", " esac;")),
    ] {
        let output = run_hook(&input);
        assert!(output.status.success(), "{}", &input[..input.len().min(60)]);
        assert!(
            output.stdout.is_empty(),
            "{}",
            &input[..input.len().min(60)]
        );
    }
}

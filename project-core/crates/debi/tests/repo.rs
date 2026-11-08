//! Real Rust Debi commands against local Git repositories and isolated profiles.
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use devora_agent_space::{CreationForm, Space};
use devora_workspace::creation::{CreationHandle, EventSink};
use serde_json::{json, Value};

struct Fixture {
    root: tempfile::TempDir,
    profile: PathBuf,
    global: PathBuf,
    workspace: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let base = root.path().canonicalize().unwrap();
        let profile = base.join("profile");
        let workspace = profile.join("workspaces/ws-1");
        fs::create_dir_all(&workspace).unwrap();
        fs::create_dir_all(profile.join("repos")).unwrap();
        fs::write(workspace.join("initialized"), "").unwrap();
        fs::write(profile.join("config.json"), r#"{"name":"Work"}"#).unwrap();
        let global = base.join("global.json");
        write_json(&global, json!({"profiles": [profile]}));
        Self {
            root,
            profile,
            global,
            workspace,
        }
    }

    fn settings(&self, extra: Value) {
        let mut settings = json!({"name": "Work"});
        settings
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        write_json(&self.profile.join("config.json"), settings);
    }

    fn repo(&self, path: &Path) {
        let origin = tempfile::tempdir_in(self.root.path()).unwrap().keep();
        git(
            self.root.path(),
            &[
                "init",
                "--bare",
                "-q",
                "-b",
                "main",
                origin.to_str().unwrap(),
            ],
        );
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        git(
            self.root.path(),
            &[
                "clone",
                "-q",
                origin.to_str().unwrap(),
                path.to_str().unwrap(),
            ],
        );
        git(
            path,
            &[
                "-c",
                "user.name=fixture",
                "-c",
                "user.email=fixture@localhost",
                "commit",
                "-q",
                "--allow-empty",
                "-m",
                "initial",
            ],
        );
        git(path, &["push", "-q", "origin", "main"]);
    }

    fn command(&self, cwd: &Path, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_debi"));
        command
            .arg("repo")
            .args(args)
            .current_dir(cwd)
            .env("DEVORA_CONFIG_PATH", &self.global)
            .env("HOME", self.root.path())
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env_remove("DEBI_PROFILE_PATH");
        command
    }

    fn run(&self, cwd: &Path, args: &[&str]) -> Output {
        self.command(cwd, args).output().unwrap()
    }
}

fn write_json(path: &Path, value: Value) {
    fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
}

fn git(cwd: &Path, args: &[&str]) -> Output {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn assert_code(output: &Output, code: i32) {
    assert_eq!(
        output.status.code(),
        Some(code),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn list_merges_registered_and_discovered_names_and_add_refuses_ambiguity() {
    let f = Fixture::new();
    let discovered = f.profile.join("repos/alpha");
    let registered = f.root.path().join("external/beta");
    let duplicate_name = f.root.path().join("external/alpha");
    for repo in [&discovered, &registered, &duplicate_name] {
        f.repo(repo);
    }
    f.settings(json!({"repos": [registered, duplicate_name, discovered]}));
    let output = f.run(&f.workspace, &["list"]);
    assert_code(&output, 0);
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "alpha\nbeta\n");
    let output = f.run(&f.workspace, &["add", "alpha"]);
    assert_code(&output, 2);
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(
        error.contains("ambiguous")
            && error.contains(discovered.to_str().unwrap())
            && error.contains(duplicate_name.to_str().unwrap())
    );
    assert!(!f.workspace.join("alpha").exists());
}

#[test]
fn workspace_add_runs_prepare_supports_postfix_and_preserves_existing_targets() {
    let f = Fixture::new();
    let source = f.profile.join("repos/repo");
    f.repo(&source);
    f.settings(json!({"prepare-command": "printf 'prepared output\\n'; touch prepared"}));
    let nested = f.workspace.join("existing/nested");
    fs::create_dir_all(&nested).unwrap();
    let output = f.run(&nested, &["add", "repo", "--postfix", "reference"]);
    assert_code(&output, 0);
    let target = f.workspace.join("repo-reference");
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!("{}\n", target.display())
    );
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("prepared output"));
    assert!(target.join("prepared").exists());
    let symbolic = Command::new("git")
        .args(["symbolic-ref", "-q", "HEAD"])
        .current_dir(&target)
        .output()
        .unwrap();
    assert!(
        !symbolic.status.success(),
        "added checkout must be detached"
    );
    assert_eq!(
        git(&target, &["rev-parse", "HEAD"]).stdout,
        git(&source, &["rev-parse", "origin/main"]).stdout
    );
    assert!(fs::read_to_string(f.workspace.join("CLAUDE.md"))
        .unwrap()
        .contains("repo-reference/"));
    fs::write(target.join("keep"), "owned by the user").unwrap();
    let output = f.run(&f.workspace, &["add", "repo", "--postfix", "reference"]);
    assert_code(&output, 1);
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("already exists"));
    assert_eq!(
        fs::read_to_string(target.join("keep")).unwrap(),
        "owned by the user"
    );
}

#[test]
fn add_to_an_open_agent_space_preserves_its_instruction_layout() {
    let f = Fixture::new();
    f.repo(&f.profile.join("repos/repo"));
    let events: EventSink = Arc::new(|_| {});
    let space = Space::create(
        &f.profile,
        CreationForm {
            name: "agent".into(),
            role: None,
            repos: vec![],
            inputs: BTreeMap::new(),
            keep_coding_instructions: true,
            model: None,
        },
        BTreeMap::new(),
        &f.global,
        &CreationHandle::new(),
        &events,
        &mut vec![],
    )
    .unwrap();
    let output = f.run(&space.path().join("memory"), &["add", "repo"]);
    assert_code(&output, 0);
    assert!(space.path().join("repo/.git").exists());
    assert!(space.path().join("memory").is_dir());
    assert!(!space.path().join("CLAUDE.md").exists());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!("{}/repo\n", space.path().display())
    );
}

#[test]
fn session_profile_wins_for_list_and_mismatch_refuses_add_before_mutation() {
    let f = Fixture::new();
    f.repo(&f.profile.join("repos/repo"));
    let other = f.root.path().canonicalize().unwrap().join("other");
    fs::create_dir_all(&other).unwrap();
    fs::write(other.join("config.json"), r#"{"name":"Other"}"#).unwrap();
    write_json(&f.global, json!({"profiles": [f.profile, other]}));
    let list = f
        .command(&f.workspace, &["list"])
        .env("DEBI_PROFILE_PATH", &other)
        .output()
        .unwrap();
    assert_code(&list, 0);
    assert!(list.stdout.is_empty());
    let output = f
        .command(&f.workspace, &["add", "repo"])
        .env("DEBI_PROFILE_PATH", &other)
        .output()
        .unwrap();
    assert_code(&output, 2);
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(error.contains(other.to_str().unwrap()) && error.contains(f.profile.to_str().unwrap()));
    assert!(!f.workspace.join("repo").exists());
}

#[test]
fn prepare_failure_retains_the_successful_add_with_a_stderr_warning() {
    let f = Fixture::new();
    f.repo(&f.profile.join("repos/repo"));
    f.settings(json!({"prepare-command": "echo prepare-failed; exit 7"}));
    let output = f.run(&f.workspace, &["add", "repo"]);
    assert_code(&output, 0);
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!("{}/repo\n", f.workspace.display())
    );
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("prepare-command failed"));
    assert!(f.workspace.join("repo/.git").exists());
}

#[test]
fn ctrl_c_waits_for_rollback_and_deregisters_the_added_worktree() {
    let f = Fixture::new();
    let source = f.profile.join("repos/repo");
    f.repo(&source);
    f.settings(json!({"prepare-command": "printf '%s' $$ > ../prepare.pid; sleep 30"}));
    let mut child = f
        .command(&f.workspace, &["add", "repo"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let ready = f.workspace.join("prepare.pid");
    let deadline = Instant::now() + Duration::from_secs(15);
    while !ready.exists() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(20));
    }
    if !ready.exists() {
        let _ = child.kill();
        let output = child.wait_with_output().unwrap();
        panic!(
            "prepare barrier not reached: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    // SAFETY: the pid belongs to the live Debi child started above.
    assert_eq!(
        unsafe { libc::kill(child.id() as libc::pid_t, libc::SIGINT) },
        0
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    while child.try_wait().unwrap().is_none() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(20));
    }
    if child.try_wait().unwrap().is_none() {
        let _ = child.kill();
        let _ = child.wait();
        panic!("Ctrl+C did not finish rollback promptly");
    }
    let output = child.wait_with_output().unwrap();
    assert_code(&output, 130);
    assert!(output.stdout.is_empty());
    assert!(!f.workspace.join("repo").exists());
    let worktrees =
        String::from_utf8(git(&source, &["worktree", "list", "--porcelain"]).stdout).unwrap();
    assert!(!worktrees.contains(f.workspace.to_str().unwrap()));
}

#[test]
fn usage_errors_and_empty_lists_have_the_documented_exit_codes() {
    let f = Fixture::new();
    let output = f.run(&f.workspace, &["list"]);
    assert_code(&output, 0);
    assert!(output.stdout.is_empty());
    assert_code(&f.run(&f.workspace, &["--help"]), 0);
    assert_code(&f.run(&f.workspace, &["unknown"]), 2);
    assert_code(&f.run(&f.workspace, &["add"]), 2);
    assert_code(&f.run(f.root.path(), &["list"]), 2);
    f.repo(&f.profile.join("repos/repo"));
    assert_code(
        &f.run(&f.workspace, &["add", "repo", "--postfix", "../escape"]),
        2,
    );
    assert!(!f.profile.join("escape").exists());
}

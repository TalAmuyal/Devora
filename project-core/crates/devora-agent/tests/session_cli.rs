//! Explicit integration coverage invoked by Sapphire's standard test command.
//! Default cargo test compiles this target but does not require Node or Claude.
use devora_agent::protocol::{KnownMessage, Message};
use devora_agent::session::*;
use devora_app::log::Log;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

struct Backend {
    child: Child,
    input: ChildStdin,
    output: Lines<BufReader<ChildStdout>>,
    port: u16,
    log: PathBuf,
}

impl Backend {
    async fn start(root: &std::path::Path) -> Self {
        let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let mut child = Command::new("node")
            .args(["--import"])
            .arg(repo.join("project-sapphire/node_modules/tsx/dist/loader.mjs"))
            .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/support/claude_backend.mts"))
            .env("RECORD_MODE", "0")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::from(
                std::fs::File::create(root.join("backend.log")).unwrap(),
            ))
            .kill_on_drop(true)
            .spawn()
            .expect("Node and Sapphire npm dependencies are required");
        let input = child.stdin.take().unwrap();
        let mut output = BufReader::new(child.stdout.take().unwrap()).lines();
        let log = root.join("backend.log");
        let ready = Self::receive(&mut output, &log).await;
        assert_eq!(ready["type"], "ready");
        let port = ready["port"].as_u64().unwrap() as u16;
        Self {
            child,
            input,
            output,
            port,
            log,
        }
    }

    async fn receive(output: &mut Lines<BufReader<ChildStdout>>, log: &std::path::Path) -> Value {
        let line = tokio::time::timeout(Duration::from_secs(20), output.next_line())
            .await
            .expect("backend protocol deadline")
            .unwrap()
            .unwrap_or_else(|| {
                panic!(
                    "backend exited: {}",
                    std::fs::read_to_string(log).unwrap_or_default()
                )
            });
        serde_json::from_str(&line).unwrap()
    }

    async fn command(&mut self, value: Value) -> Value {
        self.input
            .write_all(format!("{value}\n").as_bytes())
            .await
            .unwrap();
        Self::receive(&mut self.output, &self.log).await
    }

    async fn stop(mut self) {
        self.input
            .write_all(b"{\"action\":\"stop\"}\n")
            .await
            .unwrap();
        self.input.shutdown().await.unwrap();
        let status = tokio::time::timeout(Duration::from_secs(10), self.child.wait())
            .await
            .expect("backend stop deadline")
            .unwrap();
        assert!(status.success());
    }
}

struct Fixture {
    root: tempfile::TempDir,
    home: PathBuf,
    work: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("home");
        let work = root.path().join("work");
        std::fs::create_dir_all(home.join(".claude")).unwrap();
        std::fs::create_dir_all(work.join(".claude")).unwrap();
        for (name, directory) in [
            ("user", home.join(".claude")),
            ("project", work.join(".claude")),
        ] {
            let hook = format!(
                "touch {}",
                root.path().join(format!("{name}-loaded")).display()
            );
            std::fs::write(
                directory.join("settings.json"),
                json!({
                    "model": if name == "user" { "opus" } else { "sonnet" },
                "effortLevel": "low",
                "env":{"DEVORA_PRECEDENCE":name,"DISABLE_AUTO_COMPACT":"0","CALLER_ONLY":name,"PROCESS_ONLY":name},
                    "hooks":{"SessionStart":[{"hooks":[{"type":"command","command":hook}]}]}
                })
                .to_string(),
            )
            .unwrap();
        }
        std::fs::write(home.join(".claude/CLAUDE.md"), "DEVORA_USER_MEMORY_PROBE\n").unwrap();
        Self { root, home, work }
    }

    fn options(&self, port: u16, instructions: Instructions) -> SessionOptions {
        let hook = format!(
            "printf %s:%s:%s:%s \"$DEVORA_PRECEDENCE\" \"$DISABLE_AUTO_COMPACT\" \"$CALLER_ONLY\" \"$PROCESS_ONLY\" > {}",
            self.root.path().join("precedence").display()
        );
        SessionOptions {
            program: pinned_cli(),
            working_directory: self.work.clone(),
            instructions,
            model: "haiku".into(),
            effort: Some("high".into()),
            permission_mode: "auto".into(),
            plugins: vec![],
            settings: json!({"env":{"DEVORA_PRECEDENCE":"devora","DISABLE_AUTO_COMPACT":"1","CALLER_ONLY":"caller"},
                "hooks":{"SessionStart":[{"hooks":[{"type":"command","command":hook}]}]}}),
            environment: BTreeMap::from_iter([
                ("PATH".into(), std::env::var("PATH").unwrap()),
                ("HOME".into(), self.home.to_str().unwrap().into()),
                ("USER".into(), "probe".into()),
                ("SHELL".into(), "/bin/zsh".into()),
                ("LANG".into(), "en_US.UTF-8".into()),
                ("ANTHROPIC_API_KEY".into(), "offline-test-key".into()),
                (
                    "ANTHROPIC_BASE_URL".into(),
                    format!("http://127.0.0.1:{port}"),
                ),
                (
                    "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC".into(),
                    "1".into(),
                ),
                ("CLAUDE_CODE_ENABLE_TELEMETRY".into(), "0".into()),
                ("DISABLE_AUTO_COMPACT".into(), "0".into()),
                ("CALLER_ONLY".into(), "process".into()),
                ("PROCESS_ONLY".into(), "transport".into()),
            ]),
            resume: None,
        }
    }

    fn log(&self) -> Arc<Log> {
        Arc::new(Log::open(&self.root.path().join("session.log")).unwrap())
    }
}

async fn result(session: &mut Session) -> Value {
    result_with_status(session, false).await
}

async fn result_with_status(session: &mut Session, allow_error: bool) -> Value {
    tokio::time::timeout(Duration::from_secs(45), async {
        while let Some(event) = session.next_event().await {
            match event {
                SessionEvent::Message(Message::Known(KnownMessage::Result(result))) => {
                    if !allow_error {
                        assert!(!result.is_error, "{result:?}");
                    }
                    return serde_json::to_value(result).unwrap();
                }
                SessionEvent::Failed(error) => panic!("{error}"),
                SessionEvent::Compacted(metadata) => {
                    panic!("unexpected CLI compaction: {metadata:?}")
                }
                _ => {}
            }
        }
        panic!("CLI ended before result");
    })
    .await
    .expect("offline CLI turn deadline")
}

fn pinned_cli() -> PathBuf {
    std::env::var_os("DEVORA_TEST_CLAUDE_PROGRAM")
        .map(PathBuf::from)
        .expect("run through Sapphire test-agent-cli to provision the pinned CLI")
}

async fn check_pin() {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    // CI's pin, which Dependabot bumps (scripts/ci/tools/README.md).
    let manifest: Value = serde_json::from_str(
        &std::fs::read_to_string(repo.join("scripts/ci/tools/package.json")).unwrap(),
    )
    .unwrap();
    let pin = manifest["dependencies"]["@anthropic-ai/claude-code"]
        .as_str()
        .expect("an exact @anthropic-ai/claude-code version in scripts/ci/tools/package.json");
    let version = Command::new(pinned_cli())
        .arg("--version")
        .output()
        .await
        .expect("pinned Claude CLI is required");
    assert!(version.status.success());
    assert_eq!(
        String::from_utf8(version.stdout)
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap(),
        pin
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires CI-pinned Claude CLI, Node and Sapphire npm dependencies"]
async fn pinned_cli_preserves_user_sources_and_explicit_configuration_for_all_prompt_forms() {
    check_pin().await;
    let server_root = tempfile::tempdir().unwrap();
    let mut backend = Backend::start(server_root.path()).await;
    for (index, instructions) in [
        Instructions::Append("DEVORA_PROMPT_PROBE appended".into()),
        Instructions::Replace("DEVORA_PROMPT_PROBE replacement".into()),
        Instructions::Agent {
            name: "probe".into(),
            definition: json!({"description":"Offline probe","prompt":"DEVORA_PROMPT_PROBE agent"}),
        },
    ]
    .into_iter()
    .enumerate()
    {
        backend.command(json!({"action":"configure"})).await;
        let fixture = Fixture::new();
        let mut options = fixture.options(backend.port, instructions);
        if index == 2 {
            options.environment.remove("ANTHROPIC_API_KEY");
            options.environment.insert(
                "CLAUDE_CODE_OAUTH_TOKEN".into(),
                "offline-oauth-token".into(),
            );
        }
        let mut session = Session::start(options, fixture.log()).await.unwrap();
        session
            .handle()
            .send_turn(turn_id(), "Reply briefly.".into())
            .await
            .unwrap();
        result(&mut session).await;
        if index == 0 {
            let handle = session.handle();
            handle.set_model("sonnet".into()).await.unwrap();
            handle.set_permission_mode("plan".into()).await.unwrap();
            handle
                .send_turn(turn_id(), "Reply once more.".into())
                .await
                .unwrap();
            result(&mut session).await;
        }
        session.pause().await.unwrap();
        let snapshot = backend.command(json!({"action":"snapshot"})).await;
        let requests = snapshot["requests"].as_array().unwrap();
        assert!(!requests.is_empty());
        if index == 0 {
            assert!(requests.last().unwrap()["body"]["model"]
                .as_str()
                .unwrap()
                .contains("sonnet"));
        }
        if index == 2 {
            assert_eq!(requests[0]["auth"], "oauth");
            assert_eq!(requests[0]["callerOAuth"], true);
        }
        if index < 2 {
            assert_eq!(requests[0]["callerKey"], true);
        }
        let body = &requests[0]["body"];
        assert!(serde_json::to_string(&body["system"])
            .unwrap()
            .contains("DEVORA_PROMPT_PROBE"));
        assert!(serde_json::to_string(&requests)
            .unwrap()
            .contains("DEVORA_USER_MEMORY_PROBE"));
        assert!(fixture.root.path().join("user-loaded").exists());
        assert!(fixture.root.path().join("project-loaded").exists());
        assert_eq!(
            std::fs::read_to_string(fixture.root.path().join("precedence")).unwrap(),
            "devora:1:caller:project"
        );
        assert!(body["model"].as_str().unwrap().contains("haiku"));
        assert_eq!(
            body.pointer("/output_config/effort"),
            Some(&json!("high")),
            "output_config={:?}",
            body.get("output_config")
        );
    }
    backend.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires CI-pinned Claude CLI, Node and Sapphire npm dependencies"]
async fn pinned_cli_exposes_usage_and_suppresses_reactive_automatic_compaction() {
    check_pin().await;
    let fixture = Fixture::new();
    let mut backend = Backend::start(fixture.root.path()).await;
    backend
        .command(json!({"action":"configure","inputTokens":100_000,"errorAt":1}))
        .await;
    let options = fixture.options(
        backend.port,
        Instructions::Append("Offline usage probe".into()),
    );
    let mut session = Session::start(options.clone(), fixture.log())
        .await
        .unwrap();
    let handle = session.handle();
    handle
        .send_turn(turn_id(), "First turn.".into())
        .await
        .unwrap();
    let first = result(&mut session).await;
    assert!(first["usage"]["input_tokens"].as_u64().unwrap() > 50_000);
    handle
        .send_turn(turn_id(), "Second turn.".into())
        .await
        .unwrap();
    let blocked = result_with_status(&mut session, true).await;
    assert_eq!(
        blocked["is_error"], true,
        "the disabled CLI must surface prompt-too-long to its owner"
    );
    session.pause().await.unwrap();
    let snapshot = backend.command(json!({"action":"snapshot"})).await;
    let requests = snapshot["requests"].as_array().unwrap();
    assert_eq!(
        requests.len(),
        2,
        "an unexpected model request may indicate CLI auto-compaction"
    );
    backend
        .command(json!({"action":"configure","inputTokens":100_000,"errorAt":1}))
        .await;
    automatic_compaction_control(&options, fixture.root.path()).await;
    backend.stop().await;
}

// Counterfactual wire driver for the compaction switch. The product runtime
// always disables CLI auto-compaction, so this peer intentionally enables it.
async fn automatic_compaction_control(options: &SessionOptions, root: &std::path::Path) {
    let mut settings = options.settings.clone();
    settings["env"]
        .as_object_mut()
        .unwrap()
        .remove("DISABLE_AUTO_COMPACT");
    let id = uuid::Uuid::new_v4().to_string();
    let mut environment = options.environment.clone();
    environment.remove("DISABLE_AUTO_COMPACT");
    let mut child = Command::new(&options.program)
        .args([
            "-p",
            "--input-format",
            "stream-json",
            "--output-format",
            "stream-json",
            "--verbose",
            "--include-partial-messages",
            "--permission-prompt-tool",
            "stdio",
            "--permission-mode",
            "auto",
            "--model",
            &options.model,
            "--effort",
            options.effort.as_deref().unwrap(),
            "--session-id",
            &id,
            "--settings",
            &settings.to_string(),
        ])
        .env_clear()
        .envs(environment)
        .current_dir(&options.working_directory)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::from(
            std::fs::File::create(root.join("compaction-control.log")).unwrap(),
        ))
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap()).lines();
    input.write_all(b"{\"type\":\"control_request\",\"request_id\":\"probe-init\",\"request\":{\"subtype\":\"initialize\",\"hooks\":{}}}\n").await.unwrap();
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let line = output
                .next_line()
                .await
                .unwrap()
                .expect("control initialize");
            let value: Value = serde_json::from_str(&line).unwrap();
            if value["type"] == "control_response" {
                assert_eq!(value["response"]["subtype"], "success");
                break;
            }
        }
    })
    .await
    .expect("control initialize deadline");
    let mut automatic = false;
    for prompt in ["First control turn.", "Second control turn."] {
        let turn = Message::user_text(id.clone(), turn_id(), prompt.into());
        input
            .write_all(format!("{}\n", serde_json::to_string(&turn).unwrap()).as_bytes())
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(45), async {
            loop {
                let line = output
                    .next_line()
                    .await
                    .unwrap()
                    .expect("compaction control turn");
                match Message::parse(&line).unwrap() {
                    Message::Known(KnownMessage::System(
                        devora_agent::protocol::System::CompactBoundary {
                            compact_metadata, ..
                        },
                    )) => {
                        if compact_metadata.trigger == "auto" {
                            automatic = true;
                        }
                    }
                    Message::Known(KnownMessage::Result(result)) => {
                        assert!(!result.is_error, "control turn failed: {result:?}");
                        break;
                    }
                    _ => {}
                }
            }
        })
        .await
        .expect("compaction control deadline");
    }
    input.shutdown().await.unwrap();
    drop(input);
    let status = tokio::time::timeout(Duration::from_secs(10), child.wait())
        .await
        .expect("control stop deadline")
        .unwrap();
    assert!(status.success());
    assert!(
        automatic,
        "the enabled control must compact under the same context pressure"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires CI-pinned Claude CLI, Node and Sapphire npm dependencies"]
async fn pinned_cli_executes_the_in_process_judgess_callback_before_permission_flow() {
    check_pin().await;
    let fixture = Fixture::new();
    let mut backend = Backend::start(fixture.root.path()).await;
    let forbidden = fixture.root.path().join("must-not-run");
    backend.command(json!({"action":"configure","toolCommand":format!("eval touch {}",forbidden.display())})).await;
    let mut session = Session::start(
        fixture.options(
            backend.port,
            Instructions::Append("Offline hook probe".into()),
        ),
        fixture.log(),
    )
    .await
    .unwrap();
    session
        .handle()
        .send_turn(turn_id(), "Perform the tool call.".into())
        .await
        .unwrap();
    let mut judged = false;
    tokio::time::timeout(Duration::from_secs(45), async {
        while let Some(event) = session.next_event().await {
            match event {
                SessionEvent::ToolJudged {
                    verdict,
                    tool_use_id,
                } => {
                    assert_eq!(verdict.kind().as_str(), "deny");
                    assert!(tool_use_id.is_some());
                    judged = true;
                }
                SessionEvent::PermissionRequested { .. } => {
                    panic!("a Judgess denial must not ask for permission")
                }
                SessionEvent::Message(Message::Known(KnownMessage::Result(result))) => {
                    assert!(!result.is_error, "{result:?}");
                    break;
                }
                SessionEvent::Failed(error) => panic!("{error}"),
                _ => {}
            }
        }
    })
    .await
    .expect("hook turn deadline");
    assert!(
        judged,
        "the real CLI must invoke the registered PreToolUse callback"
    );
    assert!(!forbidden.exists());
    session.pause().await.unwrap();
    let snapshot = backend.command(json!({"action":"snapshot"})).await;
    assert!(snapshot
        .to_string()
        .contains("Command 'eval' is not allowed."));

    let user_settings = fixture.home.join(".claude/settings.json");
    let mut settings: Value =
        serde_json::from_str(&std::fs::read_to_string(&user_settings).unwrap()).unwrap();
    settings["permissions"] = json!({"ask":["Bash"]});
    std::fs::write(user_settings, settings.to_string()).unwrap();
    backend
        .command(json!({"action":"configure","toolCommand":"git status"}))
        .await;
    let mut asking = Session::start(
        fixture.options(
            backend.port,
            Instructions::Append("Offline ask-rule probe".into()),
        ),
        fixture.log(),
    )
    .await
    .unwrap();
    let handle = asking.handle();
    handle
        .send_turn(turn_id(), "Perform the tool call.".into())
        .await
        .unwrap();
    let mut approved = false;
    let mut asked = false;
    tokio::time::timeout(Duration::from_secs(45), async {
        while let Some(event) = asking.next_event().await {
            match event {
                SessionEvent::ToolJudged { verdict, .. } => {
                    assert_eq!(verdict.kind().as_str(), "approve");
                    approved = true;
                }
                SessionEvent::PermissionRequested {
                    request_id,
                    question,
                } => {
                    assert_eq!(question.tool_name, "Bash");
                    asked = true;
                    handle
                        .answer_permission(
                            request_id,
                            PermissionAnswer::Deny {
                                message: "User denied in offline probe".into(),
                            },
                        )
                        .await
                        .unwrap();
                }
                SessionEvent::Message(Message::Known(KnownMessage::Result(result))) => {
                    assert!(!result.is_error, "{result:?}");
                    break;
                }
                SessionEvent::Failed(error) => panic!("{error}"),
                _ => {}
            }
        }
    })
    .await
    .expect("ask-rule turn deadline");
    assert!(
        approved && asked,
        "user ask rules must still win over a Judgess approval"
    );
    asking.pause().await.unwrap();
    let snapshot = backend.command(json!({"action":"snapshot"})).await;
    assert!(snapshot
        .to_string()
        .contains("User denied in offline probe"));
    backend.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires CI-pinned Claude CLI, Node and Sapphire npm dependencies"]
async fn pinned_cli_pause_and_resume_retains_the_conversation() {
    async fn collect_turn(session: &mut Session) -> Vec<Value> {
        tokio::time::timeout(Duration::from_secs(45), async {
            let mut messages = vec![];
            while let Some(event) = session.next_event().await {
                match event {
                    SessionEvent::Message(message) => {
                        let done = matches!(&message, Message::Known(KnownMessage::Result(_)));
                        messages.push(serde_json::to_value(message).unwrap());
                        if done {
                            return messages;
                        }
                    }
                    SessionEvent::Failed(error) => panic!("{error}"),
                    _ => {}
                }
            }
            panic!("session ended before result");
        })
        .await
        .expect("resume probe deadline")
    }
    check_pin().await;
    let fixture = Fixture::new();
    let mut backend = Backend::start(fixture.root.path()).await;
    let options = fixture.options(
        backend.port,
        Instructions::Append("Offline resume probe".into()),
    );
    let mut first = Session::start(options.clone(), fixture.log())
        .await
        .unwrap();
    first
        .handle()
        .send_turn(turn_id(), "Remember the word AMBER.".into())
        .await
        .unwrap();
    let first_messages = collect_turn(&mut first).await;
    let mut folded = devora_agent::transcript::Transcript::new(vec![], first.session_id());
    for value in &first_messages {
        folded.event(&SessionEvent::Message(
            Message::parse(&value.to_string()).unwrap(),
        ));
    }
    for block_type in ["text", "thinking"] {
        let expected = first_messages
            .iter()
            .filter(|value| value["type"] == "assistant")
            .flat_map(|value| value["message"]["content"].as_array().into_iter().flatten())
            .filter(|block| block["type"] == block_type)
            .count();
        let actual = folded
            .items()
            .iter()
            .filter(|item| {
                matches!(
                    (&item.content, block_type),
                    (
                        devora_agent::transcript::TranscriptContent::Assistant { .. },
                        "text"
                    ) | (
                        devora_agent::transcript::TranscriptContent::Thinking { .. },
                        "thinking"
                    )
                )
            })
            .count();
        assert_eq!(
            actual, expected,
            "streamed {block_type} must be replaced by whole native blocks"
        );
    }
    let paused = first.pause().await.unwrap();
    assert!(paused.graceful);
    let mut resumed_options = options;
    resumed_options.resume = Some(Resume {
        session_id: paused.session_id.clone(),
        has_conversation: paused.has_conversation,
    });
    let mut resumed = Session::start(resumed_options, fixture.log())
        .await
        .unwrap();
    assert_eq!(resumed.session_id(), paused.session_id);
    resumed
        .handle()
        .send_turn(turn_id(), "Recall the earlier word.".into())
        .await
        .unwrap();
    let resumed_messages = collect_turn(&mut resumed).await;
    let first_ids: Vec<_> = first_messages
        .iter()
        .filter_map(|v| v["message"]["id"].as_str())
        .collect();
    assert!(!first_ids.is_empty());
    let replayed = resumed_messages
        .iter()
        .filter(|v| {
            v["message"]["id"]
                .as_str()
                .is_some_and(|id| first_ids.contains(&id))
                || (v["type"] == "user" && v["message"]["content"] == "Remember the word AMBER.")
                || v["event"]["message"]["id"]
                    .as_str()
                    .is_some_and(|id| first_ids.contains(&id))
        })
        .count();
    eprintln!(
        "pinned resume replay probe: first={} resumed={} prior_message_replays={replayed}",
        first_messages.len(),
        resumed_messages.len()
    );
    assert_eq!(
        replayed, 0,
        "resume unexpectedly replays previous transcript messages"
    );
    assert!(resumed.pause().await.unwrap().graceful);
    let snapshot = backend.command(json!({"action":"snapshot"})).await;
    let requests = snapshot["requests"].as_array().unwrap();
    assert_eq!(requests.len(), 2);
    let before = requests[0]["body"]["messages"].as_array().unwrap().len();
    let after = requests[1]["body"]["messages"].as_array().unwrap().len();
    assert!(
        after > before,
        "resumed CLI request must include prior conversation"
    );
    assert!(requests[1]["body"]["messages"]
        .to_string()
        .contains("AMBER"));
    backend.stop().await;
}

// The user may intentionally override transport credentials in settings.env.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires CI-pinned Claude CLI, Node and Sapphire npm dependencies"]
async fn pinned_cli_user_settings_can_override_transport_auth() {
    check_pin().await;
    for oauth in [false, true] {
        let fixture = Fixture::new();
        let mut backend = Backend::start(fixture.root.path()).await;
        for path in [
            fixture.home.join(".claude/settings.json"),
            fixture.work.join(".claude/settings.json"),
        ] {
            let mut settings: Value =
                serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
            if oauth {
                settings["env"]["CLAUDE_CODE_OAUTH_TOKEN"] = json!("settings-oauth-token");
            } else {
                settings["env"]["ANTHROPIC_API_KEY"] = json!("settings-api-key");
            }
            std::fs::write(path, settings.to_string()).unwrap();
        }
        let mut options = fixture.options(
            backend.port,
            Instructions::Append("Auth precedence probe".into()),
        );
        if oauth {
            options.environment.remove("ANTHROPIC_API_KEY");
            options.environment.insert(
                "CLAUDE_CODE_OAUTH_TOKEN".into(),
                "offline-oauth-token".into(),
            );
        }
        let mut session = Session::start(options, fixture.log()).await.unwrap();
        session
            .handle()
            .send_turn(turn_id(), "Reply briefly.".into())
            .await
            .unwrap();
        result(&mut session).await;
        session.pause().await.unwrap();
        let snapshot = backend.command(json!({"action":"snapshot"})).await;
        let request = &snapshot["requests"][0];
        assert_eq!(request["callerKey"], false);
        assert_eq!(request["callerOAuth"], false);
        if oauth {
            assert_eq!(request["settingsOAuth"], true);
        } else {
            assert_eq!(request["settingsKey"], true);
        }
        backend.stop().await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires CI-pinned Claude CLI, Node and Sapphire npm dependencies"]
async fn pinned_cli_can_reopen_an_unused_session() {
    check_pin().await;
    let fixture = Fixture::new();
    let backend = Backend::start(fixture.root.path()).await;
    let options = fixture.options(
        backend.port,
        Instructions::Append("Empty-session probe".into()),
    );
    let mut first = Session::start(options.clone(), fixture.log())
        .await
        .unwrap();
    let paused = first.pause().await.unwrap();
    assert!(paused.graceful);
    assert!(!paused.has_conversation);
    let mut reopened_options = options;
    reopened_options.resume = Some(Resume {
        session_id: paused.session_id.clone(),
        has_conversation: paused.has_conversation,
    });
    let mut reopened = Session::start(reopened_options, fixture.log())
        .await
        .unwrap();
    assert_ne!(reopened.session_id(), paused.session_id);
    reopened
        .handle()
        .send_turn(turn_id(), "First turn after reopening.".into())
        .await
        .unwrap();
    result(&mut reopened).await;
    reopened.pause().await.unwrap();
    backend.stop().await;
}

// No current Judgess rule emits Defer. Its mapper has a separate unit contract.
// This controlled command hook supplies that same ask payload to the real CLI,
// while the runtime's real Judgess callback abstains on the rm command.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires CI-pinned Claude CLI, Node and Sapphire npm dependencies"]
async fn pinned_cli_hook_ask_forces_a_question_in_auto_mode() {
    check_pin().await;
    let fixture = Fixture::new();
    let mut backend = Backend::start(fixture.root.path()).await;
    let marker = fixture.work.join("must-remain");
    std::fs::write(&marker, "protected by the permission answer").unwrap();
    backend
        .command(json!({"action":"configure", "toolCommand":format!("rm {}", marker.display())}))
        .await;
    let mut options = fixture.options(
        backend.port,
        Instructions::Append("Controlled Defer probe".into()),
    );
    let output = json!({"hookSpecificOutput": {
        "hookEventName":"PreToolUse", "permissionDecision":"ask",
        "permissionDecisionReason":"Judgess defers this to the user (irreversible-rm)"
    }});
    options.settings["hooks"]["PreToolUse"] = json!([{
        "matcher":"Bash", "hooks":[{"type":"command", "command":format!("printf '%s' '{}'", output)}]
    }]);
    assert_eq!(options.permission_mode, "auto");
    let mut session = Session::start(options, fixture.log()).await.unwrap();
    let handle = session.handle();
    handle
        .send_turn(turn_id(), "Perform the tool call.".into())
        .await
        .unwrap();
    let mut asked = false;
    let mut abstained = false;
    tokio::time::timeout(Duration::from_secs(45), async {
        while let Some(event) = session.next_event().await {
            match event {
                SessionEvent::ToolJudged { verdict, .. } => {
                    assert!(matches!(verdict, judgess::Verdict::Abstain(_)));
                    abstained = true;
                }
                SessionEvent::PermissionRequested {
                    request_id,
                    question,
                } => {
                    assert_eq!(question.tool_name, "Bash");
                    asked = true;
                    handle
                        .answer_permission(
                            request_id,
                            PermissionAnswer::Deny {
                                message: "User denied the deferred command".into(),
                            },
                        )
                        .await
                        .unwrap();
                }
                SessionEvent::Message(Message::Known(KnownMessage::Result(result))) => {
                    assert!(!result.is_error, "{result:?}");
                    break;
                }
                SessionEvent::Failed(error) => panic!("{error}"),
                _ => {}
            }
        }
    })
    .await
    .expect("hook ask turn deadline");
    assert!(asked && abstained);
    assert!(
        marker.exists(),
        "a denied deferred command must not execute"
    );
    session.pause().await.unwrap();
    let snapshot = backend.command(json!({"action":"snapshot"})).await;
    assert!(snapshot
        .to_string()
        .contains("User denied the deferred command"));
    backend.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires CI-pinned Claude CLI, Node and Sapphire npm dependencies"]
async fn pinned_cli_reopens_after_a_forced_stop_in_the_first_turn() {
    check_pin().await;
    first_turn_stop_roundtrip(false).await;
    first_turn_stop_roundtrip(true).await;
}

// A permission barrier exposes an existing transcript without a result, and a
// held partial stream also exercises the asynchronous transcript-write boundary.
async fn first_turn_stop_roundtrip(partial_stream: bool) {
    use std::os::unix::fs::PermissionsExt;
    check_pin().await;
    let fixture = Fixture::new();
    let mut backend = Backend::start(fixture.root.path()).await;
    backend
        .command(if partial_stream {
            json!({"action":"configure", "holdStream":true})
        } else {
            json!({"action":"configure", "toolCommand":"git status"})
        })
        .await;
    let pid_file = fixture.root.path().join("cli.pid");
    let wrapper = fixture.root.path().join("owned-cli");
    std::fs::write(
        &wrapper,
        format!(
            "#!/bin/sh\necho $$ > '{}'\nexec '{}' \"$@\"\n",
            pid_file.display(),
            pinned_cli().display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();
    let mut options = fixture.options(
        backend.port,
        Instructions::Append("First-turn stop probe".into()),
    );
    options.program = wrapper;
    options.settings["permissions"] = json!({"ask":["Bash"]});
    let mut first = Session::start(options.clone(), fixture.log())
        .await
        .unwrap();
    first
        .handle()
        .send_turn(turn_id(), "Remember FIRST_TURN_MARKER.".into())
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            let event = first.next_event().await.unwrap();
            match event {
                SessionEvent::PermissionRequested { .. } => break,
                SessionEvent::Message(Message::Known(KnownMessage::StreamEvent {
                    event:
                        devora_agent::protocol::Extensible::Known(
                            devora_agent::protocol::StreamEvent::ContentBlockDelta {
                                delta:
                                    devora_agent::protocol::Extensible::Known(
                                        devora_agent::protocol::ContentDelta::TextDelta { .. },
                                    ),
                                ..
                            },
                        ),
                    ..
                })) if partial_stream => break,
                SessionEvent::Message(Message::Known(KnownMessage::Result(_))) => {
                    panic!("unexpected result before permission barrier")
                }
                SessionEvent::Failed(error) => panic!("{error}"),
                _ => {}
            }
        }
    })
    .await
    .expect("first-turn permission barrier");
    let transcript = std::fs::read_dir(fixture.home.join(".claude/projects"))
        .unwrap()
        .filter_map(Result::ok)
        .map(|dir| dir.path().join(format!("{}.jsonl", first.session_id())))
        .find(|path| path.exists());
    if !partial_stream {
        assert!(transcript.is_some());
    }
    if let Some(file) = &transcript {
        assert!(std::fs::read_to_string(file)
            .unwrap()
            .contains("FIRST_TURN_MARKER"));
    }
    let pid = std::fs::read_to_string(&pid_file).unwrap();
    assert!(Command::new("/bin/kill")
        .args(["-STOP", pid.trim()])
        .status()
        .await
        .unwrap()
        .success());
    let stopped = first.pause().await.unwrap();
    assert!(!stopped.graceful);
    assert!(!stopped.has_conversation);
    while let Some(event) = first.next_event().await {
        assert!(!matches!(
            &event,
            SessionEvent::Message(Message::Known(KnownMessage::Result(_)))
        ));
    }
    backend.command(json!({"action":"configure"})).await;
    if !partial_stream {
        // Independent flag probe. Production never tries a flag and falls back.
        let output = tokio::time::timeout(
            Duration::from_secs(30),
            Command::new(pinned_cli())
                .args([
                    "-p",
                    "Counterfactual first turn.",
                    "--session-id",
                    &stopped.session_id,
                ])
                .current_dir(&options.working_directory)
                .env_clear()
                .envs(&options.environment)
                .output(),
        )
        .await
        .unwrap()
        .unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains(&stopped.session_id));
    }
    options.resume = Some(Resume {
        session_id: stopped.session_id.clone(),
        has_conversation: false,
    });
    let mut reopened = Session::start(options, fixture.log()).await.unwrap();
    assert_ne!(reopened.session_id(), stopped.session_id);
    reopened
        .handle()
        .send_turn(turn_id(), "Recall the first turn.".into())
        .await
        .unwrap();
    result(&mut reopened).await;
    reopened.pause().await.unwrap();
    let snapshot = backend.command(json!({"action":"snapshot"})).await;
    assert!(!snapshot.to_string().contains("FIRST_TURN_MARKER"));
    backend.stop().await;
}

/// A fresh ID for one sent turn.
fn turn_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

// Devora settles a sent turn only on the result naming its ID. The CLI keeps
// that ID as the user entry's uuid in its saved session, and the results it
// produces by itself (here, the zero-turn one on resuming after background work
// was cut short) name no user message.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires CI-pinned Claude CLI, Node and Sapphire npm dependencies"]
async fn pinned_cli_results_name_the_turn_they_answer() {
    async fn next_result(session: &mut Session) -> devora_agent::protocol::TurnResult {
        tokio::time::timeout(Duration::from_secs(45), async {
            while let Some(event) = session.next_event().await {
                match event {
                    SessionEvent::Message(Message::Known(KnownMessage::Result(result))) => {
                        return result
                    }
                    SessionEvent::Failed(error) => panic!("{error}"),
                    _ => {}
                }
            }
            panic!("CLI ended before result");
        })
        .await
        .expect("offline CLI result deadline")
    }
    check_pin().await;
    let fixture = Fixture::new();
    let mut backend = Backend::start(fixture.root.path()).await;
    // Longer than the stop's grace, so pausing cuts it short.
    backend
        .command(json!({"action":"configure", "toolCommand":"sleep 8", "toolBackground":true}))
        .await;
    let mut options = fixture.options(
        backend.port,
        Instructions::Append("Turn identity probe".into()),
    );
    options.settings["permissions"] = json!({"allow":["Bash"]});
    let mut first = Session::start(options.clone(), fixture.log())
        .await
        .unwrap();
    let sent = turn_id();
    first
        .handle()
        .send_turn(sent.clone(), "Start the background command.".into())
        .await
        .unwrap();
    assert!(next_result(&mut first).await.answers(&sent));
    let session = first.session_id();
    let saved = std::fs::read_dir(fixture.home.join(".claude/projects"))
        .unwrap()
        .filter_map(Result::ok)
        .map(|dir| dir.path().join(format!("{session}.jsonl")))
        .find(|path| path.exists())
        .expect("Claude's saved session");
    assert!(
        std::fs::read_to_string(saved)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .any(|entry| entry["type"] == "user" && entry["uuid"] == json!(sent)),
        "the saved session keeps the turn's ID as its uuid"
    );
    let paused = first.pause().await.unwrap();
    backend.command(json!({"action":"configure"})).await;
    options.resume = Some(Resume {
        session_id: paused.session_id,
        has_conversation: paused.has_conversation,
    });
    let mut resumed = Session::start(options, fixture.log()).await.unwrap();
    let unrequested = next_result(&mut resumed).await;
    assert_eq!(unrequested.num_turns, 0);
    assert!(
        !unrequested.extra.contains_key("user_message_uuids"),
        "{unrequested:?}"
    );
    let next = turn_id();
    resumed
        .handle()
        .send_turn(next.clone(), "Continue.".into())
        .await
        .unwrap();
    assert!(next_result(&mut resumed).await.answers(&next));
    resumed.pause().await.unwrap();
    backend.stop().await;
}

// Claude Code makes each background shell a session leader of its own, so only
// stopping the CLI's whole process tree ends it. A forced stop (the command
// outlives the stop's grace) must leave nothing of it running.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires CI-pinned Claude CLI, Node and Sapphire npm dependencies"]
async fn pinned_cli_forced_stop_leaves_no_background_shell_running() {
    check_pin().await;
    let fixture = Fixture::new();
    let mut backend = Backend::start(fixture.root.path()).await;
    let marker = fixture.root.path().join("background-marker");
    backend
        .command(json!({"action":"configure",
            "toolCommand":format!("sleep 60 # {}", marker.display()), "toolBackground":true}))
        .await;
    let mut options = fixture.options(
        backend.port,
        Instructions::Append("Forced stop probe".into()),
    );
    options.settings["permissions"] = json!({"allow":["Bash"]});
    let mut session = Session::start(options, fixture.log()).await.unwrap();
    session
        .handle()
        .send_turn(turn_id(), "Start the background command.".into())
        .await
        .unwrap();
    result(&mut session).await;
    let running = || {
        std::process::Command::new("pgrep")
            .args(["-f", &marker.display().to_string()])
            .output()
            .unwrap()
            .status
            .success()
    };
    assert!(running(), "the background command runs");
    let stopped = session.pause().await.unwrap();
    assert!(!stopped.graceful, "the CLI waits for it past the grace");
    assert!(!running(), "the forced stop ended the background shell too");
    backend.stop().await;
}

// A pause stops the agent's background work through the CLI (#214's design):
// the CLI lists it, `stop_task` ends a shell with no turn, and a stopped
// subagent makes the CLI run a notification turn of its own, which Devora
// interrupts as it starts. With Judgess's hook registered, as in production,
// the tool the model answers that turn with (here 1.5 s later) never runs.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires CI-pinned Claude CLI, Node and Sapphire npm dependencies"]
async fn pinned_cli_stops_background_work_and_its_interrupted_turn_runs_no_tool() {
    use devora_agent::protocol::{BackgroundTask, System};
    check_pin().await;
    let fixture = Fixture::new();
    let mut backend = Backend::start(fixture.root.path()).await;
    let root = fixture.root.path().display().to_string();
    let reacted = fixture.root.path().join("reacted");
    // Durations no other process's command line has, to find what's left by.
    let (shell_wait, agent_wait) = (100_000 + std::process::id(), 200_000 + std::process::id());
    backend
        .command(json!({"action":"configure",
            "toolCommand": format!("sleep {shell_wait}"), "toolBackground": true,
            "agentPrompt": format!("SUBAGENT_WORK {root}"),
            "subagentCommand": format!("python3 -c 'import time; time.sleep({agent_wait})'"),
            "notificationTool": {"command": format!("touch {}", reacted.display()), "delayMs": 1500}}))
        .await;
    let mut options = fixture.options(
        backend.port,
        Instructions::Append("Background work probe".into()),
    );
    // Auto mode's safety check can't run offline, and refuses the Agent tool without it.
    options.permission_mode = "default".into();
    options.settings["permissions"] = json!({"allow":["Bash","Agent"]});
    let mut session = Session::start(options, fixture.log()).await.unwrap();
    let handle = session.handle();
    let mut tasks: Vec<BackgroundTask> = vec![];
    // Waits for `done`, keeping the CLI's latest list of background work.
    async fn until(
        session: &mut Session,
        tasks: &mut Vec<BackgroundTask>,
        mut done: impl FnMut(&SessionEvent, &[BackgroundTask]) -> bool,
    ) {
        tokio::time::timeout(Duration::from_secs(45), async {
            while let Some(event) = session.next_event().await {
                if let SessionEvent::Message(Message::Known(KnownMessage::System(
                    System::BackgroundTasksChanged { tasks: listed, .. },
                ))) = &event
                {
                    *tasks = listed.clone();
                }
                if let SessionEvent::Failed(error) = &event {
                    panic!("{error}");
                }
                if done(&event, tasks) {
                    return;
                }
            }
            panic!("CLI ended");
        })
        .await
        .expect("offline CLI deadline")
    }
    let is_result = |event: &SessionEvent| {
        matches!(
            event,
            SessionEvent::Message(Message::Known(KnownMessage::Result(_)))
        )
    };
    for prompt in ["Start the shell.", "Start the subagent."] {
        handle.send_turn(turn_id(), prompt.into()).await.unwrap();
        until(&mut session, &mut tasks, |event, _| is_result(event)).await;
    }
    until(&mut session, &mut tasks, |_, tasks| tasks.len() == 2).await;
    let id = |kind: &str| {
        tasks
            .iter()
            .find(|task| task.task_type == kind)
            .unwrap_or_else(|| panic!("no {kind} in {tasks:?}"))
            .task_id
            .clone()
    };
    let (shell, agent) = (id("local_bash"), id("local_agent"));
    handle.stop_task(shell).await.unwrap();
    until(&mut session, &mut tasks, |_, tasks| tasks.len() == 1).await;
    handle.stop_task(agent).await.unwrap();
    until(&mut session, &mut tasks, |event, _| {
        matches!(
            event,
            SessionEvent::Message(Message::Known(KnownMessage::System(System::Init { .. })))
        )
    })
    .await;
    handle.interrupt().await.unwrap();
    let mut interrupted = None;
    until(&mut session, &mut tasks, |event, _| match event {
        SessionEvent::Message(Message::Known(KnownMessage::Result(result))) => {
            interrupted = Some(result.clone());
            true
        }
        _ => false,
    })
    .await;
    let interrupted = interrupted.unwrap();
    assert!(interrupted.is_error, "{interrupted:?}");
    assert!(!interrupted.extra.contains_key("user_message_uuids"));
    tokio::time::sleep(Duration::from_secs(3)).await;
    assert!(!reacted.exists(), "the interrupted turn ran no tool");
    assert!(tasks.is_empty());
    for work in [
        format!("sleep {shell_wait}"),
        format!("time.sleep.{agent_wait}."),
    ] {
        let left = std::process::Command::new("pgrep")
            .args(["-f", &work])
            .output()
            .unwrap();
        assert!(
            !left.status.success(),
            "the stopped work left nothing running: {work}"
        );
    }
    let stopped = session.pause().await.unwrap();
    assert!(
        stopped.graceful,
        "with nothing left running, the CLI exits by itself"
    );
    backend.stop().await;
}

/// Gives the fixture's home a user MCP server and a user-installed plugin with an MCP server of its own, each touching
/// a marker when it starts; the fixture's settings already give SessionStart hooks (`user-loaded`, `project-loaded`).
fn give_the_user_mcp_servers_and_a_plugin(fixture: &Fixture) {
    let marker = |name: &str| fixture.root.path().join(name);
    let server = |name: &str| json!({"type": "stdio", "command": "/bin/sh", "args": ["-c", format!("touch {}; cat >/dev/null", marker(name).display())]});
    let mut config: Value = std::fs::read_to_string(fixture.home.join(".claude.json"))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_else(|| json!({}));
    config["mcpServers"] = json!({"user-server": server("user-mcp-started")});
    std::fs::write(fixture.home.join(".claude.json"), config.to_string()).unwrap();
    let marketplace = fixture.root.path().join("marketplace");
    let plugin = marketplace.join("probe-plugin");
    std::fs::create_dir_all(marketplace.join(".claude-plugin")).unwrap();
    std::fs::create_dir_all(plugin.join(".claude-plugin")).unwrap();
    std::fs::write(
        marketplace.join(".claude-plugin/marketplace.json"),
        json!({"name": "local", "owner": {"name": "probe"}, "plugins": [{"name": "probe-plugin", "source": "./probe-plugin"}]}).to_string(),
    )
    .unwrap();
    std::fs::write(
        plugin.join(".claude-plugin/plugin.json"),
        json!({"name": "probe-plugin", "version": "1.0.0"}).to_string(),
    )
    .unwrap();
    std::fs::write(
        plugin.join(".mcp.json"),
        json!({"mcpServers": {"plugin-server": server("plugin-mcp-started")}}).to_string(),
    )
    .unwrap();
    let settings_path = fixture.home.join(".claude/settings.json");
    let mut settings: Value =
        serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
    settings["extraKnownMarketplaces"] =
        json!({"local": {"source": {"source": "directory", "path": marketplace}}});
    settings["enabledPlugins"] = json!({"probe-plugin@local": true});
    std::fs::write(&settings_path, settings.to_string()).unwrap();
}

/// Whether a marker a background process touches appears, within a bounded number of looks: MCP servers start after
/// the initialize reply.
fn appears(path: &std::path::Path) -> bool {
    (0..100).any(|_| {
        let there = path.exists();
        if !there {
            std::thread::sleep(Duration::from_millis(100));
        }
        there
    })
}

/// An initialize-only launch with no isolation flags, as the probe would be without them; returns once it has replied.
async fn plain_initialize(fixture: &Fixture, options: &SessionOptions) {
    let mut child = Command::new(&options.program)
        .args([
            "-p",
            "--input-format",
            "stream-json",
            "--output-format",
            "stream-json",
            "--verbose",
        ])
        .env_clear()
        .envs(&options.environment)
        .current_dir(&fixture.work)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap()).lines();
    input
        .write_all(b"{\"type\":\"control_request\",\"request_id\":\"plain\",\"request\":{\"subtype\":\"initialize\",\"hooks\":{}}}\n")
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(30), async {
        while let Some(line) = output.next_line().await.unwrap() {
            if line.contains("\"control_response\"") {
                break;
            }
        }
    })
    .await
    .expect("plain initialize deadline");
    drop(input);
    let _ = tokio::time::timeout(Duration::from_secs(20), child.wait()).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires CI-pinned Claude CLI, Node and Sapphire npm dependencies"]
async fn listing_the_models_starts_none_of_the_users_hooks_mcp_servers_or_plugins_and_saves_no_session(
) {
    check_pin().await;
    let server_root = tempfile::tempdir().unwrap();
    let backend = Backend::start(server_root.path()).await;
    let markers = [
        "user-loaded",
        "project-loaded",
        "user-mcp-started",
        "plugin-mcp-started",
    ];

    // The setup starts each of them in a plain launch, so their absence below is the probe's doing.
    let control = Fixture::new();
    give_the_user_mcp_servers_and_a_plugin(&control);
    let options = control.options(backend.port, Instructions::Append(String::new()));
    plain_initialize(&control, &options).await;
    let started: Vec<_> = markers
        .iter()
        .filter(|name| appears(&control.root.path().join(name)))
        .collect();
    assert_eq!(
        started.len(),
        markers.len(),
        "the plain launch should start them all, started only {started:?}"
    );

    let fixture = Fixture::new();
    give_the_user_mcp_servers_and_a_plugin(&fixture);
    let mut options = fixture.options(backend.port, Instructions::Append(String::new()));
    options.settings = json!({});
    let list = devora_agent::models::discover(&options).await.unwrap();
    assert!(!list.models.is_empty(), "{list:?}");
    assert!(list.cli_version.is_some());
    for name in markers {
        assert!(
            !fixture.root.path().join(name).exists(),
            "the probe started {name}"
        );
    }
    let sessions = fixture.home.join(".claude/sessions");
    let left: Vec<_> = std::fs::read_dir(&sessions)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|e| e.file_name())
                .collect()
        })
        .unwrap_or_default();
    assert!(left.is_empty(), "the probe left session files: {left:?}");
    backend.stop().await;
}

// The context Devora tracks is the agent's own: a subagent's messages report the subagent's context, so the
// usage the session passes on comes from the main thread alone. The CLI streams no events for a subagent, only its
// whole messages, which name it (`parent_tool_use_id`).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires CI-pinned Claude CLI, Node and Sapphire npm dependencies"]
async fn pinned_cli_usage_is_the_main_threads_while_a_subagent_runs() {
    check_pin().await;
    let fixture = Fixture::new();
    let mut backend = Backend::start(fixture.root.path()).await;
    let (main, subagent) = (100_000_u64, 3_000_u64);
    backend
        .command(json!({"action":"configure", "inputTokens": main,
            "agentPrompt": "SUBAGENT_CONTEXT", "subagentCommand": "true",
            "subagentInputTokens": subagent, "agentBackground": false}))
        .await;
    let mut options = fixture.options(
        backend.port,
        Instructions::Append("Subagent usage probe".into()),
    );
    // Auto mode's safety check can't run offline, and refuses the Agent tool without it.
    options.permission_mode = "default".into();
    options.settings["permissions"] = json!({"allow":["Bash","Agent"]});
    let mut session = Session::start(options, fixture.log()).await.unwrap();
    let handle = session.handle();
    let input_tokens = |usage: &Value| usage["input_tokens"].as_u64().unwrap_or(0);
    let mut passed_on = vec![];
    let mut subagent_messages = vec![];
    for prompt in ["Hello.", "Start the subagent."] {
        handle.send_turn(turn_id(), prompt.into()).await.unwrap();
        tokio::time::timeout(Duration::from_secs(45), async {
            while let Some(event) = session.next_event().await {
                match &event {
                    SessionEvent::Usage { source, usage } => {
                        passed_on.push((format!("{source:?}"), input_tokens(usage)))
                    }
                    SessionEvent::Message(Message::Known(KnownMessage::Assistant {
                        message,
                        extra,
                    })) if extra
                        .get("parent_tool_use_id")
                        .is_some_and(|p| !p.is_null()) =>
                    {
                        subagent_messages
                            .push(("assistant", input_tokens(&message.extra["usage"])));
                    }
                    SessionEvent::Message(Message::Known(KnownMessage::StreamEvent {
                        extra,
                        ..
                    })) if extra
                        .get("parent_tool_use_id")
                        .is_some_and(|p| !p.is_null()) =>
                    {
                        subagent_messages.push(("stream_event", 0));
                    }
                    SessionEvent::Message(Message::Known(KnownMessage::Result(result))) => {
                        assert!(!result.is_error, "{result:?}");
                        return;
                    }
                    SessionEvent::Failed(error) => panic!("{error}"),
                    _ => {}
                }
            }
            panic!("CLI ended");
        })
        .await
        .expect("offline CLI deadline");
    }
    assert!(
        subagent_messages
            .iter()
            .any(|(kind, _)| *kind == "assistant")
            && subagent_messages
                .iter()
                .all(|(kind, _)| *kind == "assistant"),
        "the subagent ran, and only its whole messages came, naming it: {subagent_messages:?}"
    );
    assert!(
        passed_on.iter().all(|(_, tokens)| *tokens != subagent),
        "no usage passed on is the subagent's ({subagent}); passed on: {passed_on:?}; the subagent's messages: {subagent_messages:?}"
    );
    session.pause().await.unwrap();
    backend.stop().await;
}

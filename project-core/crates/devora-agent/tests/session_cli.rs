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
            effort: "high".into(),
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
    let workflow = std::fs::read_to_string(repo.join(".github/workflows/ci.yml")).unwrap();
    let pin = workflow
        .lines()
        .find_map(|line| line.trim().strip_prefix("CLAUDE_CODE_VERSION:"))
        .unwrap()
        .trim()
        .trim_matches('"');
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
            .send_turn("Reply briefly.".into())
            .await
            .unwrap();
        result(&mut session).await;
        if index == 0 {
            let handle = session.handle();
            handle.set_model("sonnet".into()).await.unwrap();
            handle.set_permission_mode("plan".into()).await.unwrap();
            handle.send_turn("Reply once more.".into()).await.unwrap();
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
    handle.send_turn("First turn.".into()).await.unwrap();
    let first = result(&mut session).await;
    assert!(first["usage"]["input_tokens"].as_u64().unwrap() > 50_000);
    handle.send_turn("Second turn.".into()).await.unwrap();
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
            &options.effort,
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
        let turn = Message::user_text(id.clone(), prompt.into());
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
        .send_turn("Perform the tool call.".into())
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
        .send_turn("Perform the tool call.".into())
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
        .send_turn("Remember the word AMBER.".into())
        .await
        .unwrap();
    result(&mut first).await;
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
        .send_turn("Recall the earlier word.".into())
        .await
        .unwrap();
    result(&mut resumed).await;
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
            .send_turn("Reply briefly.".into())
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
        .send_turn("First turn after reopening.".into())
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
        .send_turn("Perform the tool call.".into())
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
        .send_turn("Remember FIRST_TURN_MARKER.".into())
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
        .send_turn("Recall the first turn.".into())
        .await
        .unwrap();
    result(&mut reopened).await;
    reopened.pause().await.unwrap();
    let snapshot = backend.command(json!({"action":"snapshot"})).await;
    assert!(!snapshot.to_string().contains("FIRST_TURN_MARKER"));
    backend.stop().await;
}

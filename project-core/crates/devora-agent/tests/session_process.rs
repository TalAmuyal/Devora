#![cfg(unix)]

use devora_agent::protocol::{Extensible, KnownMessage, Message};
use devora_agent::session::*;
use devora_app::log::Log;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

/// Writes the fake CLI as an executable whose shebang is Python's own path, since the session clears `PATH`.
/// Python writes it, not this process: a child another test thread forks while this process has the file open for
/// writing would inherit that descriptor until its own exec, and exec'ing the file meanwhile fails on Linux with
/// "Text file busy". The tests' spawns are inside `Session::start`, where no lock of theirs can reach.
fn write_fake_cli(path: &Path) {
    let source = include_str!("support/fake_cli.py")
        .split_once('\n')
        .unwrap()
        .1;
    let status = std::process::Command::new("python3")
        .args([
            "-c",
            "import os, sys\nwith open(sys.argv[1], 'w') as file: file.write('#!' + sys.executable + '\\n' + sys.argv[2])\nos.chmod(sys.argv[1], 0o755)",
        ])
        .arg(path)
        .arg(source)
        .status()
        .expect("Python 3 is required for the deterministic subprocess peer");
    assert!(status.success());
}

struct Fixture {
    root: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        write_fake_cli(&root.path().join("fake-cli"));
        Self { root }
    }

    fn options(&self) -> SessionOptions {
        let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        SessionOptions {
            program: self.root.path().join("fake-cli"),
            working_directory: self.root.path().into(),
            instructions: Instructions::Append("Caller instructions".into()),
            model: "haiku".into(),
            effort: Some("high".into()),
            permission_mode: "auto".into(),
            plugins: vec![
                Plugin {
                    name: "judgess".into(),
                    directory: "/excluded/judgess".into(),
                },
                Plugin {
                    name: "other".into(),
                    directory: "/included/other".into(),
                },
            ],
            settings: json!({"env":{"DISABLE_AUTO_COMPACT":"0","CONFIGURED":"yes"}}),
            environment: BTreeMap::from_iter([
                ("PATH".into(), std::env::var("PATH").unwrap()),
                ("HOME".into(), self.root.path().to_str().unwrap().into()),
                (
                    "DEVORA_FAKE_FIXTURES".into(),
                    crate_root
                        .join("tests/fixtures/claude-2.1.312")
                        .to_str()
                        .unwrap()
                        .into(),
                ),
                (
                    "DEVORA_FAKE_REPORT".into(),
                    self.root
                        .path()
                        .join("report.jsonl")
                        .to_str()
                        .unwrap()
                        .into(),
                ),
                ("EXPLICIT_CALLER_ENV".into(), "yes".into()),
                (
                    "ANTHROPIC_API_KEY".into(),
                    "fake-secret-transport-value".into(),
                ),
            ]),
            resume: None,
        }
    }

    fn log(&self) -> Arc<Log> {
        Arc::new(Log::open(&self.root.path().join("session.log")).unwrap())
    }

    fn reports(&self) -> Vec<Value> {
        std::fs::read_to_string(self.root.path().join("report.jsonl"))
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }
}

async fn event(session: &mut Session, matches: impl Fn(&SessionEvent) -> bool) -> SessionEvent {
    tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(event) = session.next_event().await {
            if matches(&event) {
                return event;
            }
        }
        panic!("session ended before expected event");
    })
    .await
    .expect("session event deadline")
}

async fn turn_ended(session: &mut Session) {
    event(session, |event| {
        matches!(
            event,
            SessionEvent::Message(Message::Known(KnownMessage::Result(_)))
        )
    })
    .await;
}

#[tokio::test]
async fn launch_is_explicit_and_a_session_runs_multiple_turns() {
    let fixture = Fixture::new();
    let mut session = Session::start(fixture.options(), fixture.log())
        .await
        .unwrap();
    let handle = session.handle();
    handle.send_turn(turn_id(), "first".into()).await.unwrap();
    let usage = event(&mut session, |event| {
        matches!(
            event,
            SessionEvent::Usage {
                source: UsageSource::Assistant,
                ..
            }
        )
    })
    .await;
    let SessionEvent::Usage { usage, .. } = usage else {
        unreachable!()
    };
    assert!(usage["input_tokens"].is_number());
    turn_ended(&mut session).await;
    handle.send_turn(turn_id(), "second".into()).await.unwrap();
    turn_ended(&mut session).await;
    session.pause().await.unwrap();
    let reports = fixture.reports();
    let start = &reports[0];
    assert_eq!(
        start["cwd"],
        fixture
            .root
            .path()
            .canonicalize()
            .unwrap()
            .to_str()
            .unwrap()
    );
    assert_eq!(start["environment"]["EXPLICIT_CALLER_ENV"], "yes");
    assert_eq!(start["environment"]["DISABLE_AUTO_COMPACT"], "1");
    let env = start["environment"].as_object().unwrap();
    let allowed = [
        "PATH",
        "HOME",
        "DEVORA_FAKE_FIXTURES",
        "DEVORA_FAKE_REPORT",
        "EXPLICIT_CALLER_ENV",
        "ANTHROPIC_API_KEY",
        "DISABLE_AUTO_COMPACT",
        // Python initializes locale and CoreFoundation text encoding on macOS.
        "LC_CTYPE",
        "__CF_USER_TEXT_ENCODING",
    ];
    assert!(
        env.keys().all(|key| allowed.contains(&key.as_str())),
        "{env:?}"
    );
    let args = start["args"].as_array().unwrap();
    assert!(!args.contains(&json!("/excluded/judgess")));
    assert!(args.contains(&json!("/included/other")));
    assert!(!args.contains(&json!("--setting-sources")));
    assert!(!args.contains(&json!("--bare")));
    let settings = args.iter().position(|value| value == "--settings").unwrap();
    let settings: Value = serde_json::from_str(args[settings + 1].as_str().unwrap()).unwrap();
    assert_eq!(settings["env"]["DISABLE_AUTO_COMPACT"], "1");
    assert_eq!(settings["env"]["CONFIGURED"], "yes");
    assert!(!settings["env"]
        .as_object()
        .unwrap()
        .contains_key("ANTHROPIC_API_KEY"));
    assert!(!args.iter().any(|argument| argument
        .as_str()
        .is_some_and(|value| value.contains("fake-secret-transport-value"))));
}

#[tokio::test]
async fn questions_can_apply_suggested_rules_and_withdrawn_answers_are_rejected() {
    let fixture = Fixture::new();
    let mut session = Session::start(fixture.options(), fixture.log())
        .await
        .unwrap();
    let handle = session.handle();
    handle
        .send_turn(turn_id(), "permission".into())
        .await
        .unwrap();
    let SessionEvent::PermissionRequested {
        request_id,
        question,
    } = event(&mut session, |event| {
        matches!(event, SessionEvent::PermissionRequested { .. })
    })
    .await
    else {
        unreachable!()
    };
    assert_eq!(question.decision_reason(), Some("Ask rule"));
    assert_eq!(question.tool_use_id, "tool-1");
    handle
        .answer_permission(request_id, PermissionAnswer::AllowWithSuggestedRule)
        .await
        .unwrap();
    let SessionEvent::Message(Message::Raw(answer))=event(&mut session,|event|matches!(event,SessionEvent::Message(Message::Raw(value)) if value["type"]=="fake_answer")).await else { unreachable!() };
    assert_eq!(answer["answer"]["behavior"], "allow");
    assert_eq!(
        answer["answer"]["updatedPermissions"][0]["destination"],
        "localSettings"
    );
    turn_ended(&mut session).await;
    handle
        .send_turn(turn_id(), "cancel-permission".into())
        .await
        .unwrap();
    event(&mut session,|event|matches!(event,SessionEvent::PermissionCancelled { request_id } if request_id=="question")).await;
    assert!(handle
        .answer_permission("question".into(), PermissionAnswer::Allow)
        .await
        .unwrap_err()
        .contains("no longer pending"));
    turn_ended(&mut session).await;
    session.pause().await.unwrap();
}

#[tokio::test]
async fn judgess_and_mcp_are_driven_over_the_real_child_stdio() {
    let fixture = Fixture::new();
    let mut session = Session::start(fixture.options(), fixture.log())
        .await
        .unwrap();
    let handle = session.handle();
    handle
        .send_turn(turn_id(), "hook:eval foo".into())
        .await
        .unwrap();
    let SessionEvent::ToolJudged {
        tool_use_id,
        verdict,
    } = event(&mut session, |event| {
        matches!(event, SessionEvent::ToolJudged { .. })
    })
    .await
    else {
        unreachable!()
    };
    assert_eq!(tool_use_id.as_deref(), Some("tool-1"));
    assert_eq!(verdict.kind().as_str(), "deny");
    let SessionEvent::Message(Message::Raw(answer))=event(&mut session,|event|matches!(event,SessionEvent::Message(Message::Raw(value)) if value["type"]=="fake_answer")).await else { unreachable!() };
    assert_eq!(
        answer["answer"]["hookSpecificOutput"]["permissionDecision"],
        "deny"
    );
    turn_ended(&mut session).await;
    handle.send_turn(turn_id(), "mcp".into()).await.unwrap();
    let SessionEvent::McpRequested {
        request_id,
        message,
        ..
    } = event(&mut session, |event| {
        matches!(event, SessionEvent::McpRequested { .. })
    })
    .await
    else {
        unreachable!()
    };
    assert_eq!(message["method"], "tools/list");
    handle
        .answer_mcp(
            request_id,
            json!({"jsonrpc":"2.0","id":1,"result":{"tools":[]}}),
        )
        .await
        .unwrap();
    let SessionEvent::Message(Message::Raw(answer)) = event(&mut session, |event|
        matches!(event,SessionEvent::Message(Message::Raw(value)) if value["type"]=="fake_answer")
    ).await else { unreachable!() };
    assert_eq!(
        answer["answer"]["mcp_response"],
        json!({"jsonrpc":"2.0","id":1,"result":{"tools":[]}})
    );
    turn_ended(&mut session).await;
    session.pause().await.unwrap();
}

#[tokio::test]
async fn pause_interrupts_before_eof_and_reopening_resumes_the_same_id() {
    let fixture = Fixture::new();
    let mut session = Session::start(fixture.options(), fixture.log())
        .await
        .unwrap();
    let handle = session.handle();
    handle.send_turn(turn_id(), "stream".into()).await.unwrap();
    event(&mut session,|event|matches!(event,SessionEvent::Message(Message::Raw(value)) if value["type"]=="fake_stream_ready")).await;
    let stopped = session.pause().await.unwrap();
    assert!(stopped.graceful);
    let mut interruption = false;
    let mut stopped_event = false;
    while let Some(event) = session.next_event().await {
        match event {
            SessionEvent::Message(Message::Known(KnownMessage::Result(result))) => {
                interruption = result.is_error
            }
            SessionEvent::Stopped(info) => {
                assert_eq!(info, stopped);
                stopped_event = true;
            }
            _ => {}
        }
    }
    assert!(
        interruption && stopped_event,
        "pause must retain the final transcript events"
    );
    let reports = fixture.reports();
    let interrupt = reports
        .iter()
        .position(|value| value["kind"] == "interrupt")
        .unwrap();
    let eof = reports
        .iter()
        .position(|value| value["kind"] == "eof")
        .unwrap();
    assert!(interrupt < eof);
    let mut options = fixture.options();
    options.resume = Some(Resume {
        session_id: stopped.session_id.clone(),
        has_conversation: stopped.has_conversation,
    });
    let mut reopened = Session::start(options, fixture.log()).await.unwrap();
    assert_eq!(reopened.session_id(), stopped.session_id);
    event(&mut reopened, |event| {
        matches!(event, SessionEvent::Ready { resumed: true, .. })
    })
    .await;
    reopened.pause().await.unwrap();
}

#[tokio::test]
async fn crash_recovery_preserves_id_model_and_mode_and_bounds_crash_loops() {
    let fixture = Fixture::new();
    let mut session = Session::start(fixture.options(), fixture.log())
        .await
        .unwrap();
    let handle = session.handle();
    handle.set_model("sonnet".into()).await.unwrap();
    handle.set_permission_mode("plan".into()).await.unwrap();
    let id = session.session_id().to_owned();
    handle
        .send_turn(turn_id(), "establish conversation".into())
        .await
        .unwrap();
    turn_ended(&mut session).await;
    handle.send_turn(turn_id(), "crash".into()).await.unwrap();
    event(&mut session, |event| {
        matches!(event, SessionEvent::Ready { resumed: true, .. })
    })
    .await;
    let starts: Vec<_> = fixture
        .reports()
        .into_iter()
        .filter(|value| value["kind"] == "start")
        .collect();
    assert_eq!(starts.len(), 2);
    let args = starts[1]["args"].as_array().unwrap();
    for (key, value) in [
        ("--resume", id.as_str()),
        ("--model", "sonnet"),
        ("--permission-mode", "plan"),
    ] {
        let index = args.iter().position(|arg| arg == key).unwrap();
        assert_eq!(args[index + 1], value);
    }
    handle.send_turn(turn_id(), "crash".into()).await.unwrap();
    event(&mut session, |event| {
        matches!(event, SessionEvent::Failed(_))
    })
    .await;
    assert_eq!(
        fixture
            .reports()
            .iter()
            .filter(|value| value["kind"] == "start")
            .count(),
        2
    );
    assert!(session.pause().await.is_err());
}

#[tokio::test]
async fn usage_compaction_and_once_per_kind_drift_are_visible() {
    let fixture = Fixture::new();
    let log = fixture.log();
    let mut session = Session::start(fixture.options(), log).await.unwrap();
    let handle = session.handle();
    handle.send_turn(turn_id(), "drift".into()).await.unwrap();
    let mut warnings = 0;
    let mut raw_assistants = 0;
    loop {
        match session.next_event().await.unwrap() {
            SessionEvent::Warning(message) if message.contains("protocol drift") => warnings += 1,
            SessionEvent::Message(Message::Raw(value)) if value["type"] == "assistant" => {
                raw_assistants += 1
            }
            SessionEvent::Message(Message::Known(KnownMessage::Result(_))) => break,
            _ => {}
        }
    }
    assert_eq!((warnings, raw_assistants), (1, 2));
    handle.send_turn(turn_id(), "compact".into()).await.unwrap();
    let SessionEvent::Compacted(metadata) = event(&mut session, |event| {
        matches!(event, SessionEvent::Compacted(_))
    })
    .await
    else {
        unreachable!()
    };
    assert_eq!(metadata.trigger, "manual");
    turn_ended(&mut session).await;
    session.pause().await.unwrap();
    let log = std::fs::read_to_string(fixture.root.path().join("session.log")).unwrap();
    assert_eq!(log.matches("protocol drift").count(), 1);
}

#[tokio::test]
async fn initialization_and_invalid_protocol_fail_visibly() {
    let fixture = Fixture::new();
    let mut options = fixture.options();
    options
        .environment
        .insert("DEVORA_FAKE_INIT_ERROR".into(), "1".into());
    let refused = Session::start(options, fixture.log()).await.err().unwrap();
    assert!(refused.contains("initialize refused"), "{refused}");
    let mut session = Session::start(fixture.options(), fixture.log())
        .await
        .unwrap();
    session
        .handle()
        .send_turn(turn_id(), "malformed".into())
        .await
        .unwrap();
    let SessionEvent::Failed(error) = event(&mut session, |event| {
        matches!(event, SessionEvent::Failed(_))
    })
    .await
    else {
        unreachable!()
    };
    assert!(error.contains("invalid Claude protocol JSON"));
    assert!(session.pause().await.is_err());
}

#[tokio::test]
async fn an_unsupported_cli_is_rejected_before_session_start() {
    let fixture = Fixture::new();
    let mut options = fixture.options();
    options
        .environment
        .insert("DEVORA_FAKE_VERSION".into(), "2.1.311".into());
    assert!(Session::start(options, fixture.log())
        .await
        .err()
        .unwrap()
        .contains("unsupported"));
    assert!(!fixture.root.path().join("report.jsonl").exists());
}

#[tokio::test]
async fn an_unknown_request_stays_raw_and_gets_an_explicit_unsupported_response() {
    let fixture = Fixture::new();
    let mut session = Session::start(fixture.options(), fixture.log())
        .await
        .unwrap();
    let handle = session.handle();
    handle
        .send_turn(turn_id(), "unknown-request".into())
        .await
        .unwrap();
    event(&mut session, |event| {
        matches!(event,
            SessionEvent::Message(Message::Known(KnownMessage::ControlRequest {
                request: Extensible::Raw(value), ..
            })) if value["subtype"] == "future_request"
        )
    })
    .await;
    let SessionEvent::Message(Message::Raw(answer))=event(&mut session,|event|matches!(event,SessionEvent::Message(Message::Raw(value)) if value["type"]=="fake_answer")).await else { unreachable!() };
    assert_eq!(answer["answer"]["error"], "unsupported CLI control request");
    turn_ended(&mut session).await;
    let stopped = handle.pause().await.unwrap();
    let SessionEvent::Stopped(event) = event(&mut session, |event| {
        matches!(event, SessionEvent::Stopped(_))
    })
    .await
    else {
        unreachable!()
    };
    assert_eq!(event, stopped);
    assert!(handle.send_turn(turn_id(), "late".into()).await.is_err());
    drop(session);
}

#[tokio::test]
async fn an_unresponsive_process_is_reaped_after_the_pause_grace_period() {
    let fixture = Fixture::new();
    let mut session = Session::start(fixture.options(), fixture.log())
        .await
        .unwrap();
    session
        .handle()
        .send_turn(turn_id(), "ignore-interrupt".into())
        .await
        .unwrap();
    event(&mut session,|event|matches!(event,SessionEvent::Message(Message::Raw(value)) if value["type"]=="fake_stream_ready")).await;
    let stopped = tokio::time::timeout(Duration::from_secs(10), session.pause())
        .await
        .expect("bounded pause")
        .unwrap();
    assert!(!stopped.graceful);
    let reports = fixture.reports();
    assert!(reports.iter().any(|report| report["kind"] == "interrupt"));
}

#[tokio::test]
async fn an_unused_session_recovers_with_a_fresh_id_without_resuming() {
    let fixture = Fixture::new();
    let mut options = fixture.options();
    options.environment.insert(
        "DEVORA_FAKE_CRASH_UNUSED".into(),
        fixture
            .root
            .path()
            .join("crashed-once")
            .to_str()
            .unwrap()
            .into(),
    );
    let mut session = Session::start(options, fixture.log()).await.unwrap();
    event(&mut session, |event| {
        matches!(
            event,
            SessionEvent::Exited {
                restarting: true,
                ..
            }
        )
    })
    .await;
    event(&mut session, |event| {
        matches!(event, SessionEvent::Ready { resumed: false, .. })
    })
    .await;
    let starts: Vec<_> = fixture
        .reports()
        .into_iter()
        .filter(|row| row["kind"] == "start")
        .collect();
    assert_eq!(starts.len(), 2);
    let ids: Vec<_> = starts
        .iter()
        .map(|start| {
            let args = start["args"].as_array().unwrap();
            assert!(!args.contains(&json!("--resume")));
            let index = args.iter().position(|arg| arg == "--session-id").unwrap();
            args[index + 1].as_str().unwrap().to_owned()
        })
        .collect();
    assert_ne!(ids[0], ids[1]);
    assert_eq!(session.session_id(), ids[1]);
    assert!(!session.pause().await.unwrap().has_conversation);
}

#[tokio::test]
async fn a_control_timeout_uses_the_same_bounded_recovery_as_a_crash() {
    let fixture = Fixture::new();
    let mut session = Session::start(fixture.options(), fixture.log())
        .await
        .unwrap();
    let handle = session.handle();
    handle
        .send_turn(turn_id(), "establish conversation".into())
        .await
        .unwrap();
    turn_ended(&mut session).await;
    for attempt in 0..2 {
        let control = handle.clone();
        let reply = tokio::spawn(async move { control.set_model("hang".into()).await });
        event(&mut session, |event| matches!(event,
            SessionEvent::Message(Message::Raw(value)) if value["type"] == "fake_control_waiting"
        )).await;
        // Advance only after the child confirms it received the request. Real
        // subprocess I/O remains on the running clock during restart.
        tokio::time::pause();
        tokio::time::advance(Duration::from_secs(30)).await;
        tokio::time::resume();
        assert!(reply.await.unwrap().is_err());
        event(&mut session, |event| {
            matches!(event,
                SessionEvent::Exited { restarting, .. } if *restarting == (attempt == 0)
            )
        })
        .await;
        if attempt == 0 {
            event(&mut session, |event| {
                matches!(event, SessionEvent::Ready { resumed: true, .. })
            })
            .await;
        } else {
            event(&mut session, |event| {
                matches!(event, SessionEvent::Failed(_))
            })
            .await;
        }
    }
    assert_eq!(
        fixture
            .reports()
            .iter()
            .filter(|row| row["kind"] == "start")
            .count(),
        2
    );
}

#[tokio::test]
async fn a_first_turn_crash_before_result_starts_fresh_even_after_assistant_output() {
    let fixture = Fixture::new();
    let mut session = Session::start(fixture.options(), fixture.log())
        .await
        .unwrap();
    session
        .handle()
        .send_turn(turn_id(), "assistant-then-crash".into())
        .await
        .unwrap();
    event(&mut session, |event| {
        matches!(
            event,
            SessionEvent::Message(Message::Known(KnownMessage::Assistant { .. }))
        )
    })
    .await;
    event(&mut session, |event| {
        matches!(event, SessionEvent::Ready { resumed: false, .. })
    })
    .await;
    assert!(!session.pause().await.unwrap().has_conversation);
    let starts: Vec<_> = fixture
        .reports()
        .into_iter()
        .filter(|r| r["kind"] == "start")
        .collect();
    assert!(starts[1]["args"]
        .as_array()
        .unwrap()
        .contains(&json!("--session-id")));
}

/// A fresh ID for one sent turn.
fn turn_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

// The CLI also emits results nobody sent a turn for: one for a turn it starts
// itself when background work finishes, and a zero-turn one on resume. Only the
// result naming the sent turn's ID ends it.
#[tokio::test]
async fn only_the_result_naming_the_sent_turn_ends_it() {
    let fixture = Fixture::new();
    let mut session = Session::start(fixture.options(), fixture.log())
        .await
        .unwrap();
    let handle = session.handle();
    let id = turn_id();
    handle
        .send_turn(id.clone(), "unrequested-result".into())
        .await
        .unwrap();
    let SessionEvent::Message(Message::Known(KnownMessage::Result(unrequested))) =
        event(&mut session, |event| {
            matches!(
                event,
                SessionEvent::Message(Message::Known(KnownMessage::Result(_)))
            )
        })
        .await
    else {
        unreachable!()
    };
    assert!(!unrequested.answers(&id));
    event(&mut session,|event|matches!(event,SessionEvent::Message(Message::Raw(value)) if value["type"]=="fake_stream_ready")).await;
    assert_eq!(
        handle.send_turn(turn_id(), "second".into()).await,
        Err("a turn is already running".into()),
        "a result the CLI started by itself must not end the sent turn"
    );
    handle.interrupt().await.unwrap();
    let SessionEvent::Message(Message::Known(KnownMessage::Result(answer))) =
        event(&mut session, |event| {
            matches!(
                event,
                SessionEvent::Message(Message::Known(KnownMessage::Result(_)))
            )
        })
        .await
    else {
        unreachable!()
    };
    assert!(answer.answers(&id));
    handle.send_turn(turn_id(), "second".into()).await.unwrap();
    turn_ended(&mut session).await;
    session.pause().await.unwrap();
    let sent: Vec<_> = fixture
        .reports()
        .into_iter()
        .filter(|report| report["kind"] == "user")
        .map(|report| report["uuid"].clone())
        .collect();
    assert_eq!(sent.len(), 2);
    assert_eq!(
        sent[0],
        json!(id),
        "the CLI receives the turn's ID as its uuid"
    );
}

#[tokio::test]
async fn the_session_reports_the_cli_it_spawned() {
    let fixture = Fixture::new();
    let mut session = Session::start(fixture.options(), fixture.log())
        .await
        .unwrap();
    let SessionEvent::Spawned(cli) = event(&mut session, |event| {
        matches!(event, SessionEvent::Spawned(_))
    })
    .await
    else {
        unreachable!()
    };
    let started = fixture
        .reports()
        .into_iter()
        .find(|report| report["kind"] == "start")
        .unwrap();
    assert_eq!(json!(cli.pid), started["pid"]);
    session.pause().await.unwrap();
}

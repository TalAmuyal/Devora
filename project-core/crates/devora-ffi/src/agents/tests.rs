use super::*;
use crate::test_support::{profile, Profile};
use devora_core::state::Command;
use futures_lite::future::{block_on, poll_once};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::sync::mpsc;
use std::time::Duration;

struct Fixture {
    profile: Profile,
    core: Arc<DevoraCore>,
    program: String,
    config: String,
}

impl Fixture {
    fn new(persistent: bool) -> Self {
        let profile = profile(None);
        fs::write(profile.path.join("config.json"), r#"{"claude":{"default-model":"sonnet","effort":"high","sonnet-model":"profile-sonnet"}}"#).unwrap();
        let config = profile.root.path().join("global.json");
        fs::write(
            &config,
            r#"{"claude":{"default-model":"haiku","effort":"low"}}"#,
        )
        .unwrap();
        let python = std::process::Command::new("python3")
            .args(["-c", "import sys; print(sys.executable)"])
            .output()
            .unwrap();
        assert!(python.status.success());
        let program = profile.root.path().join("fake-cli");
        let source = include_str!("../../../devora-agent/tests/support/fake_cli.py")
            .split_once('\n')
            .unwrap()
            .1;
        fs::write(
            &program,
            format!(
                "#!{}\n{source}",
                String::from_utf8(python.stdout).unwrap().trim()
            ),
        )
        .unwrap();
        fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).unwrap();
        let log = crate::log::test_log();
        let state = persistent.then(|| {
            profile
                .root
                .path()
                .join("state.db")
                .to_string_lossy()
                .into_owned()
        });
        let core =
            DevoraCore::new(Some(config.to_string_lossy().into_owned()), state, log).unwrap();
        Self {
            profile,
            core,
            program: program.to_string_lossy().into_owned(),
            config: config.to_string_lossy().into_owned(),
        }
    }

    fn launch(&self) -> AgentLaunch {
        AgentLaunch {
            program: self.program.clone(),
            plugins: vec![],
            settings_json: r#"{"env":{"HOST_ONLY":"kept"}}"#.into(),
            environment: HashMap::from([
                ("PATH".into(), std::env::var("PATH").unwrap()),
                (
                    "DEVORA_FAKE_FIXTURES".into(),
                    Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("../devora-agent/tests/fixtures/claude-2.1.312")
                        .to_string_lossy()
                        .into_owned(),
                ),
                (
                    "DEVORA_FAKE_REPORT".into(),
                    self.profile
                        .root
                        .path()
                        .join("report.jsonl")
                        .to_string_lossy()
                        .into_owned(),
                ),
            ]),
        }
    }

    fn create(&self, name: &str) {
        let creation = self
            .core
            .create_agent_space(
                self.profile.path_string(),
                AgentCreationForm {
                    name: name.into(),
                    role: None,
                    repos: vec![],
                    inputs: HashMap::new(),
                },
                HashMap::new(),
            )
            .unwrap();
        loop {
            let batch = block_on(creation.events.next_batch(100)).unwrap();
            assert!(!batch.is_empty());
            if batch
                .iter()
                .any(|e| matches!(e, CreationEvent::Done { .. }))
            {
                break;
            }
            assert!(
                !batch
                    .iter()
                    .any(|e| matches!(e, CreationEvent::Failed { .. } | CreationEvent::Cancelled)),
                "{batch:?}"
            );
        }
    }

    fn open(&self, name: &str) -> u64 {
        block_on(
            self.core
                .open_agent_space(self.profile.path_string(), name.into(), self.launch()),
        )
        .unwrap()
    }

    fn wait(
        &self,
        events: &Arc<ChatSubscription>,
        mut predicate: impl FnMut(&Chat) -> bool,
    ) -> Chat {
        loop {
            if let Some(chat) = self
                .core
                .agent_chat_snapshot()
                .unwrap()
                .into_iter()
                .find(|c| predicate(c))
            {
                return chat;
            }
            let subscription = events.clone();
            let (sent, receive) = mpsc::channel();
            let worker = std::thread::spawn(move || {
                let _ = sent.send(block_on(subscription.next_batch(100)));
            });
            let received = receive.recv_timeout(Duration::from_secs(12));
            if received.is_err() {
                events.close().unwrap();
            }
            worker.join().unwrap();
            assert!(
                !received.expect("chat event timed out").unwrap().is_empty(),
                "chat feed closed"
            );
        }
    }
}

fn has_message(chat: &Chat, kind: &str) -> bool {
    chat.items.iter().any(|item| match &item.content {
        TranscriptContent::System {
            kind: SystemKind::Result,
            ..
        } => kind == "result",
        TranscriptContent::Assistant { .. } => kind == "assistant",
        TranscriptContent::Unknown { json } => {
            serde_json::from_str::<serde_json::Value>(json).unwrap()["type"] == kind
        }
        _ => false,
    })
}

#[test]
fn live_facade_owns_session_without_subscribers_and_closes_before_pause_finishes() {
    let f = Fixture::new(false);
    f.create("Delta");
    let tab = f.open("Delta");
    let events = f.core.subscribe_agent_chats().unwrap();
    let chat = f.wait(&events, |c| c.tab == tab && c.phase == ChatPhase::Running);
    let (first, second) = block_on(futures_lite::future::zip(
        f.core
            .open_agent_space(f.profile.path_string(), "Delta".into(), f.launch()),
        f.core
            .open_agent_space(f.profile.path_string(), "Delta".into(), f.launch()),
    ));
    assert_eq!(first.unwrap(), tab);
    assert_eq!(second.unwrap(), tab);
    events.close().unwrap();
    block_on(
        f.core
            .send_agent_turn(tab, chat.generation, "ignore-interrupt".into()),
    )
    .unwrap();
    let events = f.core.subscribe_agent_chats().unwrap();
    f.wait(&events, |c| has_message(c, "fake_stream_ready"));
    // Generic tab closure uses the same lifecycle path as close_agent_space.
    f.core.dispatch(Command::CloseSession { tab }).unwrap();
    assert!(f.core.snapshot().unwrap().window.tabs.is_empty());
    assert!(f.core.agent_chat_snapshot().unwrap().is_empty());
    assert!(block_on(
        f.core
            .delete_agent_space(f.profile.path_string(), "Delta".into())
    )
    .is_err());
    let mut reopen = std::pin::pin!(f.core.open_agent_space(
        f.profile.path_string(),
        "Delta".into(),
        f.launch()
    ));
    assert!(
        block_on(poll_once(reopen.as_mut())).is_none(),
        "reopen must wait for the pausing owner"
    );
    let next_tab = block_on(reopen).unwrap();
    let next = f.wait(&events, |c| {
        c.tab == next_tab && c.phase == ChatPhase::Running
    });
    assert!(next.items.iter().any(|item| matches!(&item.content, TranscriptContent::User { text, .. } if text == "ignore-interrupt")));
    assert!(next.items.iter().any(|item| matches!(
        item.content,
        TranscriptContent::System {
            kind: SystemKind::UnfinishedFirstPrompt,
            ..
        }
    )));
    let saved = block_on(
        f.core
            .agent_history(f.profile.path_string(), "Delta".into(), 0, 500),
    )
    .unwrap();
    assert_eq!(saved, next.items);
    let tail =
        block_on(
            f.core
                .agent_history_before(f.profile.path_string(), "Delta".into(), None, 2),
        )
        .unwrap();
    assert_eq!(tail, saved[saved.len() - 2..]);
    let older = block_on(f.core.agent_history_before(
        f.profile.path_string(),
        "Delta".into(),
        Some(tail[0].sequence),
        500,
    ))
    .unwrap();
    assert_eq!(older, saved[..saved.len() - 2]);

    let first_page = block_on(
        f.core
            .agent_history(f.profile.path_string(), "Delta".into(), 0, 2),
    )
    .unwrap();
    let second_page = block_on(f.core.agent_history(
        f.profile.path_string(),
        "Delta".into(),
        first_page.last().unwrap().sequence,
        2,
    ))
    .unwrap();
    assert!(second_page[0].sequence > first_page.last().unwrap().sequence);
    assert_ne!(next.session_id, chat.session_id);
    assert_ne!(next.generation, chat.generation);
    assert!(block_on(
        f.core
            .send_agent_turn(next_tab, chat.generation, "old".into())
    )
    .is_err());
    block_on(
        f.core
            .send_agent_turn(next_tab, next.generation, "permission".into()),
    )
    .unwrap();
    f.wait(&events, |c| !c.questions.is_empty());
    assert!(block_on(f.core.answer_agent_permission(
        next_tab,
        chat.generation,
        "question".into(),
        PermissionAnswer::Allow
    ))
    .is_err());
    {
        let mut answer = std::pin::pin!(f.core.answer_agent_permission(
            next_tab,
            next.generation,
            "question".into(),
            PermissionAnswer::AllowWithSuggestedRule,
        ));
        let _ = block_on(poll_once(answer.as_mut()));
        // Cancellation of the caller's wait must still retire the answered card.
    }
    f.wait(&events, |c| {
        c.questions.is_empty() && has_message(c, "result")
    });
    block_on(f.core.pause_agent_space(next_tab)).unwrap();
    block_on(f.core.pause_agent_space(next_tab)).unwrap();
    assert_eq!(
        f.core.agent_chat_snapshot().unwrap()[0].phase,
        ChatPhase::Paused
    );
    let resumed_tab = f.open("Delta");
    let resumed = f.wait(&events, |c| {
        c.tab == resumed_tab && c.phase == ChatPhase::Running
    });
    assert_eq!(resumed.session_id, next.session_id);
    f.core.close_agent_space(resumed_tab).unwrap();
    block_on(f.core.shutdown_agents()).unwrap();
    // Shutdown waits for OS ownership release, independently of observation.
    let space = devora_core::agents::Agents::new(
        Arc::new(devora_core::state::Store::new()),
        Arc::new(devora_core::log::Log::open(&f.profile.root.path().join("other.log")).unwrap()),
    )
    .unwrap();
    block_on(space.delete(&f.profile.path, "Delta")).unwrap();
}

#[test]
fn permissions_interrupt_recovery_and_owning_profile_are_projected() {
    let f = Fixture::new(false);
    f.create("Bravo");
    let tab = f.open("Bravo");
    let events = f.core.subscribe_agent_chats().unwrap();
    let chat = f.wait(&events, |c| c.tab == tab && c.phase == ChatPhase::Running);
    block_on(
        f.core
            .send_agent_turn(tab, chat.generation, "cancel-permission".into()),
    )
    .unwrap();
    f.wait(&events, |c| {
        c.questions.is_empty() && has_message(c, "result")
    });
    assert!(block_on(f.core.answer_agent_permission(
        tab,
        chat.generation,
        "question".into(),
        PermissionAnswer::Allow
    ))
    .is_err());
    block_on(
        f.core
            .send_agent_turn(tab, chat.generation, "stream".into()),
    )
    .unwrap();
    f.wait(&events, |c| has_message(c, "fake_stream_ready"));
    block_on(f.core.interrupt_agent(tab, chat.generation)).unwrap();
    f.wait(&events, |c| {
        !c.turn_running
            && c.items.iter().any(|i| {
                matches!(
                    i.content,
                    TranscriptContent::System {
                        kind: SystemKind::Interruption,
                        ..
                    }
                )
            })
    });
    block_on(f.core.send_agent_turn(tab, chat.generation, "crash".into())).unwrap();
    let recovered = f.wait(&events, |c| {
        c.phase == ChatPhase::Running && c.generation != chat.generation
    });
    assert_eq!(
        recovered.session_id, chat.session_id,
        "completed conversation resumes"
    );
    assert!(block_on(f.core.send_agent_turn(tab, chat.generation, "stale".into())).is_err());
    let report = fs::read_to_string(f.profile.root.path().join("report.jsonl")).unwrap();
    let starts: Vec<serde_json::Value> = report
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .filter(|v: &serde_json::Value| v["kind"] == "start")
        .collect();
    let args = starts[0]["args"].as_array().unwrap();
    assert_eq!(
        args[args.iter().position(|v| v == "--model").unwrap() + 1],
        "sonnet"
    );
    let settings: serde_json::Value = serde_json::from_str(
        args[args.iter().position(|v| v == "--settings").unwrap() + 1]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        settings["env"]["ANTHROPIC_DEFAULT_SONNET_MODEL"],
        "profile-sonnet"
    );
    assert_eq!(settings["env"]["HOST_ONLY"], "kept");
    assert_eq!(
        starts[0]["cwd"],
        f.profile
            .path
            .join("agent-spaces/Bravo")
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .as_ref()
    );
    block_on(f.core.shutdown_agents()).unwrap();
}

#[test]
fn persisted_tabs_restore_the_typed_transcript_and_missing_spaces_fail_on_their_tab() {
    let mut f = Fixture::new(true);
    f.create("Restored");
    let tab = f.open("Restored");
    let events = f.core.subscribe_agent_chats().unwrap();
    let chat = f.wait(&events, |c| c.phase == ChatPhase::Running);
    block_on(f.core.send_agent_turn(tab, chat.generation, "hello".into())).unwrap();
    f.wait(&events, |c| has_message(c, "result"));
    block_on(f.core.shutdown_agents()).unwrap();
    events.close().unwrap();
    let log = crate::log::test_log();
    let old = std::mem::replace(
        &mut f.core,
        DevoraCore::new(Some(f.config.clone()), None, log.clone()).unwrap(),
    );
    drop(old);
    f.core = DevoraCore::new(
        Some(f.config.clone()),
        Some(
            f.profile
                .root
                .path()
                .join("state.db")
                .to_string_lossy()
                .into_owned(),
        ),
        log,
    )
    .unwrap();
    assert_eq!(f.core.snapshot().unwrap().window.tabs[0].id, tab);
    assert!(f.core.agent_chat_snapshot().unwrap().is_empty());
    block_on(f.core.restore_agent_spaces(f.launch())).unwrap();
    let events = f.core.subscribe_agent_chats().unwrap();
    let restored = f.wait(&events, |c| c.phase == ChatPhase::Running);
    assert_eq!(restored.session_id, chat.session_id);
    assert!(has_message(&restored, "assistant"));
    let failed = f.open("Missing");
    f.wait(&events, |c| c.tab == failed && c.phase == ChatPhase::Failed);
    assert!(f
        .core
        .snapshot()
        .unwrap()
        .window
        .tabs
        .iter()
        .any(|t| t.id == failed));
    block_on(f.core.shutdown_agents()).unwrap();
}

#[test]
fn creation_progress_cancel_failure_and_invalid_listing_use_the_facade() {
    let f = Fixture::new(false);
    fs::write(
        f.profile.path.join("config.json"),
        r#"{"prepare-command":"printf 'barrier-ready\n'; sleep 120"}"#,
    )
    .unwrap();
    let creation = f
        .core
        .create_agent_space(
            f.profile.path_string(),
            AgentCreationForm {
                name: "Cancelled".into(),
                role: None,
                repos: vec![f.profile.repo.to_string_lossy().into_owned()],
                inputs: HashMap::new(),
            },
            HashMap::new(),
        )
        .unwrap();
    let mut cancelled = false;
    loop {
        let batch = block_on(creation.events.next_batch(100)).unwrap();
        assert!(!batch.is_empty());
        for event in batch {
            match event {
                CreationEvent::Log { line } if line == "barrier-ready" => {
                    f.core.cancel_agent_creation(creation.id).unwrap()
                }
                CreationEvent::Cancelled => cancelled = true,
                CreationEvent::Failed { message } => panic!("{message}"),
                _ => {}
            }
        }
        if cancelled {
            break;
        }
    }
    assert!(!f.profile.path.join("agent-spaces/Cancelled").exists());
    let failed = f
        .core
        .create_agent_space(
            f.profile.path_string(),
            AgentCreationForm {
                name: "../bad".into(),
                role: None,
                repos: vec![],
                inputs: HashMap::new(),
            },
            HashMap::new(),
        )
        .unwrap();
    assert!(block_on(failed.events.next_batch(100))
        .unwrap()
        .iter()
        .any(|e| matches!(e, CreationEvent::Failed { .. })));
    fs::create_dir(f.profile.path.join("agent-spaces/Incomplete")).unwrap();
    let list = f.core.list_agent_spaces(f.profile.path_string()).unwrap();
    assert_eq!(list.len(), 1);
    assert!(list[0].error.is_some());
    block_on(
        f.core
            .delete_agent_space(f.profile.path_string(), "Incomplete".into()),
    )
    .unwrap();
}

#[test]
fn deferred_pause_finishes_the_turn_keeps_permissions_and_rejects_new_sends() {
    let f = Fixture::new(false);
    let config = f.profile.path.join("config.json");
    let mut profile: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&config).unwrap()).unwrap();
    profile["agent"] = serde_json::json!({"compact-threshold": 1});
    fs::write(config, profile.to_string()).unwrap();
    f.create("Pause");
    let tab = f.open("Pause");
    let events = f.core.subscribe_agent_chats().unwrap();
    let chat = f.wait(&events, |c| c.phase == ChatPhase::Running);
    assert!(f
        .core
        .pause_agent_after_turn(tab, chat.generation + 1)
        .is_err());
    block_on(
        f.core
            .send_agent_turn(tab, chat.generation, "permission".into()),
    )
    .unwrap();
    let waiting = f.wait(&events, |c| !c.questions.is_empty());
    assert!(waiting.questions[0]
        .suggested_rules_json
        .as_deref()
        .unwrap()
        .contains("destination"));
    f.core.pause_agent_after_turn(tab, chat.generation).unwrap();
    // Repeated clicks must not replace the actor's pending completion sender.
    f.core.pause_agent_after_turn(tab, chat.generation).unwrap();
    let pending = f.core.agent_chat_snapshot().unwrap().remove(0);
    assert!(pending.pause_requested && pending.turn_running);
    assert_eq!(pending.questions.len(), 1);
    assert!(block_on(f.core.send_agent_turn(tab, chat.generation, "later".into())).is_err());
    block_on(f.core.answer_agent_permission(
        tab,
        chat.generation,
        "question".into(),
        PermissionAnswer::Allow,
    ))
    .unwrap();
    let paused = f.wait(&events, |c| c.phase == ChatPhase::Paused);
    assert!(has_message(&paused, "result"));
    assert!(!paused.pause_requested);
    assert_eq!(paused.context.unwrap().threshold_percent, 1);
    assert!(!paused.items.iter().any(|item| matches!(
        item.content,
        TranscriptContent::System {
            kind: SystemKind::Compaction,
            ..
        }
    )));
    let report = fs::read_to_string(f.profile.root.path().join("report.jsonl")).unwrap();
    let prompts: Vec<serde_json::Value> = report
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .filter(|record| record["kind"] == "user")
        .collect();
    assert_eq!(
        prompts.len(),
        1,
        "a pending Pause must win over compaction: {prompts:?}"
    );
    assert_eq!(f.open("Pause"), tab);
    let resumed = f.wait(&events, |c| c.phase == ChatPhase::Running);
    assert_eq!(resumed.session_id, paused.session_id);
    f.core
        .pause_agent_after_turn(tab, resumed.generation)
        .unwrap();
    f.wait(&events, |c| c.phase == ChatPhase::Paused);
    block_on(f.core.shutdown_agents()).unwrap();
}

#[test]
fn accepted_turn_clears_its_draft_even_when_the_callers_wait_is_cancelled() {
    let f = Fixture::new(false);
    f.create("Draft");
    let tab = f.open("Draft");
    let events = f.core.subscribe_agent_chats().unwrap();
    let chat = f.wait(&events, |c| c.phase == ChatPhase::Running);
    let profile = fs::canonicalize(&f.profile.path)
        .unwrap()
        .to_string_lossy()
        .into_owned();
    f.core
        .dispatch(Command::SetAgentDraft {
            profile_path: profile,
            name: "Draft".into(),
            text: "submitted".into(),
        })
        .unwrap();
    {
        let mut turn =
            std::pin::pin!(f
                .core
                .send_agent_turn(tab, chat.generation, "submitted".into()));
        let _ = block_on(poll_once(turn.as_mut()));
    }
    let deadline = std::time::Instant::now() + Duration::from_secs(12);
    while !f.core.snapshot().unwrap().agent_views[0].draft.is_empty() {
        assert!(
            std::time::Instant::now() < deadline,
            "cancelled caller abandoned draft acknowledgement"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    block_on(f.core.shutdown_agents()).unwrap();
}

#[test]
fn deletion_retry_finishes_presentation_cleanup_after_the_directory_is_gone() {
    let f = Fixture::new(false);
    f.create("Deleted");
    block_on(
        f.core
            .delete_agent_space(f.profile.path_string(), "Deleted".into()),
    )
    .unwrap();
    // This is the state left if filesystem deletion succeeded but its following
    // state transaction failed. Retrying must complete that remaining stage.
    let profile = fs::canonicalize(&f.profile.path)
        .unwrap()
        .to_string_lossy()
        .into_owned();
    f.core
        .dispatch(Command::SetAgentDraft {
            profile_path: profile,
            name: "Deleted".into(),
            text: "old draft".into(),
        })
        .unwrap();
    block_on(
        f.core
            .delete_agent_space(f.profile.path_string(), "Deleted".into()),
    )
    .unwrap();
    assert!(f.core.snapshot().unwrap().agent_views.is_empty());
}

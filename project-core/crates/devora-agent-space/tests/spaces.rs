use devora_agent::protocol::{KnownMessage, Message};
use devora_agent::session::{stop_tree, CliProcess, SessionEvent};
use devora_agent::transcript::{SystemKind, TranscriptChange, TranscriptContent};
use devora_agent_space::{
    model_defaults, template, AppRun, ChosenEffort, ChosenModel, CreationForm, LaunchContext,
    ModelChoice, RunningSpace, Space, SpaceEvent,
};
use devora_app::log::Log;
use devora_platform::processes::{ProcessTable, SystemProcessTable};
use devora_workspace::creation::{CreationHandle, EventSink};
use serde_json::json;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

struct Fixture {
    root: tempfile::TempDir,
    profile: PathBuf,
    global: PathBuf,
    /// One app process: launches in it owe no restart notice.
    run: AppRun,
}
impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let profile = root.path().join("profile");
        std::fs::create_dir(&profile).unwrap();
        std::fs::write(profile.join("config.json"), r#"{"prepare-command":"true"}"#).unwrap();
        let global = root.path().join("global.json");
        std::fs::write(&global, "{}").unwrap();
        Self {
            root,
            profile,
            global,
            run: AppRun::new(),
        }
    }
    fn form(&self, name: &str) -> CreationForm {
        CreationForm {
            name: name.into(),
            role: None,
            repos: vec![],
            inputs: BTreeMap::new(),
            keep_coding_instructions: true,
            model: None,
        }
    }
    fn create(&self, form: CreationForm) -> Result<Space, String> {
        self.create_with(form, BTreeMap::new(), &CreationHandle::new())
    }
    fn create_with(
        &self,
        form: CreationForm,
        inputs: BTreeMap<String, String>,
        handle: &CreationHandle,
    ) -> Result<Space, String> {
        let events: EventSink = Arc::new(|_| {});
        Space::create(
            &self.profile,
            form,
            inputs,
            &self.global,
            handle,
            &events,
            &mut vec![],
        )
    }
    fn repo(&self, name: &str) -> PathBuf {
        let source = self.profile.join("repos").join(name);
        std::fs::create_dir_all(&source).unwrap();
        git(&source, &["init", "-q", "-b", "main"]);
        git(
            &source,
            &[
                "-c",
                "user.name=fixture",
                "-c",
                "user.email=fixture@example.test",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "--allow-empty",
                "-qm",
                "initial",
            ],
        );
        let remote = self.root.path().join(format!("{name}-remote.git"));
        git(
            self.root.path(),
            &[
                "clone",
                "--bare",
                source.to_str().unwrap(),
                remote.to_str().unwrap(),
            ],
        );
        git(
            &source,
            &["remote", "add", "origin", remote.to_str().unwrap()],
        );
        git(&source, &["fetch", "origin"]);
        git(
            &source,
            &[
                "symbolic-ref",
                "refs/remotes/origin/HEAD",
                "refs/remotes/origin/main",
            ],
        );
        source
    }
    fn context(&self) -> LaunchContext {
        let source = include_str!("../../devora-agent/tests/support/fake_cli.py")
            .split_once('\n')
            .unwrap()
            .1;
        let program = self.root.path().join("fake-cli");
        // Python writes the executable, not this process: a child another test thread forks while this process has
        // it open for writing would inherit the descriptor until its own exec, and exec'ing the file meanwhile fails
        // on Linux with "Text file busy".
        let written = Command::new("python3")
            .args([
                "-c",
                "import os, sys\nwith open(sys.argv[1], 'w') as file: file.write('#!' + sys.executable + '\\n' + sys.argv[2])\nos.chmod(sys.argv[1], 0o755)",
            ])
            .arg(&program)
            .arg(source)
            .status()
            .unwrap();
        assert!(written.success());
        LaunchContext {
            global_config: self.global.clone(),
            program,
            plugins: vec![],
            settings: json!({"env":{"SPACE_CONFIG":"explicit"}}),
            environment: BTreeMap::from([
                ("PATH".into(), std::env::var("PATH").unwrap()),
                ("HOME".into(), self.root.path().display().to_string()),
                (
                    "CLAUDE_CONFIG_DIR".into(),
                    self.root.path().join("claude").display().to_string(),
                ),
                (
                    "DEVORA_FAKE_FIXTURES".into(),
                    Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("../devora-agent/tests/fixtures/claude-2.1.312")
                        .display()
                        .to_string(),
                ),
                (
                    "DEVORA_FAKE_REPORT".into(),
                    self.root.path().join("report.jsonl").display().to_string(),
                ),
            ]),
        }
    }
}
fn git(cwd: &Path, args: &[&str]) -> String {
    let result = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout).unwrap()
}

#[test]
fn creates_lists_locks_and_deletes_real_worktrees() {
    let fixture = Fixture::new();
    let source = fixture.repo("repo");
    let mut form = fixture.form("Delta");
    form.repos.push(source.display().to_string());
    let space = fixture.create(form.clone()).unwrap();
    let path = space.path().to_path_buf();
    assert!(path.join("memory").is_dir());
    assert!(path.join("repo/.git").is_file());
    assert!(!path.join("CLAUDE.md").exists());
    assert!(!path.join("task.json").exists());
    assert_eq!(
        Space::list(&fixture.profile).unwrap()[0]
            .identity
            .as_ref()
            .unwrap()
            .name,
        "Delta"
    );
    assert!(fixture
        .create(form)
        .err()
        .unwrap()
        .contains("already exists"));
    assert!(Space::open(&fixture.profile, "Delta")
        .err()
        .unwrap()
        .contains("in use"));
    assert!(Space::delete(&fixture.profile, "Delta")
        .unwrap_err()
        .contains("in use"));
    drop(space);
    Space::delete(&fixture.profile, "Delta").unwrap();
    assert!(!path.exists());
    assert!(!git(&source, &["worktree", "list", "--porcelain"]).contains(path.to_str().unwrap()));
}

#[test]
fn validates_names_repos_and_cleans_failed_or_cancelled_creation() {
    let fixture = Fixture::new();
    for name in [
        "",
        "..",
        "../outside",
        "nested/name",
        "hidden\\path",
        " leading",
        ".hidden",
        "line\nbreak",
    ] {
        assert!(fixture.create(fixture.form(name)).is_err(), "{name}");
    }
    let reserved = fixture.repo("memory");
    let mut form = fixture.form("bad-repo");
    form.repos.push(reserved.display().to_string());
    assert!(fixture
        .create(form)
        .err()
        .unwrap()
        .contains("reserved repo"));
    let source = fixture.repo("broken");
    git(
        &source,
        &[
            "remote",
            "set-url",
            "origin",
            fixture.root.path().join("missing.git").to_str().unwrap(),
        ],
    );
    let mut form = fixture.form("failed");
    form.repos.push(source.display().to_string());
    assert!(fixture.create(form).is_err());
    assert!(!fixture.profile.join("agent-spaces/failed").exists());
    let handle = CreationHandle::new();
    handle.request_cancel();
    assert!(fixture
        .create_with(fixture.form("cancelled"), BTreeMap::new(), &handle)
        .is_err());
    assert!(!fixture.profile.join("agent-spaces/cancelled").exists());
}

#[test]
fn frozen_rendering_merges_inputs_and_survives_template_removal() {
    let fixture = Fixture::new();
    let template = fixture.profile.join("instructions.md");
    std::fs::write(
        &template,
        "{{agent.name}}|{{agent.role}}|{{value}}|{{global}}|{{user.details}}",
    )
    .unwrap();
    std::fs::write(&fixture.global,json!({"agent":{"role":"global role","inputs":{"value":"global","global":"from global"},"user-details":"global user"}}).to_string()).unwrap();
    std::fs::write(fixture.profile.join("config.json"),json!({"agent":{"template-file":"instructions.md","role":"profile role","inputs":{"value":"profile"},"user-details":"profile user"}}).to_string()).unwrap();
    let mut form = fixture.form("Delta");
    form.role = Some("form role".into());
    form.inputs
        .insert("value".into(), "form {{literal}}".into());
    let space = fixture
        .create_with(
            form,
            BTreeMap::from([("value".into(), "launch".into())]),
            &CreationHandle::new(),
        )
        .unwrap();
    assert_eq!(
        space.instructions(),
        "Delta|form role|form {{literal}}|from global|profile user\n"
    );
    let id = space.saved_session().session_id.clone();
    drop(space);
    std::fs::remove_file(template).unwrap();
    std::fs::write(
        fixture.profile.join("config.json"),
        r#"{"agent":{"template-file":"gone.md","role":"changed"},"claude":{"default-model":"sonnet","effort":"low"}}"#,
    )
    .unwrap();
    let reopened = Space::open(&fixture.profile, "Delta").unwrap();
    assert!(reopened.instructions().contains("form role"));
    assert_eq!(reopened.saved_session().session_id, id);
    assert!(!reopened.saved_session().has_conversation);
    assert_eq!(reopened.form().inputs["value"], "form {{literal}}");
    let options = reopened.session_options(&fixture.context()).unwrap();
    assert_eq!(options.model, "sonnet");
    assert_eq!(options.effort.as_deref(), Some("low"));
    assert!(
        matches!(options.instructions,devora_agent::session::Instructions::Append(text) if text.contains("form role"))
    );
}

#[test]
fn missing_or_malformed_placeholders_fail_before_reservation() {
    let fixture = Fixture::new();
    let file = fixture.profile.join("template.md");
    std::fs::write(
        fixture.profile.join("config.json"),
        r#"{"agent":{"template-file":"template.md"}}"#,
    )
    .unwrap();
    for text in [
        "{{missing}}",
        "{{unclosed",
        "{{bad key}}",
        "unmatched}}",
        "{{nested{{key}}",
    ] {
        std::fs::write(&file, text).unwrap();
        assert!(fixture.create(fixture.form("bad")).is_err(), "{text}");
        assert!(!fixture.profile.join("agent-spaces/bad").exists());
    }
    assert_eq!(
        template::render(
            "{{k}}",
            &BTreeMap::from([("k".into(), "{{literal}}".into())])
        )
        .unwrap(),
        "{{literal}}"
    );
}

#[test]
fn incomplete_and_future_schema_entries_are_visible_and_deletable() {
    let fixture = Fixture::new();
    let space = fixture.create(fixture.form("incomplete")).unwrap();
    let path = space.path().join(".agent-space.db");
    drop(space);
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection.execute("UPDATE space SET ready=0", []).unwrap();
    drop(connection);
    assert!(Space::list(&fixture.profile).unwrap()[0]
        .error
        .as_ref()
        .unwrap()
        .contains("incomplete"));
    Space::delete(&fixture.profile, "incomplete").unwrap();
    let space = fixture.create(fixture.form("future")).unwrap();
    let path = space.path().join(".agent-space.db");
    drop(space);
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection.pragma_update(None, "user_version", 999).unwrap();
    drop(connection);
    assert!(Space::open(&fixture.profile, "future")
        .err()
        .unwrap()
        .contains("version 999"));
    Space::delete(&fixture.profile, "future").unwrap();
    std::fs::create_dir(fixture.profile.join("agent-spaces/abandoned")).unwrap();
    assert!(Space::list(&fixture.profile).unwrap()[0].error.is_some());
    Space::delete(&fixture.profile, "abandoned").unwrap();
}

#[tokio::test]
async fn session_events_are_persisted_before_delivery_and_reopen_roundtrips() {
    let fixture = Fixture::new();
    let context = fixture.context();
    let space = fixture.create(fixture.form("Delta")).unwrap();
    let path = space.path().join(".agent-space.db");
    let creation_id = space.saved_session().session_id.clone();
    let options = space.session_options(&context).unwrap();
    assert_eq!(options.permission_mode, "auto");
    assert_eq!(options.working_directory, space.path());
    let log = Arc::new(Log::open(&fixture.root.path().join("log")).unwrap());
    let mut running = space
        .start(&context, log.clone(), &fixture.run)
        .await
        .unwrap();
    let id = match running.next_event().await.unwrap() {
        SessionEvent::Ready {
            session_id,
            resumed: false,
            ..
        } => session_id,
        event => panic!("unexpected initial event: {event:?}"),
    };
    assert_ne!(id, creation_id);
    let stored_id: String = rusqlite::Connection::open(&path)
        .unwrap()
        .query_row("SELECT session_id FROM space", [], |row| row.get(0))
        .unwrap();
    assert_eq!(stored_id, id);
    running
        .handle()
        .send_turn("first turn".into())
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(event) = running.next_event().await {
            match event {
                SessionEvent::Message(Message::Known(KnownMessage::Result(result))) => {
                    assert_eq!(result.session_id, id);
                    let db = rusqlite::Connection::open(&path).unwrap();
                    assert!(db
                        .query_row("SELECT has_conversation FROM space", [], |row| row
                            .get::<_, bool>(0))
                        .unwrap());
                    break;
                }
                SessionEvent::Failed(error) => panic!("{error}"),
                _ => {}
            }
        }
    })
    .await
    .unwrap();
    let stopped = running.pause().await.unwrap();
    assert!(stopped.has_conversation);
    let reopened = Space::open(&fixture.profile, "Delta").unwrap();
    assert_eq!(reopened.saved_session().session_id, id);
    assert!(reopened.saved_session().has_conversation);
    let mut resumed = reopened.start(&context, log, &fixture.run).await.unwrap();
    assert!(matches!(
        resumed.next_event().await,
        Some(SessionEvent::Ready { resumed: true, .. })
    ));
    resumed.pause().await.unwrap();
}

#[tokio::test]
async fn dropping_after_a_cancelled_pause_persists_before_releasing_ownership() {
    let fixture = Fixture::new();
    let context = fixture.context();
    let space = fixture.create(fixture.form("Delta")).unwrap();
    let log = Arc::new(Log::open(&fixture.root.path().join("drop.log")).unwrap());
    let mut running = space.start(&context, log, &fixture.run).await.unwrap();
    running.handle().send_turn("stream".into()).await.unwrap();
    tokio::time::timeout(Duration::from_secs(10),async {
        while let Some(event) = running.next_event().await {
            if matches!(event,SessionEvent::Message(Message::Raw(value)) if value["type"] == "fake_stream_ready") { break; }
        }
    }).await.unwrap();
    let handle = running.handle();
    let mut pausing = Box::pin(handle.pause());
    std::future::poll_fn(|cx| {
        use std::future::Future;
        assert!(pausing.as_mut().poll(cx).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
    drop(pausing);
    drop(running);
    // Opening succeeds only after the asynchronous drop path has stopped the
    // child, written its StopInfo and released the OS lock.
    let reopened = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            match Space::open(&fixture.profile, "Delta") {
                Ok(space) => break space,
                Err(error) if error.contains("in use") => tokio::task::yield_now().await,
                Err(error) => panic!("{error}"),
            }
        }
    })
    .await
    .unwrap();
    assert!(reopened.saved_session().has_conversation);
}

#[test]
fn a_held_creation_keeps_the_catalogue_available_for_other_spaces() {
    use devora_workspace::creation::CreationEvent;
    use std::sync::{mpsc, Mutex};
    let fixture = Fixture::new();
    assert!(Space::list(&fixture.profile).unwrap().is_empty());
    assert!(
        !fixture.profile.join("agent-spaces").exists(),
        "listing is read-only"
    );
    drop(fixture.create(fixture.form("existing")).unwrap());
    drop(fixture.create(fixture.form("deletable")).unwrap());
    let source = fixture.repo("repo");
    let mut form = fixture.form("creating");
    form.repos.push(source.display().to_string());
    let (entered, waiting) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let released = Mutex::new(released);
    let events: EventSink = Arc::new(move |event| {
        if matches!(event,CreationEvent::Step{label} if label == "Fetching repo") {
            entered.send(()).unwrap();
            released
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(20))
                .unwrap();
        }
    });
    let profile = fixture.profile.clone();
    let global = fixture.global.clone();
    let worker = std::thread::spawn(move || {
        Space::create(
            &profile,
            form,
            BTreeMap::new(),
            &global,
            &CreationHandle::new(),
            &events,
            &mut vec![],
        )
    });
    waiting.recv_timeout(Duration::from_secs(10)).unwrap();
    // Listing must also complete while a short catalogue mutation owns the lock.
    let catalogue = std::fs::OpenOptions::new()
        .write(true)
        .open(fixture.profile.join("agent-spaces/.lock"))
        .unwrap();
    fs2::FileExt::lock_exclusive(&catalogue).unwrap();
    let profile = fixture.profile.clone();
    let (listed, listing) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        listed.send(Space::list(&profile)).unwrap();
    });
    let result = listing.recv_timeout(Duration::from_secs(5));
    fs2::FileExt::unlock(&catalogue).unwrap();
    reader.join().unwrap();
    let entries = result
        .expect("listing must not wait for the catalogue lock")
        .unwrap();
    assert!(entries
        .iter()
        .find(|entry| entry.name == "creating")
        .unwrap()
        .error
        .as_ref()
        .unwrap()
        .contains("incomplete"));
    drop(Space::open(&fixture.profile, "existing").unwrap());
    Space::delete(&fixture.profile, "deletable").unwrap();
    release.send(()).unwrap();
    let completed = worker.join().unwrap().unwrap();
    assert_eq!(completed.identity().name, "creating");
}

#[test]
fn with_no_model_tier_set_an_agent_launches_with_none_and_claude_code_picks() {
    let fixture = Fixture::new();
    std::fs::write(&fixture.global, json!({}).to_string()).unwrap();
    let space = fixture.create(fixture.form("Delta")).unwrap();
    let options = space.session_options(&fixture.context()).unwrap();
    for tier in [
        "ANTHROPIC_DEFAULT_OPUS_MODEL",
        "ANTHROPIC_DEFAULT_SONNET_MODEL",
        "ANTHROPIC_DEFAULT_HAIKU_MODEL",
    ] {
        assert!(
            options.settings["env"].get(tier).is_none(),
            "{tier} in {}",
            options.settings
        );
    }
}

#[test]
fn profile_model_aliases_reach_inline_settings_and_explicit_host_values_win() {
    let fixture = Fixture::new();
    std::fs::write(
        &fixture.global,
        json!({"claude":{
            "opus-model":"global-opus","sonnet-model":"global-sonnet","haiku-model":"global-haiku"
        }})
        .to_string(),
    )
    .unwrap();
    std::fs::write(fixture.profile.join("config.json"),json!({"claude":{
        "opus-model":"profile-opus","sonnet-model":"profile-sonnet","haiku-model":null,"permission-mode":"plan"
    }}).to_string()).unwrap();
    let space = fixture.create(fixture.form("Delta")).unwrap();
    let mut context = fixture.context();
    context.settings = json!({"env":{"ANTHROPIC_DEFAULT_OPUS_MODEL":"host-opus","HOST_ONLY":"keep"},"host_setting":true});
    let options = space.session_options(&context).unwrap();
    assert_eq!(
        options.settings["env"]["ANTHROPIC_DEFAULT_OPUS_MODEL"],
        "host-opus"
    );
    assert_eq!(
        options.settings["env"]["ANTHROPIC_DEFAULT_SONNET_MODEL"],
        "profile-sonnet"
    );
    assert!(options.settings["env"]
        .get("ANTHROPIC_DEFAULT_HAIKU_MODEL")
        .is_none());
    assert_eq!(options.settings["env"]["HOST_ONLY"], "keep");
    assert_eq!(options.settings["host_setting"], true);
    assert!(options.settings["env"]
        .get("DEVORA_CCC_PERMISSION_MODE")
        .is_none());
    assert_eq!(options.permission_mode, "auto");
}

#[test]
fn a_chosen_model_and_effort_outlive_profile_changes_and_no_effort_leaves_it_to_claude_code() {
    let fixture = Fixture::new();
    let choose = |name: &str, effort: Option<&str>| {
        let mut form = fixture.form(name);
        form.model = Some(ModelChoice {
            model: "sonnet".into(),
            effort: effort.map(Into::into),
        });
        fixture.create(form).unwrap();
    };
    choose("Delta", Some("high"));
    choose("Echo", None);
    std::fs::write(
        fixture.profile.join("config.json"),
        r#"{"claude":{"default-model":"haiku","effort":"low"}}"#,
    )
    .unwrap();
    let options = |name: &str| {
        Space::open(&fixture.profile, name)
            .unwrap()
            .session_options(&fixture.context())
            .unwrap()
    };
    let delta = options("Delta");
    assert_eq!(
        (delta.model.as_str(), delta.effort.as_deref()),
        ("sonnet", Some("high"))
    );
    assert_eq!(options("Echo").effort, None);

    let mut form = fixture.form("Foxtrot");
    form.model = Some(ModelChoice {
        model: " ".into(),
        effort: Some("extreme".into()),
    });
    assert!(fixture.create(form).is_err());
}

#[test]
fn the_form_starts_with_opus_and_only_an_effort_the_user_set() {
    let fixture = Fixture::new();
    let defaults = || model_defaults(&fixture.global, &fixture.profile).unwrap();
    // The built-in effort default doesn't count as the user's choice.
    assert_eq!(
        defaults(),
        ModelChoice {
            model: "opus".into(),
            effort: None
        }
    );
    std::fs::write(&fixture.global, r#"{"claude":{"effort":"medium"}}"#).unwrap();
    assert_eq!(defaults().effort.as_deref(), Some("medium"));
    std::fs::write(
        fixture.profile.join("config.json"),
        r#"{"claude":{"effort":"max"}}"#,
    )
    .unwrap();
    assert_eq!(defaults().effort.as_deref(), Some("max"));
    // A profile that clears the effort leaves it to Claude Code, over User Defaults.
    std::fs::write(
        fixture.profile.join("config.json"),
        r#"{"claude":{"effort":null}}"#,
    )
    .unwrap();
    assert_eq!(defaults().effort, None);
}

#[test]
fn creation_fields_list_form_only_then_prefilled_then_derived_with_their_sources() {
    let fixture = Fixture::new();
    std::fs::write(
        fixture.profile.join("instructions.md"),
        "{{agent.name}} {{agent.role}} {{team}} {{focus}} {{user.details}} {{space.path}} {{ticket}} {{focus}}",
    )
    .unwrap();
    std::fs::write(
        &fixture.global,
        json!({"agent":{"user-details":"Tal"}}).to_string(),
    )
    .unwrap();
    std::fs::write(
        fixture.profile.join("config.json"),
        json!({"agent":{"template-file":"instructions.md","inputs":{"team":"core"}}}).to_string(),
    )
    .unwrap();
    let fields = |form: &CreationForm| {
        devora_agent_space::fields(
            &fixture.profile,
            &fixture.global,
            BTreeMap::from([("ticket".into(), "T-1".into())]),
            form,
        )
        .unwrap()
        .into_iter()
        .map(|f| (f.key, f.value, f.source))
        .collect::<Vec<_>>()
    };
    use devora_agent_space::Source::*;
    let some = |s: &str| Some(s.to_string());
    assert_eq!(
        fields(&fixture.form("")),
        vec![
            ("focus".into(), None, None),
            (
                "agent.role".into(),
                some("a general-purpose assistant"),
                Some(Default)
            ),
            ("team".into(), some("core"), Some(Profile)),
            ("user.details".into(), some("Tal"), Some(Global)),
            ("ticket".into(), some("T-1"), Some(Launch)),
            ("agent.name".into(), None, Some(Derived)),
            ("space.path".into(), None, Some(Derived)),
        ]
    );
    let named = fields(&fixture.form("Echo"));
    let space = std::fs::canonicalize(&fixture.profile)
        .unwrap()
        .join("agent-spaces/Echo");
    assert_eq!(named[5], ("agent.name".into(), some("Echo"), Some(Derived)));
    assert_eq!(
        named[6],
        (
            "space.path".into(),
            some(&space.to_string_lossy()),
            Some(Derived)
        )
    );
}

#[tokio::test]
async fn an_intention_write_failure_prevents_transport_and_uncommitted_publication() {
    let fixture = Fixture::new();
    let context = fixture.context();
    let space = fixture.create(fixture.form("Durable")).unwrap();
    let path = space.path().join(".agent-space.db");
    let log = Arc::new(Log::open(&fixture.root.path().join("failure.log")).unwrap());
    let mut running = space.start(&context, log, &fixture.run).await.unwrap();
    assert!(matches!(
        running.next_event().await,
        Some(SessionEvent::Ready { .. })
    ));
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection.execute_batch("CREATE TRIGGER fail_user BEFORE INSERT ON events WHEN NEW.kind='user' BEGIN SELECT RAISE(ABORT,'injected intention failure'); END;").unwrap();
    let error = running
        .handle()
        .send_turn("must never reach CLI".into())
        .await
        .unwrap_err();
    assert!(error.contains("injected intention failure"), "{error}");
    while let Some(event) = running.next_event().await {
        if matches!(event, SessionEvent::Failed(_)) {
            break;
        }
    }
    let _ = running.pause().await;
    let history = Space::read_history(&fixture.profile, "Durable", 0, 500).unwrap();
    assert!(history.iter().any(|item| matches!(&item.content,
        devora_agent::transcript::TranscriptContent::System { kind:devora_agent::transcript::SystemKind::Failure, text, .. }
            if text.contains("injected intention failure")
    )));
    let report = std::fs::read_to_string(fixture.root.path().join("report.jsonl")).unwrap();
    assert!(!report
        .lines()
        .any(|line| serde_json::from_str::<serde_json::Value>(line).unwrap()["kind"] == "user"));
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM events WHERE kind='user'", [], |row| {
                row.get::<_, u64>(0)
            })
            .unwrap(),
        0
    );
}

fn legacy_space(fixture: &Fixture, name: &str) -> (PathBuf, String) {
    let path = fixture.profile.join("agent-spaces").join(name);
    std::fs::create_dir_all(path.join("memory")).unwrap();
    let database = path.join(".agent-space.db");
    let connection = rusqlite::Connection::open(&database).unwrap();
    connection
        .execute_batch(include_str!("fixtures/space-v1.sql"))
        .unwrap();
    let id = uuid::Uuid::new_v4().to_string();
    connection
        .execute(
            "INSERT INTO space VALUES(0,?1,'Legacy role',?2,'Frozen legacy instructions',?3,0,1)",
            rusqlite::params![
                name,
                serde_json::to_string(&fixture.form(name)).unwrap(),
                id
            ],
        )
        .unwrap();
    (database, id)
}

#[tokio::test]
async fn a_real_v1_shape_lists_without_migration_then_upgrades_and_reopens_with_history() {
    let fixture = Fixture::new();
    let (database, previous_id) = legacy_space(&fixture, "Legacy");
    let connection = rusqlite::Connection::open(&database).unwrap();
    let version = || {
        connection
            .pragma_query_value::<u32, _>(None, "user_version", |row| row.get(0))
            .unwrap()
    };
    let listing = Space::list(&fixture.profile).unwrap();
    assert_eq!(listing.len(), 1);
    assert!(listing[0].error.is_none());
    assert!(Space::read_history(&fixture.profile, "Legacy", 0, 50)
        .unwrap()
        .is_empty());
    assert_eq!(version(), 1);
    let space = Space::open(&fixture.profile, "Legacy").unwrap();
    assert_eq!(version(), 6);
    assert_eq!(space.instructions(), "Frozen legacy instructions");
    assert_eq!(space.identity().role, "Legacy role");
    assert_eq!(space.saved_session().session_id, previous_id);
    assert_eq!(
        connection
            .query_row(
                "SELECT session_id FROM cli_sessions ORDER BY position LIMIT 1",
                [],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
        previous_id
    );
    let log = Arc::new(Log::open(&fixture.root.path().join("legacy.log")).unwrap());
    let mut running = space
        .start(&fixture.context(), log, &fixture.run)
        .await
        .unwrap();
    assert!(matches!(
        running.next_event().await,
        Some(SessionEvent::Ready { .. })
    ));
    running
        .handle()
        .send_turn("Legacy prompt".into())
        .await
        .unwrap();
    while let Some(event) = running.next_event().await {
        if matches!(
            event,
            SessionEvent::Message(Message::Known(KnownMessage::Result(_)))
        ) {
            break;
        }
    }
    running.pause().await.unwrap();
    let reopened = Space::open(&fixture.profile, "Legacy").unwrap();
    assert_eq!(version(), 6);
    assert!(reopened.history().unwrap().iter().any(|item| matches!(&item.content, devora_agent::transcript::TranscriptContent::User { text, .. } if text == "Legacy prompt")));
    assert_eq!(reopened.instructions(), "Frozen legacy instructions");
}

#[test]
fn a_failed_v1_migration_rolls_back_its_tables_and_version() {
    let fixture = Fixture::new();
    let (database, id) = legacy_space(&fixture, "Blocked");
    let connection = rusqlite::Connection::open(&database).unwrap();
    connection
        .execute_batch(
            "CREATE TABLE events (keep_me TEXT); INSERT INTO events VALUES('preserved');",
        )
        .unwrap();
    assert!(Space::open(&fixture.profile, "Blocked").is_err());
    assert_eq!(
        connection
            .pragma_query_value::<u32, _>(None, "user_version", |row| row.get(0))
            .unwrap(),
        1
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE name='cli_sessions'",
                [],
                |row| row.get::<_, u64>(0)
            )
            .unwrap(),
        0
    );
    assert_eq!(
        connection
            .query_row("SELECT keep_me FROM events", [], |row| row
                .get::<_, String>(0))
            .unwrap(),
        "preserved"
    );
    assert_eq!(
        connection
            .query_row("SELECT session_id FROM space", [], |row| row
                .get::<_, String>(0))
            .unwrap(),
        id
    );
}

#[tokio::test]
async fn large_history_reopens_with_only_the_tail_and_pages_back_in_order() {
    use devora_agent::transcript::{ItemState, TranscriptContent, TranscriptItem};
    use devora_agent_space::SpaceEvent;
    let fixture = Fixture::new();
    let space = fixture.create(fixture.form("History")).unwrap();
    let session = space.saved_session().session_id.clone();
    let mut db = rusqlite::Connection::open(space.path().join(".agent-space.db")).unwrap();
    let tx = db.transaction().unwrap();
    tx.execute(
        "INSERT INTO cli_sessions(session_id,position) VALUES(?1,1)",
        [&session],
    )
    .unwrap();
    for sequence in 1..=650 {
        let item = TranscriptItem {
            sequence,
            cli_session: session.clone(),
            parent_tool_use_id: None,
            state: ItemState::Complete,
            content: TranscriptContent::Assistant {
                text: format!("item {sequence}"),
                model: None,
            },
            created_at_ms: None,
        };
        tx.execute(
            "INSERT INTO events(sequence,cli_session,kind,payload) VALUES(?1,?2,'assistant',?3)",
            rusqlite::params![sequence, session, serde_json::to_string(&item).unwrap()],
        )
        .unwrap();
    }
    tx.commit().unwrap();
    drop(db);
    drop(space);
    let reopened = Space::open(&fixture.profile, "History").unwrap();
    let tail = reopened.history().unwrap();
    assert_eq!(tail.len(), 200);
    assert_eq!(tail.first().unwrap().sequence, 451);
    assert_eq!(tail.last().unwrap().sequence, 650);
    let mut all = tail.clone();
    loop {
        let page =
            Space::history_before(&fixture.profile, "History", Some(all[0].sequence), 137).unwrap();
        if page.is_empty() {
            break;
        }
        all.splice(..0, page);
    }
    assert_eq!(
        all.iter().map(|item| item.sequence).collect::<Vec<_>>(),
        (1..=650).collect::<Vec<_>>()
    );
    assert_eq!(
        Space::history_before(&fixture.profile, "History", None, 0)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        Space::history_before(&fixture.profile, "History", None, u32::MAX)
            .unwrap()
            .len(),
        500
    );
    let log = Arc::new(Log::open(&fixture.root.path().join("history.log")).unwrap());
    let mut running = reopened
        .start(&fixture.context(), log, &fixture.run)
        .await
        .unwrap();
    let Some(SpaceEvent::Restored(restored)) = running.next_update().await else {
        panic!("expected restored tail")
    };
    assert_eq!(restored, tail);
    while let Some(update) = running.next_update().await {
        if let SpaceEvent::Transcript(devora_agent::transcript::TranscriptChange::Upsert { item }) =
            update
        {
            assert_eq!(item.sequence, 651, "sequence must include evicted history");
            break;
        }
    }
    running.pause().await.unwrap();
}

// The time a UI shows on a row is the one its item was first stored with: the same on every update of the item, live
// and after a relaunch.
#[tokio::test]
async fn each_item_reaches_the_ui_with_the_time_it_was_first_stored() {
    let fixture = Fixture::new();
    let context = fixture.context();
    let space = fixture.create(fixture.form("Delta")).unwrap();
    let log = Arc::new(Log::open(&fixture.root.path().join("log")).unwrap());
    let mut running = space
        .start(&context, log.clone(), &fixture.run)
        .await
        .unwrap();
    running.handle().send_turn("a turn".into()).await.unwrap();
    let mut live: std::collections::BTreeMap<u64, Vec<Option<u64>>> = Default::default();
    tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(update) = running.next_update().await {
            match update {
                SpaceEvent::Transcript(devora_agent::transcript::TranscriptChange::Upsert {
                    item,
                }) => live
                    .entry(item.sequence)
                    .or_default()
                    .push(item.created_at_ms),
                SpaceEvent::TurnEnded => break,
                _ => {}
            }
        }
    })
    .await
    .expect("the turn ends");
    running.pause().await.unwrap();
    assert!(live.len() > 1, "the turn's prompt and its reply at least");
    let stored = Space::read_history(&fixture.profile, "Delta", 0, 500).unwrap();
    for item in &stored {
        assert!(
            item.created_at_ms.is_some(),
            "item {} has a time",
            item.sequence
        );
        // The pause's exit note is stored after the turn, which is all this reads live.
        let times = live.get(&item.sequence).map_or(&[][..], Vec::as_slice);
        assert!(
            times.iter().all(|time| *time == item.created_at_ms),
            "item {} reached the UI with {times:?}, stored with {:?}",
            item.sequence,
            item.created_at_ms
        );
    }
    let mut running = Space::open(&fixture.profile, "Delta")
        .unwrap()
        .start(&context, log, &fixture.run)
        .await
        .unwrap();
    let Some(SpaceEvent::Restored(restored)) = running.next_update().await else {
        panic!("expected the restored tail")
    };
    assert_eq!(restored, stored);
    running.pause().await.unwrap();
}

#[tokio::test]
async fn over_the_threshold_the_agent_saves_its_memory_then_devora_compacts() {
    let fixture = Fixture::new();
    // The recorded turn fills about 2% of its model's 1,000,000-token window.
    std::fs::write(
        fixture.profile.join("config.json"),
        json!({"prepare-command":"true","agent":{"compact-threshold":1}}).to_string(),
    )
    .unwrap();
    let context = fixture.context();
    let space = fixture.create(fixture.form("Delta")).unwrap();
    let log = Arc::new(Log::open(&fixture.root.path().join("log")).unwrap());
    let mut running = space.start(&context, log, &fixture.run).await.unwrap();
    running
        .handle()
        .send_turn("first turn".into())
        .await
        .unwrap();
    let mut contexts = vec![];
    let mut announcements = vec![];
    let mut results = 0;
    tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(update) = running.next_update().await {
            match update {
                SpaceEvent::Context(context) => contexts.push(context),
                SpaceEvent::Transcript(TranscriptChange::Upsert { item }) => {
                    if let TranscriptContent::System {
                        kind: SystemKind::Compaction,
                        text,
                        ..
                    } = item.content
                    {
                        announcements.push(text);
                    }
                }
                SpaceEvent::Session(SessionEvent::Message(Message::Known(
                    KnownMessage::Result(_),
                ))) => {
                    results += 1;
                    // The user's turn, the memory save, then `/compact`.
                    if results == 3 {
                        break;
                    }
                }
                SpaceEvent::Session(SessionEvent::Failed(error)) => panic!("{error}"),
                _ => {}
            }
        }
    })
    .await
    .unwrap();
    running.pause().await.unwrap();

    let report = std::fs::read_to_string(fixture.root.path().join("report.jsonl")).unwrap();
    let prompts: Vec<String> = report
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .filter(|entry| entry["kind"] == "user")
        .map(|entry| entry["message"]["content"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(prompts.len(), 3, "{prompts:?}");
    assert_eq!(prompts[0], "first turn");
    assert!(prompts[1].contains("save to memory/"), "{}", prompts[1]);
    assert_eq!(prompts[2], "/compact");
    // Devora's two steps, then the boundary the CLI reports.
    assert_eq!(announcements.len(), 3, "{announcements:?}");
    assert!(announcements[0].contains("save its memory"));
    assert!(announcements[1].contains("Devora compacts"));

    assert_eq!(contexts[0].threshold_percent, 1);
    assert!(contexts
        .iter()
        .any(|c| c.window_tokens == 1_000_000 && c.used_tokens > 10_000));
    assert_eq!(
        contexts.last().unwrap().used_tokens,
        2000,
        "the compaction's post_tokens"
    );
}

/// Sends `prompt` to a new agent with the profile's `threshold`, and returns the contexts the space published and
/// the prompts the CLI received by the time `results` turns have ended.
async fn contexts_and_prompts(
    threshold: u8,
    prompt: &str,
    results: usize,
) -> (Vec<devora_agent_space::Context>, Vec<String>) {
    let fixture = Fixture::new();
    std::fs::write(
        fixture.profile.join("config.json"),
        json!({"prepare-command":"true","agent":{"compact-threshold":threshold}}).to_string(),
    )
    .unwrap();
    let context = fixture.context();
    let space = fixture.create(fixture.form("Delta")).unwrap();
    let log = Arc::new(Log::open(&fixture.root.path().join("log")).unwrap());
    let mut running = space.start(&context, log, &fixture.run).await.unwrap();
    running.handle().send_turn(prompt.into()).await.unwrap();
    let mut contexts = vec![];
    let mut ended = 0;
    tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(update) = running.next_update().await {
            match update {
                SpaceEvent::Context(context) => contexts.push(context),
                SpaceEvent::Session(SessionEvent::Message(Message::Known(
                    KnownMessage::Result(_),
                ))) => {
                    ended += 1;
                    if ended == results {
                        break;
                    }
                }
                SpaceEvent::Session(SessionEvent::Failed(error)) => panic!("{error}"),
                _ => {}
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("{ended} of {results} turns ended; contexts: {contexts:?}"));
    running.pause().await.unwrap();
    let report = std::fs::read_to_string(fixture.root.path().join("report.jsonl")).unwrap();
    let prompts = report
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .filter(|entry| entry["kind"] == "user")
        .map(|entry| entry["message"]["content"].as_str().unwrap().to_owned())
        .collect();
    (contexts, prompts)
}

// The fake's subagent message reports a context of 4 tokens; the agent's own is the recorded turn's, over 10,000.
#[tokio::test]
async fn a_subagents_message_mid_turn_leaves_the_agents_context_alone() {
    let (contexts, _) = contexts_and_prompts(80, "subagent:mid", 1).await;
    let used: Vec<u64> = contexts.iter().map(|c| c.used_tokens).collect();
    assert!(used.iter().any(|&tokens| tokens > 10_000), "{used:?}");
    assert!(
        used.iter().all(|&tokens| tokens == 0 || tokens > 10_000),
        "the subagent's context never shows as the agent's: {used:?}"
    );
}

#[tokio::test]
async fn the_threshold_decides_on_the_agents_context_when_a_subagents_message_comes_last() {
    // The recorded turn fills about 2% of its model's window: over 1%, while the subagent's 4 tokens are not.
    let (_, prompts) = contexts_and_prompts(1, "subagent:last", 2).await;
    assert_eq!(prompts[0], "subagent:last");
    assert!(
        prompts[1].contains("save to memory/"),
        "the compaction starts: {prompts:?}"
    );
}

#[tokio::test]
async fn pause_after_turn_waits_for_the_turn_and_wins_over_compaction() {
    let fixture = Fixture::new();
    std::fs::write(
        fixture.profile.join("config.json"),
        json!({"prepare-command":"true","agent":{"compact-threshold":1}}).to_string(),
    )
    .unwrap();
    let context = fixture.context();
    let space = fixture.create(fixture.form("Delta")).unwrap();
    let log = Arc::new(Log::open(&fixture.root.path().join("log")).unwrap());
    let mut running = space
        .start(&context, log.clone(), &fixture.run)
        .await
        .unwrap();
    let handle = running.handle();
    // The fake CLI holds this turn on a permission question until it is answered.
    handle.send_turn("permission".into()).await.unwrap();
    let request = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let Some(SpaceEvent::Session(SessionEvent::PermissionRequested {
                request_id, ..
            })) = running.next_update().await
            {
                return request_id;
            }
        }
    })
    .await
    .unwrap();
    let pausing = tokio::spawn(handle.pause_after_turn());
    assert_eq!(
        handle
            .send_turn("must stay unsent".into())
            .await
            .unwrap_err(),
        "This agent is pausing. It starts no new turn until it resumes."
    );
    handle
        .answer_permission(request, devora_agent::session::PermissionAnswer::Allow)
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(10), pausing)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let report = std::fs::read_to_string(fixture.root.path().join("report.jsonl")).unwrap();
    let prompts = report
        .lines()
        .filter(|line| line.contains("\"kind\": \"user\"") || line.contains("\"kind\":\"user\""))
        .count();
    assert_eq!(
        prompts, 1,
        "the turn's result is over the threshold, but the pause wins: {report}"
    );
    // Already paused: it reports that stop and waits for the space to release its ownership.
    assert!(running.pause().await.unwrap().graceful);

    // An idle agent pauses at once.
    let idle = Space::open(&fixture.profile, "Delta").unwrap();
    let running = idle.start(&context, log, &fixture.run).await.unwrap();
    tokio::time::timeout(Duration::from_secs(10), running.handle().pause_after_turn())
        .await
        .unwrap()
        .unwrap();
}

#[test]
fn the_built_in_template_needs_only_a_name_and_leaves_no_gaps_for_unset_sections() {
    let fixture = Fixture::new();
    let space = fixture.create(fixture.form("Echo")).unwrap();
    let instructions = space.instructions();
    assert!(
        instructions.starts_with(
            "Hi,\n\nYou are Echo and you are working with me as a general-purpose assistant.\n"
        ),
        "{instructions}"
    );
    assert!(!instructions.contains("{{"));
    assert!(
        !instructions.contains("\n\n\n"),
        "unset optional sections leave no runs of blank lines"
    );
    assert!(
        instructions.ends_with("confirm with me first.\n"),
        "{instructions}"
    );
}

#[test]
fn the_preview_is_what_creation_renders_and_coding_instructions_can_be_replaced() {
    let fixture = Fixture::new();
    let mut form = fixture.form("Echo");
    form.keep_coding_instructions = false;
    let preview =
        devora_agent_space::preview(&fixture.profile, &fixture.global, BTreeMap::new(), &form)
            .unwrap();
    let space = fixture.create(form).unwrap();
    assert_eq!(preview, space.instructions());
    assert!(matches!(
        space.session_options(&fixture.context()).unwrap().instructions,
        devora_agent::session::Instructions::Replace(text) if text == preview
    ));

    // A template value only the form can fill makes the preview say why creation would fail.
    std::fs::write(
        fixture.profile.join("template.md"),
        "{{agent.name}} on {{team}}",
    )
    .unwrap();
    std::fs::write(
        fixture.profile.join("config.json"),
        json!({"prepare-command":"true","agent":{"template-file":"template.md"}}).to_string(),
    )
    .unwrap();
    let error = devora_agent_space::preview(
        &fixture.profile,
        &fixture.global,
        BTreeMap::new(),
        &fixture.form(""),
    )
    .unwrap_err();
    assert!(error.contains("team"), "{error}");
}

// A result the CLI produced by itself (background work finishing, a resume)
// leaves the sent turn running: it neither ends the turn in the chat, nor moves
// compaction on, nor lets a pause stop the agent mid-turn.
#[tokio::test]
async fn a_result_nobody_asked_for_leaves_the_sent_turn_running() {
    let fixture = Fixture::new();
    // Any result of ours would start compaction.
    std::fs::write(
        fixture.profile.join("config.json"),
        json!({"prepare-command":"true","agent":{"compact-threshold":1}}).to_string(),
    )
    .unwrap();
    let context = fixture.context();
    let space = fixture.create(fixture.form("Delta")).unwrap();
    let log = Arc::new(Log::open(&fixture.root.path().join("log")).unwrap());
    let mut running = space.start(&context, log, &fixture.run).await.unwrap();
    let handle = running.handle();
    handle.send_turn("unrequested-result".into()).await.unwrap();
    let mut unrequested = false;
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            match running.next_update().await.unwrap() {
                SpaceEvent::Session(SessionEvent::Message(Message::Known(
                    KnownMessage::Result(_),
                ))) => unrequested = true,
                SpaceEvent::Session(SessionEvent::Message(Message::Raw(value)))
                    if value["type"] == "fake_stream_ready" =>
                {
                    break
                }
                SpaceEvent::TurnEnded => panic!("a result nobody asked for ended the turn"),
                SpaceEvent::Transcript(TranscriptChange::Upsert { item })
                    if matches!(
                        item.content,
                        TranscriptContent::System {
                            kind: SystemKind::Compaction,
                            ..
                        }
                    ) =>
                {
                    panic!("a result nobody asked for started compaction")
                }
                _ => {}
            }
        }
    })
    .await
    .unwrap();
    assert!(unrequested);
    assert_eq!(
        handle.send_turn("second".into()).await.unwrap_err(),
        "a turn is already running"
    );
    let pausing = tokio::spawn(handle.pause_after_turn());
    handle.interrupt().await.unwrap();
    tokio::time::timeout(Duration::from_secs(10), async {
        while !matches!(running.next_update().await.unwrap(), SpaceEvent::TurnEnded) {}
    })
    .await
    .expect("the turn's own result ends it");
    tokio::time::timeout(Duration::from_secs(10), pausing)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let report: Vec<serde_json::Value> =
        std::fs::read_to_string(fixture.root.path().join("report.jsonl"))
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
    let kinds: Vec<_> = report
        .iter()
        .map(|entry| entry["kind"].as_str().unwrap())
        .collect();
    assert_eq!(
        kinds.iter().filter(|kind| **kind == "user").count(),
        1,
        "no compaction step was sent: {kinds:?}"
    );
    let interrupt = kinds.iter().position(|kind| *kind == "interrupt").unwrap();
    let eof = kinds.iter().position(|kind| *kind == "eof").unwrap();
    assert!(interrupt < eof, "the pause waited for the turn: {kinds:?}");
}

/// The notices (after a restart, or a stop) the fake CLI received, in order.
fn notices(fixture: &Fixture) -> Vec<serde_json::Value> {
    std::fs::read_to_string(fixture.root.path().join("report.jsonl"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .filter(|entry| {
            entry["kind"] == "user"
                && entry["message"]["content"].as_str().is_some_and(|text| {
                    text.starts_with("Devora restarted your session")
                        || text.starts_with("Devora had to stop your session")
                        || text.starts_with("Devora paused you")
                })
        })
        .collect()
}

/// Notice rows a launch showed, as (text, recovery).
fn notice_row(update: &SpaceEvent) -> Option<(u64, String, bool)> {
    let SpaceEvent::Transcript(TranscriptChange::Upsert { item }) = update else {
        return None;
    };
    let TranscriptContent::System {
        kind: SystemKind::RestartNotice,
        text,
        detail_json,
    } = &item.content
    else {
        return None;
    };
    let detail: serde_json::Value = serde_json::from_str(detail_json).unwrap();
    Some((item.sequence, text.clone(), detail["recovery"] == true))
}

/// Waits for the running turn's end, collecting the notice rows shown on the way.
async fn turn_end(
    running: &mut RunningSpace,
    rows: &mut std::collections::BTreeMap<u64, (String, bool)>,
) {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let update = running.next_update().await.expect("the space stopped");
            if let Some((sequence, text, recovery)) = notice_row(&update) {
                rows.insert(sequence, (text, recovery));
            }
            match update {
                SpaceEvent::TurnEnded => return,
                SpaceEvent::Session(SessionEvent::Failed(error)) => panic!("{error}"),
                _ => {}
            }
        }
    })
    .await
    .expect("turn end deadline")
}

/// Lets a launch's notice, if it owes one, run to its end, then runs one turn of
/// the user's, which a running notice would refuse. Returns the notice rows shown.
async fn settle(running: &mut RunningSpace) -> Vec<(String, bool)> {
    let mut rows = std::collections::BTreeMap::new();
    loop {
        match running.handle().send_turn("probe".into()).await {
            Ok(()) => break,
            Err(error) if error == "a turn is already running" => {
                turn_end(running, &mut rows).await
            }
            Err(error) => panic!("{error}"),
        }
    }
    turn_end(running, &mut rows).await;
    rows.into_values().collect()
}

/// A launch, in `run`, that holds itself at once, as a global hold does.
async fn launch_held(fixture: &Fixture, context: &LaunchContext, log: &Arc<Log>, run: &AppRun) {
    let mut running = Space::open(&fixture.profile, "Delta")
        .unwrap()
        .start_with_global_pause(context, log.clone(), Arc::new(|| true), run)
        .await
        .unwrap();
    while running.next_update().await.is_some() {}
    let _ = running.pause().await;
}

async fn launch_and_settle(
    fixture: &Fixture,
    context: &LaunchContext,
    log: &Arc<Log>,
    run: &AppRun,
) -> Vec<(String, bool)> {
    let mut running = Space::open(&fixture.profile, "Delta")
        .unwrap()
        .start(context, log.clone(), run)
        .await
        .unwrap();
    let rows = settle(&mut running).await;
    running.pause().await.unwrap();
    rows
}

#[tokio::test]
async fn a_restart_owes_an_agent_one_notice_and_a_relaunch_in_the_same_run_none() {
    let fixture = Fixture::new();
    let context = fixture.context();
    let log = Arc::new(Log::open(&fixture.root.path().join("log")).unwrap());
    fixture.create(fixture.form("Delta")).unwrap();
    assert!(
        launch_and_settle(&fixture, &context, &log, &fixture.run)
            .await
            .is_empty(),
        "without a conversation there is nothing to re-establish"
    );
    assert!(launch_and_settle(&fixture, &context, &log, &fixture.run)
        .await
        .is_empty());

    let restarted = AppRun::new();
    let rows = launch_and_settle(&fixture, &context, &log, &restarted).await;
    assert_eq!(
        rows,
        vec![(
            "Devora restarted: asked the agent to re-check its waits".into(),
            false
        )]
    );
    assert!(
        launch_and_settle(&fixture, &context, &log, &restarted)
            .await
            .is_empty(),
        "a relaunch in the same run owes nothing"
    );
    assert_eq!(
        launch_and_settle(&fixture, &context, &log, &AppRun::new())
            .await
            .len(),
        1,
        "a delivered notice doesn't cover a later restart"
    );

    let sent = notices(&fixture);
    assert_eq!(sent.len(), 2);
    assert_ne!(sent[0]["uuid"], sent[1]["uuid"]);
    let prompt = sent[0]["message"]["content"].as_str().unwrap();
    assert!(prompt.contains("re-establish whatever is gone"), "{prompt}");
    assert!(!prompt.contains("may repeat"), "{prompt}");
    // The notice is history too: a reopened chat shows it.
    let history = Space::open(&fixture.profile, "Delta")
        .unwrap()
        .history()
        .unwrap();
    assert_eq!(
        history
            .iter()
            .filter(|item| matches!(
                item.content,
                TranscriptContent::System {
                    kind: SystemKind::RestartNotice,
                    ..
                }
            ))
            .count(),
        2
    );
}

// A hold at launch (or a crash before the notice's attempt commits) leaves it
// owed, and a second restart before it goes out still owes only one.
#[tokio::test]
async fn a_notice_waits_out_holds_and_two_restarts_before_it_goes_out_owe_one() {
    let fixture = Fixture::new();
    let context = fixture.context();
    let log = Arc::new(Log::open(&fixture.root.path().join("log")).unwrap());
    fixture.create(fixture.form("Delta")).unwrap();
    launch_and_settle(&fixture, &context, &log, &fixture.run).await;
    launch_held(&fixture, &context, &log, &AppRun::new()).await;
    let latest = AppRun::new();
    launch_held(&fixture, &context, &log, &latest).await;
    assert!(
        notices(&fixture).is_empty(),
        "a held agent gets nothing yet"
    );
    let rows = launch_and_settle(&fixture, &context, &log, &latest).await;
    assert_eq!(rows.len(), 1);
    assert!(!rows[0].1, "nothing was sent before, so it's no recovery");
    assert_eq!(notices(&fixture).len(), 1);
}

// The fake CLI dies on the notice, as a crash mid-delivery would, with Claude's
// saved session holding it or not. The next launch, in the same run, decides by
// that session alone. Missing from it is also what a crash between committing
// the attempt and sending it leaves, after which the agent never got the first:
// that one is marked as resent too, which its "may repeat" wording allows.
#[tokio::test]
async fn a_notice_cut_short_is_resent_as_a_recovery_unless_claudes_session_saved_it() {
    for saved in [false, true] {
        let fixture = Fixture::new();
        let mut context = fixture.context();
        context.environment.insert(
            "DEVORA_FAKE_NOTICE_CRASH".into(),
            fixture.root.path().join("crashed").display().to_string(),
        );
        if saved {
            context
                .environment
                .insert("DEVORA_FAKE_NOTICE_SAVED".into(), "1".into());
        }
        let log = Arc::new(Log::open(&fixture.root.path().join("log")).unwrap());
        fixture.create(fixture.form("Delta")).unwrap();
        launch_and_settle(&fixture, &context, &log, &fixture.run).await;

        let restarted = AppRun::new();
        let mut running = Space::open(&fixture.profile, "Delta")
            .unwrap()
            .start(&context, log.clone(), &restarted)
            .await
            .unwrap();
        let mut crashed = false;
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                match running.next_update().await.unwrap() {
                    SpaceEvent::Session(SessionEvent::Exited { .. }) => crashed = true,
                    SpaceEvent::Session(SessionEvent::Ready { .. }) if crashed => break,
                    _ => {}
                }
            }
        })
        .await
        .expect("the CLI dies on the notice and comes back");
        running.pause().await.unwrap();

        let rows = launch_and_settle(&fixture, &context, &log, &restarted).await;
        let sent = notices(&fixture);
        if saved {
            assert!(rows.is_empty(), "the saved session shows it was delivered");
            assert_eq!(sent.len(), 1);
        } else {
            assert_eq!(
                rows,
                vec![(
                    "Devora restarted: asked the agent to re-check its waits (resent)".into(),
                    true
                )]
            );
            assert_eq!(sent.len(), 2);
            assert_ne!(sent[0]["uuid"], sent[1]["uuid"]);
            assert!(sent[1]["message"]["content"]
                .as_str()
                .unwrap()
                .contains("may repeat"));
        }
    }
}

// The zero-turn result a resume brings can arrive after the notice is sent.
// The notice's turn runs on until its own result.
#[tokio::test]
async fn a_result_nobody_asked_for_doesnt_end_the_notices_turn() {
    let fixture = Fixture::new();
    let mut context = fixture.context();
    let log = Arc::new(Log::open(&fixture.root.path().join("log")).unwrap());
    fixture.create(fixture.form("Delta")).unwrap();
    launch_and_settle(&fixture, &context, &log, &fixture.run).await;
    context
        .environment
        .insert("DEVORA_FAKE_NOTICE_UNREQUESTED".into(), "1".into());
    let mut running = Space::open(&fixture.profile, "Delta")
        .unwrap()
        .start(&context, log.clone(), &AppRun::new())
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            match running.next_update().await.unwrap() {
                SpaceEvent::Session(SessionEvent::Message(Message::Raw(value)))
                    if value["type"] == "fake_stream_ready" =>
                {
                    break
                }
                SpaceEvent::TurnEnded => {
                    panic!("a result nobody asked for ended the notice's turn")
                }
                _ => {}
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(
        running
            .handle()
            .send_turn("too early".into())
            .await
            .unwrap_err(),
        "a turn is already running"
    );
    running.handle().interrupt().await.unwrap();
    tokio::time::timeout(Duration::from_secs(10), async {
        while !matches!(running.next_update().await.unwrap(), SpaceEvent::TurnEnded) {}
    })
    .await
    .expect("the notice's own result ends it");
    running.pause().await.unwrap();
}

// A space from before restart notices has no record of its last launch: its
// first launch after the upgrade, itself a restart, owes one.
#[tokio::test]
async fn a_space_from_before_restart_notices_is_owed_one_on_its_first_launch_after_the_upgrade() {
    let fixture = Fixture::new();
    let context = fixture.context();
    let log = Arc::new(Log::open(&fixture.root.path().join("log")).unwrap());
    fixture.create(fixture.form("Delta")).unwrap();
    launch_and_settle(&fixture, &context, &log, &fixture.run).await;
    let database = fixture.profile.join("agent-spaces/Delta/.agent-space.db");
    let connection = rusqlite::Connection::open(&database).unwrap();
    connection
        .execute_batch(
            "ALTER TABLE events DROP COLUMN created_at; ALTER TABLE space DROP COLUMN compact_threshold; ALTER TABLE space DROP COLUMN cli_process; ALTER TABLE space DROP COLUMN launched_run; ALTER TABLE space DROP COLUMN restart_notice; PRAGMA user_version = 2;",
        )
        .unwrap();
    assert_eq!(
        launch_and_settle(&fixture, &context, &log, &fixture.run)
            .await
            .len(),
        1
    );
    assert_eq!(
        connection
            .pragma_query_value::<u32, _>(None, "user_version", |row| row.get(0))
            .unwrap(),
        6
    );
}

/// A stand-in for a CLI an earlier Devora left running: a parent whose child
/// made itself a session leader, as Claude Code's background shells do.
/// Returns the parent and the child's pid.
fn left_running() -> (std::process::Child, u32) {
    use std::io::BufRead;
    let mut parent = Command::new("python3")
        .args([
            "-c",
            "import os, time\n\
             pid = os.fork()\n\
             if pid == 0:\n    os.setsid(); time.sleep(60); os._exit(0)\n\
             print(pid, flush=True); time.sleep(60)",
        ])
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut line = String::new();
    std::io::BufReader::new(parent.stdout.as_mut().unwrap())
        .read_line(&mut line)
        .unwrap();
    (parent, line.trim().parse().unwrap())
}

fn record_cli(fixture: &Fixture, cli: Option<&CliProcess>) {
    rusqlite::Connection::open(fixture.profile.join("agent-spaces/Delta/.agent-space.db"))
        .unwrap()
        .execute(
            "UPDATE space SET cli_process=?1",
            [cli.map(|cli| serde_json::to_string(cli).unwrap())],
        )
        .unwrap();
}

fn recorded_cli(fixture: &Fixture) -> Option<CliProcess> {
    rusqlite::Connection::open(fixture.profile.join("agent-spaces/Delta/.agent-space.db"))
        .unwrap()
        .query_row("SELECT cli_process FROM space", [], |row| {
            row.get::<_, Option<String>>(0)
        })
        .unwrap()
        .map(|cli| serde_json::from_str(&cli).unwrap())
}

#[tokio::test]
async fn a_launch_stops_a_cli_an_earlier_devora_left_running_before_resuming() {
    let fixture = Fixture::new();
    let context = fixture.context();
    let log = Arc::new(Log::open(&fixture.root.path().join("log")).unwrap());
    fixture.create(fixture.form("Delta")).unwrap();
    launch_and_settle(&fixture, &context, &log, &fixture.run).await;

    let table = SystemProcessTable;
    let (mut orphan, shell) = left_running();
    let cli = CliProcess::of(&table, orphan.id()).unwrap();
    record_cli(&fixture, Some(&cli));
    let mut running = Space::open(&fixture.profile, "Delta")
        .unwrap()
        .start(&context, log.clone(), &fixture.run)
        .await
        .unwrap();
    // Stopped before the new CLI started, so there is never a second writer.
    assert_eq!(table.get(orphan.id()), None);
    assert_eq!(table.get(shell), None, "its session-leading shell too");
    orphan.wait().unwrap();
    settle(&mut running).await;
    running.pause().await.unwrap();
    assert!(std::fs::read_to_string(fixture.root.path().join("log"))
        .unwrap()
        .contains(&format!(
            "stopped Claude Code (pid {}) that an earlier Devora left running",
            cli.pid
        )));
}

#[tokio::test]
async fn a_recorded_pid_that_another_process_now_has_is_left_alone() {
    let fixture = Fixture::new();
    let context = fixture.context();
    let log = Arc::new(Log::open(&fixture.root.path().join("log")).unwrap());
    fixture.create(fixture.form("Delta")).unwrap();
    launch_and_settle(&fixture, &context, &log, &fixture.run).await;

    let table = SystemProcessTable;
    let (mut other, shell) = left_running();
    let live = CliProcess::of(&table, other.id()).unwrap();
    record_cli(
        &fixture,
        Some(&CliProcess {
            started: live.started + 1,
            ..live
        }),
    );
    launch_and_settle(&fixture, &context, &log, &fixture.run).await;
    assert_eq!(table.get(other.id()).map(CliProcess::from), Some(live));
    assert!(table.get(shell).is_some());
    stop_tree(&table, live, Duration::from_secs(5)).unwrap();
    other.wait().unwrap();
}

#[tokio::test]
async fn the_running_cli_is_recorded_and_cleared_once_it_stops_under_devoras_watch() {
    let fixture = Fixture::new();
    let context = fixture.context();
    let log = Arc::new(Log::open(&fixture.root.path().join("log")).unwrap());
    fixture.create(fixture.form("Delta")).unwrap();
    let mut running = Space::open(&fixture.profile, "Delta")
        .unwrap()
        .start(&context, log.clone(), &fixture.run)
        .await
        .unwrap();
    settle(&mut running).await;
    let started = std::fs::read_to_string(fixture.root.path().join("report.jsonl"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .find(|report| report["kind"] == "start")
        .unwrap();
    assert_eq!(
        recorded_cli(&fixture).map(|cli| serde_json::json!(cli.pid)),
        Some(started["pid"].clone())
    );
    running.pause().await.unwrap();
    assert_eq!(recorded_cli(&fixture), None);
}

/// Starts the agent's launch in `run`, sends `prompt`, waits until the fake CLI
/// holds that turn open, and pauses: the stop interrupts the turn.
async fn stop_mid_turn(fixture: &Fixture, context: &LaunchContext, log: &Arc<Log>, prompt: &str) {
    let mut running = Space::open(&fixture.profile, "Delta")
        .unwrap()
        .start(context, log.clone(), &fixture.run)
        .await
        .unwrap();
    running.handle().send_turn(prompt.into()).await.unwrap();
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let SpaceEvent::Session(SessionEvent::Message(Message::Raw(value))) =
                running.next_update().await.unwrap()
            {
                if value["type"] == "fake_stream_ready" {
                    return;
                }
            }
        }
    })
    .await
    .unwrap();
    running.pause().await.unwrap();
}

// The host pausing mid-turn (the tab closing, a quit, a hold) interrupts the turn, which the CLI ends with an error
// result; the space asked for that stop, so the transcript keeps it as one: "Turn stopped", not a failure.
#[tokio::test]
async fn a_turn_the_host_paused_mid_way_ends_stopped_not_failed() {
    let fixture = Fixture::new();
    let context = fixture.context();
    let log = Arc::new(Log::open(&fixture.root.path().join("log")).unwrap());
    fixture.create(fixture.form("Delta")).unwrap();
    stop_mid_turn(&fixture, &context, &log, "stream").await;
    let results: Vec<(String, serde_json::Value)> =
        Space::read_history(&fixture.profile, "Delta", 0, 500)
            .unwrap()
            .into_iter()
            .filter_map(|item| match item.content {
                devora_agent::transcript::TranscriptContent::System {
                    kind: devora_agent::transcript::SystemKind::Result,
                    text,
                    detail_json,
                } => Some((text, serde_json::from_str(&detail_json).unwrap())),
                _ => None,
            })
            .collect();
    assert_eq!(results.len(), 1, "{results:?}");
    assert_eq!(results[0].0, "Turn stopped");
    assert_eq!(results[0].1["stopped"], true);
}

// In one run: a clean idle stop owes nothing, while a stop that interrupted a
// turn, or had to be forced, ended the agent's background work and owes it the
// notice at its next launch.
#[tokio::test]
async fn a_stop_that_interrupted_a_turn_owes_the_notice_and_a_clean_idle_stop_none() {
    let fixture = Fixture::new();
    let context = fixture.context();
    let log = Arc::new(Log::open(&fixture.root.path().join("log")).unwrap());
    fixture.create(fixture.form("Delta")).unwrap();
    launch_and_settle(&fixture, &context, &log, &fixture.run).await;
    assert!(
        launch_and_settle(&fixture, &context, &log, &fixture.run)
            .await
            .is_empty(),
        "a clean idle stop owes nothing"
    );
    stop_mid_turn(&fixture, &context, &log, "stream").await;
    assert_eq!(
        launch_and_settle(&fixture, &context, &log, &fixture.run).await,
        vec![(
            "Devora stopped the agent to pause it: asked it to re-check its waits".into(),
            false
        )]
    );
    assert!(launch_and_settle(&fixture, &context, &log, &fixture.run)
        .await
        .is_empty());
    let sent = notices(&fixture);
    assert_eq!(sent.len(), 1);
    assert!(sent[0]["message"]["content"]
        .as_str()
        .unwrap()
        .contains("to pause it"));
}

// A forced stop (here the turn ignores the interrupt) stops the CLI's whole
// tree, its session-leading background shell included, and owes the notice.
#[tokio::test]
async fn a_forced_stop_ends_the_cli_with_its_background_shell_and_owes_the_notice() {
    let fixture = Fixture::new();
    let context = fixture.context();
    let log = Arc::new(Log::open(&fixture.root.path().join("log")).unwrap());
    fixture.create(fixture.form("Delta")).unwrap();
    launch_and_settle(&fixture, &context, &log, &fixture.run).await;
    stop_mid_turn(&fixture, &context, &log, "background-shell").await;
    let shell = std::fs::read_to_string(fixture.root.path().join("report.jsonl"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .find(|report| report["kind"] == "background")
        .unwrap()["pid"]
        .as_u64()
        .unwrap() as u32;
    assert_eq!(
        SystemProcessTable.get(shell),
        None,
        "a forced stop leaves no background shell running"
    );
    assert_eq!(
        launch_and_settle(&fixture, &context, &log, &fixture.run)
            .await
            .len(),
        1
    );
}

/// The context the space publishes next.
async fn next_context(running: &mut RunningSpace) -> devora_agent_space::Context {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            match running.next_update().await {
                Some(SpaceEvent::Context(context)) => return context,
                Some(SpaceEvent::Session(SessionEvent::Failed(error))) => panic!("{error}"),
                Some(_) => {}
                None => panic!("the space stopped before publishing its context"),
            }
        }
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn an_agents_own_threshold_outlives_a_restart_and_clearing_it_follows_the_profile_again() {
    let fixture = Fixture::new();
    std::fs::write(
        fixture.profile.join("config.json"),
        json!({"prepare-command":"true","agent":{"compact-threshold":70}}).to_string(),
    )
    .unwrap();
    let context = fixture.context();
    let log = Arc::new(Log::open(&fixture.root.path().join("log")).unwrap());
    fixture.create(fixture.form("Delta")).unwrap();
    let start = || async {
        Space::open(&fixture.profile, "Delta")
            .unwrap()
            .start(&context, log.clone(), &fixture.run)
            .await
            .unwrap()
    };

    let mut running = start().await;
    let first = next_context(&mut running).await;
    assert_eq!(
        (
            first.threshold_percent,
            first.default_threshold_percent,
            first.own_threshold_percent
        ),
        (70, 70, None)
    );
    running
        .handle()
        .set_compact_threshold(Some(70))
        .await
        .unwrap();
    assert_eq!(
        next_context(&mut running).await.own_threshold_percent,
        Some(70),
        "the profile's value, but the agent's own: a later profile change won't reach it"
    );
    running
        .handle()
        .set_compact_threshold(Some(40))
        .await
        .unwrap();
    assert_eq!(next_context(&mut running).await.threshold_percent, 40);
    assert!(running
        .handle()
        .set_compact_threshold(Some(0))
        .await
        .is_err());
    running.pause().await.unwrap();

    let mut running = start().await;
    let restarted = next_context(&mut running).await;
    assert_eq!(
        (
            restarted.threshold_percent,
            restarted.default_threshold_percent,
            restarted.own_threshold_percent
        ),
        (40, 70, Some(40)),
        "its own, beside the profile's"
    );
    running.handle().set_compact_threshold(None).await.unwrap();
    let cleared = next_context(&mut running).await;
    assert_eq!(
        (cleared.threshold_percent, cleared.own_threshold_percent),
        (70, None)
    );
    running.pause().await.unwrap();

    let mut running = start().await;
    assert_eq!(next_context(&mut running).await.threshold_percent, 70);
    running.pause().await.unwrap();
}

#[tokio::test]
async fn compact_now_saves_the_memory_then_compacts_under_the_threshold_once_at_a_time() {
    let fixture = Fixture::new();
    let context = fixture.context();
    let space = fixture.create(fixture.form("Delta")).unwrap();
    let log = Arc::new(Log::open(&fixture.root.path().join("log")).unwrap());
    let mut running = space.start(&context, log, &fixture.run).await.unwrap();
    // The recorded turn fills about 2% of its window, far under the default threshold.
    let mut rows = BTreeMap::new();
    running
        .handle()
        .send_turn("first turn".into())
        .await
        .unwrap();
    turn_end(&mut running, &mut rows).await;

    running.handle().compact_now().await.unwrap();
    assert_eq!(
        running.handle().compact_now().await.unwrap_err(),
        "A compaction is already under way."
    );
    let mut progress = vec![];
    let mut announcements = vec![];
    let mut results = 0;
    tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(update) = running.next_update().await {
            match update {
                SpaceEvent::Context(context) => progress.push(context.progress),
                SpaceEvent::Transcript(TranscriptChange::Upsert { item }) => {
                    if let TranscriptContent::System {
                        kind: SystemKind::Compaction,
                        text,
                        ..
                    } = item.content
                    {
                        announcements.push(text);
                    }
                }
                SpaceEvent::Session(SessionEvent::Message(Message::Known(
                    KnownMessage::Result(_),
                ))) => {
                    results += 1;
                    // The memory save, then `/compact`.
                    if results == 2 {
                        break;
                    }
                }
                SpaceEvent::Session(SessionEvent::Failed(error)) => panic!("{error}"),
                _ => {}
            }
        }
    })
    .await
    .unwrap();
    running.pause().await.unwrap();

    let report = std::fs::read_to_string(fixture.root.path().join("report.jsonl")).unwrap();
    let prompts: Vec<String> = report
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .filter(|entry| entry["kind"] == "user")
        .map(|entry| entry["message"]["content"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(prompts.len(), 3, "{prompts:?}");
    assert!(
        prompts[1].contains("the user asked to compact"),
        "{}",
        prompts[1]
    );
    assert_eq!(prompts[2], "/compact");
    assert!(
        announcements[0].starts_with("Compaction requested at"),
        "{announcements:?}"
    );
    assert!(announcements[1].contains("Devora compacts"));
    assert_eq!(
        progress.last(),
        Some(&devora_agent_space::CompactionProgress::Idle),
        "{progress:?}"
    );
}

// Once the context is too full for any ordinary turn, the save-memory turn is refused, while `/compact` still works:
// Devora compacts without the save, and the agent refreshes its memory afterwards. Should `/compact` fail too, the
// transcript says so. Either way, Claude Code's error comes first, then what Devora does about it.
#[tokio::test]
async fn a_save_refused_as_too_long_compacts_without_it_and_a_failed_compact_says_so() {
    let save = "Devora: the user asked to compact";
    for compact_fails in [false, true] {
        let fixture = Fixture::new();
        let mut context = fixture.context();
        let too_long = if compact_fails {
            format!("{save}|/compact")
        } else {
            save.to_owned()
        };
        context
            .environment
            .insert("DEVORA_FAKE_TOO_LONG".into(), too_long);
        let space = fixture.create(fixture.form("Delta")).unwrap();
        let log = Arc::new(Log::open(&fixture.root.path().join("log")).unwrap());
        let mut running = space.start(&context, log, &fixture.run).await.unwrap();
        running
            .handle()
            .send_turn("first turn".into())
            .await
            .unwrap();
        turn_end(&mut running, &mut BTreeMap::new()).await;

        running.handle().compact_now().await.unwrap();
        // Every row from the compaction on, in transcript order.
        let mut rows = BTreeMap::new();
        let mut progress = None;
        let turns = if compact_fails { 2 } else { 3 };
        let mut ended = 0;
        tokio::time::timeout(Duration::from_secs(10), async {
            while let Some(update) = running.next_update().await {
                match update {
                    SpaceEvent::Context(context) => progress = Some(context.progress),
                    SpaceEvent::Transcript(TranscriptChange::Upsert { item }) => {
                        let row = match item.content {
                            TranscriptContent::Assistant { text, .. } => {
                                format!("assistant: {text}")
                            }
                            TranscriptContent::System {
                                kind: SystemKind::Compaction,
                                text,
                                ..
                            } => {
                                format!("compaction: {text}")
                            }
                            TranscriptContent::System {
                                kind: SystemKind::Warning,
                                text,
                                ..
                            } => {
                                format!("warning: {text}")
                            }
                            TranscriptContent::System {
                                kind: SystemKind::Failure,
                                text,
                                ..
                            } => {
                                format!("failure: {text}")
                            }
                            TranscriptContent::System {
                                kind: SystemKind::Result,
                                text,
                                ..
                            } => {
                                format!("result: {text}")
                            }
                            _ => continue,
                        };
                        rows.insert(item.sequence, row);
                    }
                    SpaceEvent::TurnEnded => ended += 1,
                    SpaceEvent::Session(SessionEvent::Failed(error)) => panic!("{error}"),
                    _ => {}
                }
                // A failure's row follows its turn's end.
                let failure_shown = rows.values().any(|row| row.starts_with("failure: "));
                if ended == turns && failure_shown == compact_fails {
                    break;
                }
            }
        })
        .await
        .unwrap();
        running.pause().await.unwrap();

        let report = std::fs::read_to_string(fixture.root.path().join("report.jsonl")).unwrap();
        let prompts: Vec<String> = report
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
            .filter(|entry| entry["kind"] == "user")
            .map(|entry| entry["message"]["content"].as_str().unwrap().to_owned())
            .collect();
        let rows: Vec<&str> = rows
            .values()
            .map(|row| match row.as_str() {
                "assistant: Prompt is too long" => row.as_str(),
                row if row.starts_with("assistant: ") => "assistant: (a reply)",
                row => row,
            })
            .collect();
        assert!(prompts[1].starts_with(save), "{prompts:?}");
        assert_eq!(prompts[2], "/compact");
        let mut expected = vec![
            "compaction: Compaction requested at 1% full: Devora asks the agent to save its memory, then compacts.",
            "assistant: Prompt is too long",
            "result: Turn ended with an error",
            "warning: The context is too full for the agent to save its memory first: Devora compacts the conversation now, and asks the agent to refresh its memory afterwards.",
        ];
        if compact_fails {
            assert_eq!(
                prompts.len(),
                3,
                "no refresh after a failed compaction: {prompts:?}"
            );
            expected.extend([
                "assistant: Prompt is too long",
                "result: Turn ended with an error",
                "failure: Devora couldn't compact the conversation. Press Compact Now to try again, or send /compact as a prompt.",
            ]);
        } else {
            assert_eq!(prompts.len(), 4, "{prompts:?}");
            assert!(
                prompts[3].starts_with("Devora: the conversation was just compacted"),
                "{prompts:?}"
            );
            expected.extend([
                "compaction: Conversation compacted",
                "result: Turn completed",
                "compaction: The conversation was compacted before the agent could save its memory: the agent is asked to refresh it.",
                "assistant: (a reply)",
                "result: Turn completed",
            ]);
        }
        assert_eq!(rows, expected);
        assert_eq!(progress, Some(devora_agent_space::CompactionProgress::Idle));
    }
}

// A quit asks every agent to pause, including one whose own pause after its
// turn (a hold's, say) has already stopped it. That isn't a problem: it gets
// the stop the actor recorded, graceful or forced, and only that one stop, so
// a forced one is reported and owes one notice.
#[tokio::test]
async fn pausing_an_agent_its_own_pause_already_stopped_reports_that_stop() {
    for forced in [false, true] {
        let fixture = Fixture::new();
        let mut context = fixture.context();
        let log = Arc::new(Log::open(&fixture.root.path().join("log")).unwrap());
        fixture.create(fixture.form("Delta")).unwrap();
        launch_and_settle(&fixture, &context, &log, &fixture.run).await;
        if forced {
            context
                .environment
                .insert("DEVORA_FAKE_HANG_AT_EOF".into(), "1".into());
        }
        let mut running = Space::open(&fixture.profile, "Delta")
            .unwrap()
            .start(&context, log.clone(), &fixture.run)
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(15), running.handle().pause_after_turn())
            .await
            .unwrap()
            .unwrap();
        let stop = running.pause().await.unwrap();
        assert_eq!(stop.graceful, !forced);
        let mut persisted = vec![];
        while let Some(update) = running.next_update().await {
            if let SpaceEvent::Session(SessionEvent::Stopped(info)) = update {
                persisted.push(info);
            }
        }
        assert_eq!(
            persisted,
            vec![stop],
            "one stop, the one the actor persisted"
        );
        context.environment.remove("DEVORA_FAKE_HANG_AT_EOF");
        let rows = launch_and_settle(&fixture, &context, &log, &fixture.run).await;
        assert_eq!(rows.len(), usize::from(forced), "{rows:?}");
    }
}

/// Launches in the fixture's run, starts the fake CLI's background work
/// (`prompt`), lets the turn end, then pauses after it, as the Pause button does.
async fn pause_with_background_work(
    fixture: &Fixture,
    context: &LaunchContext,
    log: &Arc<Log>,
    prompt: &str,
) -> Vec<SpaceEvent> {
    let mut running = Space::open(&fixture.profile, "Delta")
        .unwrap()
        .start(context, log.clone(), &fixture.run)
        .await
        .unwrap();
    running.handle().send_turn(prompt.into()).await.unwrap();
    let mut rows = std::collections::BTreeMap::new();
    turn_end(&mut running, &mut rows).await;
    let pausing = tokio::spawn(running.handle().pause_after_turn());
    let mut updates = vec![];
    tokio::time::timeout(Duration::from_secs(15), async {
        while let Some(update) = running.next_update().await {
            updates.push(update);
        }
    })
    .await
    .expect("the pause ends the space");
    pausing.await.unwrap().unwrap();
    updates
}

fn report_kinds(fixture: &Fixture) -> Vec<String> {
    std::fs::read_to_string(fixture.root.path().join("report.jsonl"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .filter(|entry| {
            ["stop_task", "notification_turn", "interrupt", "eof"]
                .contains(&entry["kind"].as_str().unwrap())
        })
        // The fake reports every control request by kind, and a stopped task again with its ID.
        .filter(|entry| entry["kind"] != "stop_task" || entry["task_id"].is_string())
        .map(|entry| match entry["task_id"].as_str() {
            Some(task) => format!("stop_task {task}"),
            None => entry["kind"].as_str().unwrap().to_owned(),
        })
        .collect()
}

// A pause stops the agent's background work through the CLI, shells first, cuts
// a stopped subagent's notification turn short (else the agent could start the
// work again), and only then closes the input, so the CLI exits by itself. The
// next launch tells the agent exactly what was stopped. With
// `DEVORA_FAKE_AGENT_TURN_DELAY` that turn starts only after the last
// `stop_task` returns, and the input must still stay open for it.
#[tokio::test]
async fn a_pause_stops_background_work_first_and_the_notice_names_it() {
    for late in [false, true] {
        let fixture = Fixture::new();
        let mut context = fixture.context();
        if late {
            context
                .environment
                .insert("DEVORA_FAKE_AGENT_TURN_DELAY".into(), "0.5".into());
        }
        let log = Arc::new(Log::open(&fixture.root.path().join("log")).unwrap());
        fixture.create(fixture.form("Delta")).unwrap();
        launch_and_settle(&fixture, &context, &log, &fixture.run).await;
        let updates = pause_with_background_work(&fixture, &context, &log, "background-work").await;
        let kinds = report_kinds(&fixture);
        let start = kinds
            .iter()
            .position(|kind| kind.starts_with("stop_task"))
            .unwrap();
        assert_eq!(
            kinds[start..],
            [
                "stop_task shell-1",
                "stop_task agent-1",
                "notification_turn",
                "interrupt",
                "eof"
            ],
            "late: {late}"
        );
        let stopped = updates
            .iter()
            .find_map(|update| match update {
                SpaceEvent::Session(SessionEvent::Stopped(info)) => Some(info.clone()),
                _ => None,
            })
            .unwrap();
        assert!(
            stopped.graceful,
            "nothing was left running, so the CLI exited by itself"
        );
        assert_eq!(
            launch_and_settle(&fixture, &context, &log, &fixture.run).await,
            vec![(
                "Devora paused the agent and stopped a watcher (shell), a CI wait (subagent): asked it to re-establish what it needs".into(),
                false
            )]
        );
        let sent = notices(&fixture);
        assert_eq!(sent.len(), 1);
        assert!(sent[0]["message"]["content"]
            .as_str()
            .unwrap()
            .contains("a watcher (shell), a CI wait (subagent)"));
    }
}

// A task that won't stop leaves the pause to the worker's grace and forced
// stop, and the notice still names everything the pause set out to stop.
#[tokio::test]
async fn a_task_that_wont_stop_falls_back_to_the_pause_and_its_notice_still_names_the_work() {
    let fixture = Fixture::new();
    let mut context = fixture.context();
    context
        .environment
        .insert("DEVORA_FAKE_STOP_FAILS".into(), "shell-1".into());
    let log = Arc::new(Log::open(&fixture.root.path().join("log")).unwrap());
    fixture.create(fixture.form("Delta")).unwrap();
    launch_and_settle(&fixture, &context, &log, &fixture.run).await;
    let updates =
        pause_with_background_work(&fixture, &context, &log, "background-work:shell").await;
    assert!(updates.iter().any(|update| matches!(update,
        SpaceEvent::Session(SessionEvent::Warning(warning)) if warning.contains("couldn't stop a background task"))));
    let rows = launch_and_settle(&fixture, &context, &log, &fixture.run).await;
    assert_eq!(rows.len(), 1);
    assert!(rows[0].0.contains("stopped a watcher (shell)"), "{rows:?}");
}

async fn next_chosen(running: &mut RunningSpace) -> ChosenModel {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let Some(SpaceEvent::ChosenModel(chosen)) = running.next_update().await {
                return chosen;
            }
        }
    })
    .await
    .unwrap()
}

fn models_told(fixture: &Fixture) -> Vec<String> {
    std::fs::read_to_string(fixture.root.path().join("report.jsonl"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .filter(|entry| entry["kind"] == "set_model")
        .map(|entry| entry["model"].as_str().unwrap().to_owned())
        .collect()
}

#[tokio::test]
async fn a_chosen_model_is_stored_at_once_and_told_to_the_cli_only_between_turns_the_last_one_winning(
) {
    let fixture = Fixture::new();
    let context = fixture.context();
    let space = fixture.create(fixture.form("Foxtrot")).unwrap();
    let log = Arc::new(Log::open(&fixture.root.path().join("log")).unwrap());
    let mut running = space
        .start(&context, log.clone(), &fixture.run)
        .await
        .unwrap();
    let handle = running.handle();
    // The fake CLI holds this turn on a permission question until it is answered, then replies.
    handle
        .send_turn("permission-then-reply".into())
        .await
        .unwrap();
    let request = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let Some(SpaceEvent::Session(SessionEvent::PermissionRequested {
                request_id, ..
            })) = running.next_update().await
            {
                return request_id;
            }
        }
    })
    .await
    .unwrap();

    handle.set_model("claude-sonnet-5-5".into()).await.unwrap();
    handle.set_model("claude-opus-5-5".into()).await.unwrap();
    let chosen = ChosenModel {
        set_to: Some("claude-opus-5-5".into()),
        awaiting_reply: true,
        not_taken: None,
    };
    assert_eq!(
        next_chosen(&mut running).await.set_to.as_deref(),
        Some("claude-sonnet-5-5")
    );
    assert_eq!(next_chosen(&mut running).await, chosen);
    assert_eq!(
        models_told(&fixture),
        Vec::<String>::new(),
        "never mid-turn"
    );

    handle
        .answer_permission(request, devora_agent::session::PermissionAnswer::Allow)
        .await
        .unwrap();
    // The turn's own replies came from the model it started on, so the change still awaits one.
    assert_eq!(next_chosen(&mut running).await, chosen, "told, and taken");
    assert_eq!(models_told(&fixture), ["claude-opus-5-5"]);
    handle.send_turn("hello".into()).await.unwrap();
    assert_eq!(
        next_chosen(&mut running).await,
        ChosenModel {
            awaiting_reply: false,
            ..chosen
        },
        "the first reply since the change clears its mark"
    );

    // Idle: told at once. A CLI that refuses it keeps the stored choice, and says so.
    handle.set_model("refused".into()).await.unwrap();
    let refused = ChosenModel {
        set_to: Some("refused".into()),
        awaiting_reply: true,
        not_taken: None,
    };
    assert_eq!(next_chosen(&mut running).await, refused);
    assert_eq!(
        next_chosen(&mut running).await,
        ChosenModel {
            not_taken: Some("model refused".into()),
            awaiting_reply: false,
            ..refused
        },
        "a refused model never replies, so nothing awaits it"
    );
    assert!(running.pause().await.unwrap().graceful);
    let stored = Space::open(&fixture.profile, "Foxtrot").unwrap();
    assert_eq!(stored.form().model.as_ref().unwrap().model, "refused");
}

/// A space whose fake CLI starts turns of its own (as when a background task reports) on cues the test writes.
struct OwnTurns {
    fixture: Fixture,
    cues: PathBuf,
    log: Arc<Log>,
}

impl OwnTurns {
    fn new() -> Self {
        let fixture = Fixture::new();
        let cues = fixture.root.path().join("own-turns");
        std::fs::create_dir_all(&cues).unwrap();
        let log = Arc::new(Log::open(&fixture.root.path().join("log")).unwrap());
        fixture.create(fixture.form("Delta")).unwrap();
        Self { fixture, cues, log }
    }

    async fn start(&self) -> RunningSpace {
        let mut context = self.fixture.context();
        context.environment.insert(
            "DEVORA_FAKE_OWN_TURNS".into(),
            self.cues.display().to_string(),
        );
        Space::open(&self.fixture.profile, "Delta")
            .unwrap()
            .start(&context, self.log.clone(), &self.fixture.run)
            .await
            .unwrap()
    }

    fn cue(&self, name: &str) {
        std::fs::write(self.cues.join(name), "").unwrap();
    }

    /// Starts a turn of the CLI's own, and returns once the actor has seen it start.
    async fn begin(&self, running: &mut RunningSpace) {
        self.cue("start");
        until(running, |update| {
            matches!(
                update,
                SpaceEvent::Session(SessionEvent::Message(Message::Known(KnownMessage::System(
                    devora_agent::protocol::System::Init { .. }
                ))))
            )
        })
        .await;
    }

    /// The fake's report, in order: what reached the CLI, and its own turns ending.
    fn reported(&self) -> Vec<String> {
        std::fs::read_to_string(self.fixture.root.path().join("report.jsonl"))
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
            .filter_map(|entry| match entry["kind"].as_str().unwrap() {
                "own_turn_ended" | "eof" | "set_model" | "interrupt" => {
                    Some(entry["kind"].as_str().unwrap().to_owned())
                }
                "user" => Some(format!(
                    "user {}",
                    entry["message"]["content"].as_str().unwrap_or("")
                )),
                _ => None,
            })
            .collect()
    }
}

impl OwnTurns {
    /// The report once it has an entry starting with `prefix`: Devora publishes what it sends once it's committed,
    /// before the CLI has read it.
    async fn reported_with(&self, prefix: &str) -> Vec<String> {
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                let reported = self.reported();
                if reported.iter().any(|entry| entry.starts_with(prefix)) {
                    return reported;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap_or_else(|_| panic!("no {prefix} in {:?}", self.reported()))
    }
}

/// Waits for an update `done` accepts.
async fn until(running: &mut RunningSpace, mut done: impl FnMut(&SpaceEvent) -> bool) {
    tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(update) = running.next_update().await {
            if done(&update) {
                return;
            }
        }
        panic!("the space ended");
    })
    .await
    .expect("the update came")
}

fn position(reported: &[String], prefix: &str) -> usize {
    reported
        .iter()
        .position(|entry| entry.starts_with(prefix))
        .unwrap_or_else(|| panic!("no {prefix} in {reported:?}"))
}

// A pause during a turn the CLI started itself waits for that turn's end, as it does for a turn Devora sent: the
// CLI's input closes only after it.
#[tokio::test]
async fn a_pause_waits_for_a_turn_the_cli_started_itself() {
    let space = OwnTurns::new();
    let mut running = space.start().await;
    space.begin(&mut running).await;
    let pausing = tokio::spawn(running.handle().pause_after_turn());
    // Commands are handled in order: once this one is answered, the actor has seen the pause.
    running.handle().set_compact_threshold(None).await.unwrap();
    assert!(!pausing.is_finished());
    space.cue("end");
    tokio::time::timeout(Duration::from_secs(10), pausing)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let reported = space.reported();
    assert!(
        position(&reported, "own_turn_ended") < position(&reported, "eof"),
        "{reported:?}"
    );
}

// A model chosen during a turn the CLI started itself is told to the CLI once that turn ends, never mid-turn.
#[tokio::test]
async fn a_model_chosen_during_a_turn_the_cli_started_itself_is_told_after_it() {
    let space = OwnTurns::new();
    let mut running = space.start().await;
    space.begin(&mut running).await;
    running.handle().set_model("sonnet".into()).await.unwrap();
    assert!(!space.reported().iter().any(|entry| entry == "set_model"));
    space.cue("end");
    let reported = space.reported_with("set_model").await;
    assert!(
        position(&reported, "own_turn_ended") < position(&reported, "set_model"),
        "{reported:?}"
    );
    running.pause().await.unwrap();
}

// A compaction asked for during a turn the CLI started itself starts at that turn's end, as during Devora's own.
#[tokio::test]
async fn a_compaction_asked_for_during_a_turn_the_cli_started_itself_starts_at_its_end() {
    let space = OwnTurns::new();
    let mut running = space.start().await;
    space.begin(&mut running).await;
    running.handle().compact_now().await.unwrap();
    assert!(!space
        .reported()
        .iter()
        .any(|entry| entry.starts_with("user ")));
    space.cue("end");
    let reported = space.reported_with("user ").await;
    assert!(
        position(&reported, "own_turn_ended") < position(&reported, "user "),
        "{reported:?}"
    );
    running.pause().await.unwrap();
}

// A prompt sent during a turn the CLI started itself joins that turn, whose one result answers it (as the pinned CLI
// does): the turn is then Devora's, it ends with that result, and the agent pauses after it.
#[tokio::test]
async fn a_prompt_sent_during_a_turn_the_cli_started_itself_joins_it() {
    let space = OwnTurns::new();
    let mut running = space.start().await;
    space.begin(&mut running).await;
    running.handle().send_turn("drift".into()).await.unwrap();
    until(&mut running, |update| {
        matches!(update, SpaceEvent::TurnEnded)
    })
    .await;
    tokio::time::timeout(Duration::from_secs(10), running.handle().pause_after_turn())
        .await
        .expect("no turn is left running")
        .unwrap();
}

// However a turn the CLI started itself ends (its result, Stop's interrupted result, or the CLI crashing during it),
// it ends: nothing is left waiting for it, and the agent pauses at once afterwards.
#[tokio::test]
async fn a_turn_the_cli_started_itself_never_outlives_its_end() {
    for ending in ["result", "stop", "crash"] {
        let space = OwnTurns::new();
        let mut running = space.start().await;
        space.begin(&mut running).await;
        match ending {
            "result" => space.cue("end"),
            "stop" => {
                running.handle().interrupt().await.unwrap();
            }
            _ => space.cue("crash"),
        }
        until(&mut running, |update| match ending {
            "crash" => matches!(update, SpaceEvent::Session(SessionEvent::Exited { .. })),
            _ => matches!(
                update,
                SpaceEvent::Session(SessionEvent::Message(Message::Known(KnownMessage::Result(
                    _
                ))))
            ),
        })
        .await;
        if ending == "crash" {
            // The worker restarts the CLI after a crash; the agent is idle again once it's back.
            until(&mut running, |update| {
                matches!(update, SpaceEvent::Session(SessionEvent::Ready { .. }))
            })
            .await;
        }
        tokio::time::timeout(Duration::from_secs(10), running.handle().pause_after_turn())
            .await
            .unwrap_or_else(|_| panic!("{ending}: the turn stuck"))
            .unwrap();
    }
}

async fn next_chosen_effort(running: &mut RunningSpace) -> ChosenEffort {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let Some(SpaceEvent::ChosenEffort(chosen)) = running.next_update().await {
                return chosen;
            }
        }
    })
    .await
    .unwrap()
}

fn efforts_told(fixture: &Fixture) -> Vec<serde_json::Value> {
    std::fs::read_to_string(fixture.root.path().join("report.jsonl"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .filter(|entry| entry["kind"] == "apply_flag_settings")
        .map(|entry| entry["settings"]["effortLevel"].clone())
        .collect()
}

#[tokio::test]
async fn a_chosen_effort_is_stored_at_once_and_told_to_the_cli_only_between_turns_the_last_one_winning(
) {
    let fixture = Fixture::new();
    let mut context = fixture.context();
    context
        .environment
        .insert("DEVORA_FAKE_EFFORT_REFUSED".into(), "low".into());
    let space = fixture.create(fixture.form("Golf")).unwrap();
    let log = Arc::new(Log::open(&fixture.root.path().join("log")).unwrap());
    let mut running = space
        .start(&context, log.clone(), &fixture.run)
        .await
        .unwrap();
    let handle = running.handle();
    // The fake CLI holds this turn on a permission question until it is answered.
    handle.send_turn("permission".into()).await.unwrap();
    let request = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let Some(SpaceEvent::Session(SessionEvent::PermissionRequested {
                request_id, ..
            })) = running.next_update().await
            {
                return request_id;
            }
        }
    })
    .await
    .unwrap();

    handle.set_effort(Some("high".into())).await.unwrap();
    handle.set_effort(Some("max".into())).await.unwrap();
    let max = ChosenEffort {
        set_to: Some("max".into()),
        pending: true,
        not_taken: None,
    };
    assert_eq!(
        next_chosen_effort(&mut running).await.set_to.as_deref(),
        Some("high")
    );
    assert_eq!(next_chosen_effort(&mut running).await, max);
    assert!(efforts_told(&fixture).is_empty(), "never mid-turn");

    handle
        .answer_permission(request, devora_agent::session::PermissionAnswer::Allow)
        .await
        .unwrap();
    assert_eq!(
        next_chosen_effort(&mut running).await,
        ChosenEffort {
            pending: false,
            ..max
        },
        "told when the turn ended"
    );
    assert_eq!(efforts_told(&fixture), [serde_json::json!("max")]);

    // Idle: told at once. Claude Code default is no level; a CLI that refuses a level keeps it stored, and says why.
    handle.set_effort(None).await.unwrap();
    assert_eq!(next_chosen_effort(&mut running).await.set_to, None);
    handle.set_effort(Some("low".into())).await.unwrap();
    let refused = loop {
        let chosen = next_chosen_effort(&mut running).await;
        if chosen.not_taken.is_some() {
            break chosen;
        }
    };
    assert_eq!(
        refused,
        ChosenEffort {
            set_to: Some("low".into()),
            pending: false,
            not_taken: Some("effort refused".into())
        }
    );
    assert_eq!(
        efforts_told(&fixture),
        [
            serde_json::json!("max"),
            serde_json::Value::Null,
            serde_json::json!("low")
        ]
    );
    assert!(running.pause().await.unwrap().graceful);
    let stored = Space::open(&fixture.profile, "Golf").unwrap();
    assert_eq!(
        stored.form().model.as_ref().unwrap().effort.as_deref(),
        Some("low")
    );
}

// The chat learns of a turn the CLI started itself from the space: `AgentTurnStarted` and `AgentTurnEnded` bracket it.
// A prompt sent during it joins it, so that turn ends as the prompt's, with one `TurnEnded` and no `AgentTurnEnded`.
#[tokio::test]
async fn the_space_publishes_a_turn_the_cli_started_itself_and_its_end() {
    let space = OwnTurns::new();
    let mut running = space.start().await;
    space.cue("start");
    until(&mut running, |update| {
        matches!(update, SpaceEvent::AgentTurnStarted)
    })
    .await;
    space.cue("end");
    until(&mut running, |update| {
        matches!(update, SpaceEvent::AgentTurnEnded)
    })
    .await;

    space.cue("start");
    until(&mut running, |update| {
        matches!(update, SpaceEvent::AgentTurnStarted)
    })
    .await;
    running.handle().send_turn("drift".into()).await.unwrap();
    let mut ends = vec![];
    until(&mut running, |update| {
        match update {
            SpaceEvent::TurnEnded => ends.push("ours"),
            SpaceEvent::AgentTurnEnded => ends.push("the agent's"),
            _ => {}
        }
        matches!(update, SpaceEvent::TurnEnded)
    })
    .await;
    assert_eq!(ends, ["ours"]);
    running.pause().await.unwrap();
}

/// The user row with `text`, as `update` carries it, if it does.
fn user_row(update: &SpaceEvent, text: &str) -> Option<Option<devora_agent::transcript::Delivery>> {
    match update {
        SpaceEvent::Transcript(TranscriptChange::Upsert { item }) => match &item.content {
            TranscriptContent::User {
                text: sent,
                delivery,
                ..
            } if sent == text => Some(*delivery),
            _ => None,
        },
        _ => None,
    }
}

fn stored_delivery(fixture: &Fixture, text: &str) -> Option<devora_agent::transcript::Delivery> {
    Space::read_history(&fixture.profile, "Delta", 0, 500)
        .unwrap()
        .into_iter()
        .find_map(|item| match item.content {
            TranscriptContent::User {
                text: sent,
                delivery,
                ..
            } if sent == text => Some(delivery),
            _ => None,
        })
        .expect("the row is stored")
}

impl OwnTurns {
    /// As `start`, with a prompt sent into the CLI's own turn waiting for the agent's next step (the `step` cue).
    async fn start_waiting(&self) -> RunningSpace {
        let mut context = self.fixture.context();
        context.environment.insert(
            "DEVORA_FAKE_OWN_TURNS".into(),
            self.cues.display().to_string(),
        );
        context
            .environment
            .insert("DEVORA_FAKE_FOLDED_WAITS".into(), "1".into());
        Space::open(&self.fixture.profile, "Delta")
            .unwrap()
            .start(&context, self.log.clone(), &self.fixture.run)
            .await
            .unwrap()
    }
}

// A prompt sent into a turn the agent started itself waits for the agent's next step, and its row says so until the
// CLI reports the agent has it ("started"); stored, as every row is.
#[tokio::test]
async fn a_prompt_sent_into_the_agents_turn_waits_until_the_agent_has_it() {
    use devora_agent::transcript::Delivery;
    let space = OwnTurns::new();
    let mut running = space.start_waiting().await;
    space.begin(&mut running).await;
    running.handle().send_turn("Joins it".into()).await.unwrap();
    until(&mut running, |update| {
        user_row(update, "Joins it") == Some(Some(Delivery::Waiting))
    })
    .await;
    assert_eq!(
        stored_delivery(&space.fixture, "Joins it"),
        Some(Delivery::Waiting)
    );
    space.cue("step");
    until(&mut running, |update| {
        user_row(update, "Joins it") == Some(None)
    })
    .await;
    assert_eq!(stored_delivery(&space.fixture, "Joins it"), None);
    running.pause().await.unwrap();
}

// A prompt sent just as the agent starts a turn of its own, before Devora has read that turn's start, joins it and waits
// like one sent into it.
#[tokio::test]
async fn a_prompt_sent_as_the_agent_starts_its_own_turn_waits_until_the_agent_has_it() {
    use devora_agent::transcript::Delivery;
    let space = OwnTurns::new();
    let mut running = space.start_waiting().await;
    space.cue("race");
    running
        .handle()
        .send_turn("Just in time".into())
        .await
        .unwrap();
    until(&mut running, |update| {
        user_row(update, "Just in time") == Some(Some(Delivery::Waiting))
    })
    .await;
    assert_eq!(
        stored_delivery(&space.fixture, "Just in time"),
        Some(Delivery::Waiting)
    );
    space.cue("step");
    until(&mut running, |update| {
        user_row(update, "Just in time") == Some(None)
    })
    .await;
    assert_eq!(stored_delivery(&space.fixture, "Just in time"), None);
    running.pause().await.unwrap();
}

// A prompt sent while the agent is idle reaches it at once: its row is never marked.
#[tokio::test]
async fn a_prompt_sent_while_the_agent_is_idle_is_never_marked() {
    let space = OwnTurns::new();
    let mut running = space.start_waiting().await;
    running.handle().send_turn("drift".into()).await.unwrap();
    let mut marks = vec![];
    until(&mut running, |update| {
        if let Some(delivery) = user_row(update, "drift") {
            marks.push(delivery);
        }
        matches!(update, SpaceEvent::TurnEnded)
    })
    .await;
    assert!(
        !marks.is_empty() && marks.iter().all(Option::is_none),
        "{marks:?}"
    );
    running.pause().await.unwrap();
}

// A crash while a prompt waits loses it (the resumed session never gets it, probed on the pinned CLI): its row says it
// wasn't delivered. So does a row a dead Devora left waiting, at the next launch.
#[tokio::test]
async fn a_prompt_whose_session_ends_before_the_agent_has_it_was_not_delivered() {
    use devora_agent::transcript::Delivery;
    let space = OwnTurns::new();
    let mut running = space.start_waiting().await;
    space.begin(&mut running).await;
    running
        .handle()
        .send_turn("Lost in a crash".into())
        .await
        .unwrap();
    until(&mut running, |update| {
        user_row(update, "Lost in a crash") == Some(Some(Delivery::Waiting))
    })
    .await;
    space.cue("crash");
    until(&mut running, |update| {
        user_row(update, "Lost in a crash") == Some(Some(Delivery::NotDelivered))
    })
    .await;
    assert_eq!(
        stored_delivery(&space.fixture, "Lost in a crash"),
        Some(Delivery::NotDelivered)
    );
    running.pause().await.unwrap();

    // Devora dying leaves the row waiting: the next launch knows the message never reached the agent.
    let database = space
        .fixture
        .profile
        .join("agent-spaces/Delta/.agent-space.db");
    rusqlite::Connection::open(&database)
        .unwrap()
        .execute(
            "UPDATE events SET payload=json_set(payload,'$.content.User.delivery','Waiting') WHERE kind='user'",
            [],
        )
        .unwrap();
    let mut relaunched = space.start().await;
    let Some(SpaceEvent::Restored(restored)) = relaunched.next_update().await else {
        panic!("expected the restored tail")
    };
    assert!(restored.iter().any(|item| matches!(&item.content,
        TranscriptContent::User { text, delivery: Some(Delivery::NotDelivered), .. } if text == "Lost in a crash")));
    relaunched.pause().await.unwrap();
}

// A send while the CLI restarts after a crash isn't one that joined a turn: whether the worker refuses it or takes it,
// its row never waits.
#[tokio::test]
async fn a_send_during_a_crash_restart_leaves_no_waiting_row() {
    use devora_agent::transcript::Delivery;
    let space = OwnTurns::new();
    let mut running = space.start_waiting().await;
    space.begin(&mut running).await;
    space.cue("crash");
    until(&mut running, |update| {
        matches!(update, SpaceEvent::Session(SessionEvent::Exited { .. }))
    })
    .await;
    let _ = running
        .handle()
        .send_turn("During the restart".into())
        .await;
    let mut marks = vec![];
    tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(update) = running.next_update().await {
            if let Some(delivery) = user_row(&update, "During the restart") {
                marks.push(delivery);
            }
            if matches!(update, SpaceEvent::TurnEnded)
                || matches!(update, SpaceEvent::Session(SessionEvent::Ready { .. }))
                    && !marks.is_empty()
            {
                break;
            }
        }
    })
    .await
    .ok();
    assert!(!marks.contains(&Some(Delivery::Waiting)), "{marks:?}");
    assert_ne!(
        stored_delivery(&space.fixture, "During the restart"),
        Some(Delivery::Waiting)
    );
    running.pause().await.unwrap();
}

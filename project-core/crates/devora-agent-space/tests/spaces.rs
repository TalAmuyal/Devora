use devora_agent::protocol::{KnownMessage, Message};
use devora_agent::session::SessionEvent;
use devora_agent::transcript::{SystemKind, TranscriptChange, TranscriptContent};
use devora_agent_space::{template, CreationForm, LaunchContext, Space, SpaceEvent};
use devora_app::log::Log;
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
        }
    }
    fn form(&self, name: &str) -> CreationForm {
        CreationForm {
            name: name.into(),
            role: None,
            repos: vec![],
            inputs: BTreeMap::new(),
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
        use std::os::unix::fs::PermissionsExt;
        let python = Command::new("python3")
            .args(["-c", "import sys;print(sys.executable)"])
            .output()
            .unwrap();
        assert!(python.status.success());
        let source = include_str!("../../devora-agent/tests/support/fake_cli.py")
            .split_once('\n')
            .unwrap()
            .1;
        let program = self.root.path().join("fake-cli");
        std::fs::write(
            &program,
            format!(
                "#!{}\n{source}",
                String::from_utf8(python.stdout).unwrap().trim()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        LaunchContext {
            global_config: self.global.clone(),
            program,
            plugins: vec![],
            settings: json!({"env":{"SPACE_CONFIG":"explicit"}}),
            environment: BTreeMap::from([
                ("PATH".into(), std::env::var("PATH").unwrap()),
                ("HOME".into(), self.root.path().display().to_string()),
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
        "Delta|form role|form {{literal}}|from global|profile user"
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
    assert_eq!(options.effort, "low");
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
    let mut running = space.start(&context, log.clone()).await.unwrap();
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
    let mut resumed = reopened.start(&context, log).await.unwrap();
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
    let mut running = space.start(&context, log).await.unwrap();
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
                some("General assistant"),
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
    let mut running = space.start(&context, log).await.unwrap();
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
    assert_eq!(version(), 2);
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
    let mut running = space.start(&fixture.context(), log).await.unwrap();
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
    assert_eq!(version(), 2);
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
            },
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
    let mut running = reopened.start(&fixture.context(), log).await.unwrap();
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
    let mut running = space.start(&context, log).await.unwrap();
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
    let mut running = space.start(&context, log.clone()).await.unwrap();
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
    let pausing = tokio::spawn({
        let handle = handle.clone();
        async move { handle.pause_after_turn().await }
    });
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
    // Already paused; this only waits for the space to release its ownership.
    assert!(running.pause().await.is_err());

    // An idle agent pauses at once.
    let idle = Space::open(&fixture.profile, "Delta").unwrap();
    let running = idle.start(&context, log).await.unwrap();
    tokio::time::timeout(Duration::from_secs(10), running.handle().pause_after_turn())
        .await
        .unwrap()
        .unwrap();
}

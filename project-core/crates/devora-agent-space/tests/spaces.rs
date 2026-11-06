use devora_agent::protocol::{KnownMessage, Message};
use devora_agent::session::SessionEvent;
use devora_agent_space::{template, CreationForm, LaunchContext, Space};
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

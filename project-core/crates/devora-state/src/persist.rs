//! The SQLite database a persistent [`crate::Store`] writes through to (ADR-008).
//!
//! Each command's resulting state is written in one transaction, replacing every row: the state is a handful of rows, and a full rewrite can't drift from the reducer the way a per-event mapping could.
//! WAL mode with `synchronous=NORMAL` keeps every committed command across a process crash or `kill -9`; a power loss may drop the last few.

use crate::model::{AppState, Pane, Session, SessionKind, Tab, Window};
use crate::{State, StateError};
use rusqlite::{params, Connection};
use std::collections::HashMap;
use std::path::Path;

/// Entry `i` migrates the schema from version `i` to `i + 1`, the version being the database's `user_version`.
///
/// A new database runs them all; a released entry never changes, so a schema change appends one.
const MIGRATIONS: &[&str] = &["
    CREATE TABLE meta (
        id INTEGER PRIMARY KEY CHECK (id = 0),
        next_id INTEGER NOT NULL,
        active_tab INTEGER,
        active_profile TEXT
    );
    CREATE TABLE tabs (
        id INTEGER PRIMARY KEY,
        position INTEGER NOT NULL UNIQUE,
        title TEXT NOT NULL,
        kind TEXT NOT NULL,
        kind_data TEXT,
        profile_path TEXT
    );
    CREATE TABLE panes (
        id INTEGER PRIMARY KEY,
        tab_id INTEGER NOT NULL REFERENCES tabs (id),
        kind TEXT NOT NULL
    );
    INSERT INTO meta (id, next_id) VALUES (0, 0);
"];

pub(crate) struct Db {
    conn: Connection,
}

impl Db {
    /// Opens (or creates) the database at `path` and loads its state, failing any workspace creation a previous run left in progress.
    pub(crate) fn open(path: &Path) -> Result<(Self, State), StateError> {
        let fail = |error: Error| StateError::Storage {
            message: format!("couldn't open the app state at {}: {error}", path.display()),
        };
        let mut db = Self {
            conn: connect(path).map_err(fail)?,
        };
        let mut state = load(&db.conn).map_err(fail)?;
        if state.interrupt_creations() {
            db.save(&state)?;
        }
        Ok((db, state))
    }

    pub(crate) fn save(&mut self, state: &State) -> Result<(), StateError> {
        write(&mut self.conn, state).map_err(|error| StateError::Storage {
            message: format!("couldn't save the app state: {error}"),
        })
    }
}

#[derive(Debug)]
enum Error {
    Sqlite(rusqlite::Error),
    Io(std::io::Error),
    Invalid(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sqlite(error) => error.fmt(f),
            Self::Io(error) => error.fmt(f),
            Self::Invalid(why) => f.write_str(why),
        }
    }
}

fn inconsistent(why: String) -> Error {
    Error::Invalid(format!("the stored state is inconsistent: {why}"))
}

impl From<rusqlite::Error> for Error {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}

fn connect(path: &Path) -> Result<Connection, Error> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(Error::Io)?;
    }
    let mut conn = Connection::open(path)?;
    let mode: String =
        conn.pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get(0))?;
    if !mode.eq_ignore_ascii_case("wal") {
        return Err(Error::Invalid(format!("journal mode is {mode}, not WAL")));
    }
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "foreign_keys", true)?;
    migrate(&mut conn)?;
    Ok(conn)
}

fn migrate(conn: &mut Connection) -> Result<(), Error> {
    let tx = conn.transaction()?;
    let version: usize = tx.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version > MIGRATIONS.len() {
        return Err(Error::Invalid(format!(
            "its schema version {version} is newer than this build's {}",
            MIGRATIONS.len()
        )));
    }
    for migration in &MIGRATIONS[version..] {
        tx.execute_batch(migration)?;
    }
    tx.pragma_update(None, "user_version", MIGRATIONS.len())?;
    tx.commit()?;
    Ok(())
}

fn load(conn: &Connection) -> Result<State, Error> {
    let (next_id, active_tab, active_profile): (u64, Option<u64>, Option<String>) = conn
        .query_row(
            "SELECT next_id, active_tab, active_profile FROM meta WHERE id = 0",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;

    let mut panes = HashMap::new();
    let mut statement = conn.prepare("SELECT id, tab_id, kind FROM panes")?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let (id, tab, kind): (u64, u64, String) = (row.get(0)?, row.get(1)?, row.get(2)?);
        let pane = match kind.as_str() {
            "terminal" => Pane::Terminal { id },
            _ => return Err(inconsistent(format!("unknown pane kind {kind:?}"))),
        };
        if panes.insert(tab, pane).is_some() {
            return Err(inconsistent(format!("tab {tab} has several panes")));
        }
    }

    let mut tabs = Vec::new();
    let mut statement = conn
        .prepare("SELECT id, title, kind, kind_data, profile_path FROM tabs ORDER BY position")?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let id: u64 = row.get(0)?;
        let kind: String = row.get(2)?;
        let kind = session_kind(&kind, row.get(3)?)
            .ok_or_else(|| inconsistent(format!("tab {id} has an unknown kind {kind:?}")))?;
        let pane = panes
            .remove(&id)
            .ok_or_else(|| inconsistent(format!("tab {id} has no pane")))?;
        tabs.push(Tab {
            id,
            title: row.get(1)?,
            session: Session {
                kind,
                profile_path: row.get(4)?,
            },
            pane,
        });
    }

    let mut ids = tabs.iter().flat_map(|tab| {
        let Pane::Terminal { id: pane } = tab.pane;
        [tab.id, pane]
    });
    if ids.any(|id| id > next_id) {
        return Err(inconsistent(format!(
            "an id is above the next id {next_id}"
        )));
    }
    let active_is_valid = match active_tab {
        Some(active) => tabs.iter().any(|tab| tab.id == active),
        None => tabs.is_empty(),
    };
    if !active_is_valid {
        return Err(inconsistent(format!(
            "the active tab {active_tab:?} doesn't match its tabs"
        )));
    }

    Ok(State {
        app: AppState {
            window: Window { tabs, active_tab },
            active_profile,
        },
        next_id,
    })
}

fn write(conn: &mut Connection, state: &State) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM panes", [])?;
    tx.execute("DELETE FROM tabs", [])?;
    {
        let mut insert_tab = tx.prepare_cached(
            "INSERT INTO tabs (id, position, title, kind, kind_data, profile_path) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )?;
        let mut insert_pane =
            tx.prepare_cached("INSERT INTO panes (id, tab_id, kind) VALUES (?1, ?2, ?3)")?;
        for (position, tab) in state.app.window.tabs.iter().enumerate() {
            let (kind, kind_data) = session_kind_row(&tab.session.kind);
            insert_tab.execute(params![
                tab.id,
                position,
                tab.title,
                kind,
                kind_data,
                tab.session.profile_path
            ])?;
            let Pane::Terminal { id: pane } = tab.pane;
            insert_pane.execute(params![pane, tab.id, "terminal"])?;
        }
    }
    tx.execute(
        "UPDATE meta SET next_id = ?1, active_tab = ?2, active_profile = ?3 WHERE id = 0",
        params![
            state.next_id,
            state.app.window.active_tab,
            state.app.active_profile
        ],
    )?;
    tx.commit()
}

fn session_kind_row(kind: &SessionKind) -> (&'static str, Option<&str>) {
    match kind {
        SessionKind::Shell => ("shell", None),
        SessionKind::CreatingWorkspace => ("creating_workspace", None),
        SessionKind::CreationFailed { reason } => ("creation_failed", Some(reason)),
        SessionKind::Workspace { path } => ("workspace", Some(path)),
    }
}

fn session_kind(kind: &str, data: Option<String>) -> Option<SessionKind> {
    Some(match (kind, data) {
        ("shell", None) => SessionKind::Shell,
        ("creating_workspace", None) => SessionKind::CreatingWorkspace,
        ("creation_failed", Some(reason)) => SessionKind::CreationFailed { reason },
        ("workspace", Some(path)) => SessionKind::Workspace { path },
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use crate::{AppState, Command, Event, Pane, SessionKind, State, StateError, Store};
    use rusqlite::Connection;
    use std::io::{BufRead, BufReader, Write};
    use std::path::{Path, PathBuf};
    use std::process::Stdio;
    use tempfile::TempDir;

    fn db_path() -> (TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state").join("state.db");
        (dir, path)
    }

    fn open_tab(store: &Store, title: &str, kind: SessionKind) -> u64 {
        let events = store
            .dispatch(Command::OpenSession {
                title: title.into(),
                kind,
                profile_path: Some("/profiles/work".into()),
            })
            .unwrap();
        match &events[0] {
            Event::SessionOpened { tab, .. } => tab.id,
            other => panic!("expected SessionOpened, got {other:?}"),
        }
    }

    fn storage_error(result: Result<Store, StateError>) -> String {
        match result {
            Err(StateError::Storage { message }) => message,
            Err(other) => panic!("expected a storage error, got {other:?}"),
            Ok(_) => panic!("expected a storage error, got a store"),
        }
    }

    #[test]
    fn a_reopened_store_has_the_state_it_was_left_with() {
        let (_dir, path) = db_path();
        let store = Store::open(&path).unwrap();
        let shell = open_tab(&store, "Shell", SessionKind::Shell);
        let api = open_tab(
            &store,
            "api",
            SessionKind::Workspace {
                path: "/ws/1".into(),
            },
        );
        let failed = open_tab(&store, "task", SessionKind::CreatingWorkspace);
        for command in [
            Command::WorkspaceCreationFailed {
                tab: failed,
                reason: "clone failed".into(),
            },
            Command::MoveTab {
                tab: shell,
                index: 2,
            },
            Command::SetTitle {
                tab: api,
                title: "vim".into(),
            },
            Command::ActivateTab { tab: api },
            Command::SetActiveProfile {
                profile_path: Some("/profiles/work".into()),
            },
        ] {
            store.dispatch(command).unwrap();
        }
        let left = store.state();
        drop(store);

        assert_eq!(Store::open(&path).unwrap().state(), left);
    }

    #[test]
    fn ids_stay_unique_across_restarts() {
        let (_dir, path) = db_path();
        let store = Store::open(&path).unwrap();
        let first = open_tab(&store, "a", SessionKind::Shell);
        store
            .dispatch(Command::CloseSession { tab: first })
            .unwrap();
        drop(store);

        let store = Store::open(&path).unwrap();
        let second = open_tab(&store, "b", SessionKind::Shell);
        let Pane::Terminal { id: pane } = store.snapshot().window.tabs[0].pane;
        assert!(
            second > first && pane > first,
            "{first} was reused after the restart"
        );
    }

    #[test]
    fn a_workspace_creation_left_in_progress_restores_as_interrupted() {
        let (_dir, path) = db_path();
        let store = Store::open(&path).unwrap();
        let tab = open_tab(&store, "task", SessionKind::CreatingWorkspace);
        drop(store);

        let store = Store::open(&path).unwrap();
        assert_eq!(
            store.snapshot().window.tabs[0].session.kind,
            SessionKind::CreationFailed {
                reason: "interrupted".into()
            }
        );
        assert_eq!(
            store.dispatch(Command::WorkspaceCreated {
                tab,
                path: "/ws".into()
            }),
            Err(StateError::NotCreating { tab })
        );
        drop(store);
        let kind: String = Connection::open(&path)
            .unwrap()
            .query_row("SELECT kind FROM tabs", [], |row| row.get(0))
            .unwrap();
        assert_eq!(kind, "creation_failed", "the restore is saved too");
    }

    #[test]
    fn a_file_that_isnt_a_state_database_fails_to_open() {
        let (_dir, path) = db_path();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, vec![0x5a; 8192]).unwrap();
        let message = storage_error(Store::open(&path));
        assert!(
            message.starts_with("couldn't open the app state at"),
            "{message}"
        );
    }

    #[test]
    fn a_database_from_a_newer_build_fails_to_open() {
        let (_dir, path) = db_path();
        drop(Store::open(&path).unwrap());
        Connection::open(&path)
            .unwrap()
            .pragma_update(None, "user_version", 99)
            .unwrap();
        let message = storage_error(Store::open(&path));
        assert!(message.contains("schema version 99"), "{message}");
    }

    #[test]
    fn a_command_that_fails_to_save_takes_no_effect() {
        let (_dir, path) = db_path();
        let store = Store::open(&path).unwrap();
        open_tab(&store, "a", SessionKind::Shell);
        let before = store.state();
        let subscription = store.subscribe();
        subscription.try_next_batch(100);

        let saboteur = Connection::open(&path).unwrap();
        saboteur
            .execute_batch(
                "CREATE TRIGGER fail BEFORE INSERT ON tabs BEGIN SELECT RAISE(ABORT, 'disk full'); END;",
            )
            .unwrap();
        let result = store.dispatch(Command::OpenSession {
            title: "b".into(),
            kind: SessionKind::Shell,
            profile_path: None,
        });
        assert!(
            matches!(&result, Err(StateError::Storage { message }) if message.contains("disk full")),
            "{result:?}"
        );
        assert_eq!(store.state(), before);
        assert_eq!(subscription.try_next_batch(100), vec![]);

        saboteur.execute_batch("DROP TRIGGER fail;").unwrap();
        drop(store);
        assert_eq!(Store::open(&path).unwrap().state(), before);
    }

    /// Set to the database path when this test binary runs as the child of the `kill -9` test.
    const KILL_CHILD_DB: &str = "DEVORA_STATE_KILL_CHILD_DB";
    const KILL_CHILD_LIMIT: u64 = 100_000;

    /// The `i`th command of the `kill -9` test's script: always valid, and keeping between two and about seven tabs.
    fn scripted(i: u64, state: &AppState) -> Command {
        let tabs = &state.window.tabs;
        match i % 5 {
            _ if tabs.len() < 2 => Command::OpenSession {
                title: format!("tab {i}"),
                kind: SessionKind::Workspace {
                    path: format!("/ws/{i}"),
                },
                profile_path: Some("/profiles/work".into()),
            },
            0 => Command::OpenSession {
                title: format!("tab {i}"),
                kind: SessionKind::Shell,
                profile_path: None,
            },
            1 => Command::SetTitle {
                tab: tabs[0].id,
                title: format!("title {i}"),
            },
            2 => Command::MoveTab {
                tab: tabs[0].id,
                index: tabs.len() as u32 - 1,
            },
            3 => Command::ActivateTab {
                tab: tabs[i as usize % tabs.len()].id,
            },
            4 if tabs.len() > 6 => Command::CloseSession { tab: tabs[1].id },
            _ => Command::SetActiveProfile {
                profile_path: Some(format!("/profiles/{i}")),
            },
        }
    }

    /// The child side of the `kill -9` test: dispatches the script, reporting each command once it returns.
    #[test]
    #[ignore = "runs only as the child process of the kill -9 test"]
    fn kill_child() {
        let Ok(path) = std::env::var(KILL_CHILD_DB) else {
            return;
        };
        let store = Store::open(path).unwrap();
        let mut out = std::io::stdout();
        for i in 0..KILL_CHILD_LIMIT {
            store.dispatch(scripted(i, &store.snapshot())).unwrap();
            writeln!(out, "dispatched {}", i + 1).unwrap();
            out.flush().unwrap();
        }
    }

    /// Runs [`kill_child`] in a child process and `kill -9`s it once it reports `kill_after` dispatched commands; returns how many it had reported.
    fn run_and_kill_child(path: &Path, kill_after: u64) -> u64 {
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "persist::tests::kill_child",
                "--ignored",
                "--nocapture",
            ])
            .env(KILL_CHILD_DB, path)
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut reported = 0;
        for line in BufReader::new(child.stdout.take().unwrap()).lines() {
            if let Some(n) = line.unwrap().strip_prefix("dispatched ") {
                reported = n.parse().unwrap();
                if reported >= kill_after {
                    break;
                }
            }
        }
        child.kill().unwrap();
        child.wait().unwrap();
        reported
    }

    #[test]
    fn kill_9_mid_dispatch_keeps_a_prefix_of_the_commands() {
        for kill_after in [1, 50, 500] {
            let (_dir, path) = db_path();
            let reported = run_and_kill_child(&path, kill_after);
            assert!(reported >= kill_after, "the child stopped after {reported}");
            let restored = Store::open(&path).unwrap().state();

            // Every reported command is committed, and the child may have committed more before the kill, but never part of one.
            let mut expected = State::new();
            for i in 0.. {
                if i >= reported && expected == restored {
                    break;
                }
                assert!(
                    i < KILL_CHILD_LIMIT,
                    "the restored state isn't the state after any of commands {reported}..={KILL_CHILD_LIMIT}"
                );
                expected.apply(scripted(i, expected.snapshot())).unwrap();
            }
        }
    }
}

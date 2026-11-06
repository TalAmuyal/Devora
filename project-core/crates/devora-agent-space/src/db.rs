use crate::{CreationForm, Identity};
use devora_agent::session::Resume;
use devora_agent::transcript::{TranscriptChange, TranscriptContent, TranscriptItem};
use rusqlite::{params, Connection, OpenFlags};
use std::path::Path;
use std::time::Duration;

const SCHEMA: &str = "
CREATE TABLE space (
 id INTEGER PRIMARY KEY CHECK(id = 0),
 name TEXT NOT NULL,
 role TEXT NOT NULL,
 form TEXT NOT NULL,
 instructions TEXT NOT NULL,
 session_id TEXT NOT NULL,
 has_conversation INTEGER NOT NULL CHECK(has_conversation IN (0,1)),
 ready INTEGER NOT NULL CHECK(ready IN (0,1))
);
CREATE TABLE cli_sessions (
 session_id TEXT PRIMARY KEY,
 position INTEGER NOT NULL UNIQUE CHECK(position > 0)
);
CREATE TABLE events (
 sequence INTEGER PRIMARY KEY CHECK(sequence > 0),
 cli_session TEXT NOT NULL REFERENCES cli_sessions(session_id),
 kind TEXT NOT NULL,
 payload TEXT NOT NULL
);
PRAGMA user_version = 1;";

pub(crate) fn storage(error: impl std::fmt::Display) -> String {
    format!("agent space storage: {error}")
}

pub(crate) struct Db {
    pub conn: Connection,
}

impl Db {
    pub fn create(
        path: &Path,
        identity: &Identity,
        form: &CreationForm,
        instructions: &str,
    ) -> Result<Self, String> {
        let mut db = Self {
            conn: Connection::open(path).map_err(storage)?,
        };
        db.configure()?;
        let tx = db.conn.transaction().map_err(storage)?;
        tx.execute_batch(SCHEMA).map_err(storage)?;
        tx.execute(
            "INSERT INTO space VALUES (0,?1,?2,?3,?4,?5,0,0)",
            params![
                identity.name,
                identity.role,
                serde_json::to_string(form).map_err(storage)?,
                instructions,
                uuid::Uuid::new_v4().to_string()
            ],
        )
        .map_err(storage)?;
        tx.commit().map_err(storage)?;
        Ok(db)
    }

    pub fn open(path: &Path, readonly: bool) -> Result<Self, String> {
        if std::fs::symlink_metadata(path)
            .map_err(storage)?
            .file_type()
            .is_symlink()
        {
            return Err("agent space database cannot be a symlink".into());
        }
        let flags = if readonly {
            OpenFlags::SQLITE_OPEN_READ_ONLY
        } else {
            OpenFlags::SQLITE_OPEN_READ_WRITE
        };
        let db = Self {
            conn: Connection::open_with_flags(path, flags).map_err(storage)?,
        };
        let version: u32 = db
            .conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .map_err(storage)?;
        if version != 1 {
            return Err(format!("unsupported agent space schema version {version}"));
        }
        if !readonly {
            db.configure()?;
        }
        Ok(db)
    }

    fn configure(&self) -> Result<(), String> {
        self.conn
            .pragma_update(None, "foreign_keys", "ON")
            .map_err(storage)?;
        self.conn
            .busy_timeout(Duration::from_secs(1))
            .map_err(storage)?;
        self.conn
            .pragma_update(None, "journal_mode", "WAL")
            .map_err(storage)?;
        self.conn
            .pragma_update(None, "synchronous", "NORMAL")
            .map_err(storage)
    }

    pub fn load(&self) -> Result<(Identity, CreationForm, String, Resume), String> {
        let (identity, form, instructions, resume, ready) = self.conn.query_row(
            "SELECT name,role,form,instructions,session_id,has_conversation,ready FROM space WHERE id=0",
            [],
            |row| {
                Ok((
                    Identity { name: row.get(0)?, role: row.get(1)? },
                    row.get::<_,String>(2)?,
                    row.get::<_,String>(3)?,
                    Resume { session_id: row.get(4)?, has_conversation: row.get(5)? },
                    row.get::<_,bool>(6)?,
                ))
            },
        ).map_err(storage)?;
        if !ready {
            return Err("agent space creation is incomplete".into());
        }
        uuid::Uuid::parse_str(&resume.session_id).map_err(storage)?;
        let form: CreationForm = serde_json::from_str(&form).map_err(storage)?;
        crate::validate_name(&identity.name)?;
        if form.name != identity.name || identity.role.trim().is_empty() {
            return Err("agent space identity is invalid".into());
        }
        Ok((identity, form, instructions, resume))
    }

    pub fn finish_creation(&mut self) -> Result<(), String> {
        let tx = self.conn.transaction().map_err(storage)?;
        tx.execute("UPDATE space SET ready=1 WHERE id=0", [])
            .map_err(storage)?;
        tx.commit().map_err(storage)
    }

    /// Lifecycle identity and whole transcript items share the same transaction.
    /// Token append changes deliberately produce no database writes.
    pub fn save_changes(
        &mut self,
        resume: Option<&Resume>,
        changes: &[TranscriptChange],
    ) -> Result<(), String> {
        let whole: Vec<_> = changes
            .iter()
            .filter_map(|change| match change {
                TranscriptChange::Upsert { item } => Some(item),
                _ => None,
            })
            .collect();
        if resume.is_none() && whole.is_empty() {
            return Ok(());
        }
        let tx = self.conn.transaction().map_err(storage)?;
        if let Some(resume) = resume {
            let changed = tx.execute(
                "UPDATE space SET has_conversation=?1, session_id=?2 WHERE id=0 AND (session_id=?2 OR has_conversation=0) AND (has_conversation=0 OR ?1=1)",
                params![resume.has_conversation, resume.session_id],
            ).map_err(storage)?;
            if changed != 1 {
                return Err("agent space session identity changed".into());
            }
            register_session(&tx, &resume.session_id)?;
        }
        for item in whole {
            register_session(&tx, &item.cli_session)?;
            let kind = match item.content {
                TranscriptContent::User { .. } => "user",
                TranscriptContent::Assistant { .. } => "assistant",
                TranscriptContent::Thinking { .. } => "thinking",
                TranscriptContent::Tool { .. } => "tool",
                TranscriptContent::System { .. } => "system",
                TranscriptContent::Unknown { .. } => "unknown",
            };
            let changed = tx.execute("INSERT INTO events(sequence,cli_session,kind,payload) VALUES(?1,?2,?3,?4) ON CONFLICT(sequence) DO UPDATE SET kind=excluded.kind,payload=excluded.payload WHERE events.cli_session=excluded.cli_session",
                params![item.sequence, item.cli_session, kind, serde_json::to_string(item).map_err(storage)?]).map_err(storage)?;
            if changed != 1 {
                return Err("transcript sequence belongs to another CLI session".into());
            }
        }
        tx.commit().map_err(storage)
    }

    pub fn history(&self, after: u64, limit: u32) -> Result<Vec<TranscriptItem>, String> {
        let mut query = self.conn.prepare(
            "SELECT sequence,cli_session,payload FROM events WHERE sequence > ?1 ORDER BY sequence LIMIT ?2",
        ).map_err(storage)?;
        let rows = query
            .query_map(params![after, limit], |row| {
                Ok((
                    row.get::<_, u64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(storage)?;
        rows.map(|row| {
            let (sequence, session, payload) = row.map_err(storage)?;
            let item: TranscriptItem = serde_json::from_str(&payload).map_err(storage)?;
            if item.sequence != sequence || item.cli_session != session {
                return Err("transcript payload identity disagrees with its row".into());
            }
            Ok(item)
        })
        .collect()
    }

    pub fn all_history(&self) -> Result<Vec<TranscriptItem>, String> {
        self.history(0, u32::MAX)
    }
}

fn register_session(tx: &rusqlite::Transaction<'_>, id: &str) -> Result<(), String> {
    tx.execute("INSERT OR IGNORE INTO cli_sessions(session_id,position) SELECT ?1, COALESCE(MAX(position),0)+1 FROM cli_sessions", [id]).map_err(storage)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use devora_agent::transcript::{ItemState, SystemKind};
    use std::collections::BTreeMap;

    #[test]
    fn lifecycle_and_whole_history_commit_atomically_and_deltas_never_write() {
        let root = tempfile::tempdir().unwrap();
        let mut db = Db::create(
            &root.path().join("space.db"),
            &Identity {
                name: "Delta".into(),
                role: "Developer".into(),
            },
            &CreationForm {
                name: "Delta".into(),
                role: None,
                repos: vec![],
                inputs: BTreeMap::new(),
            },
            "instructions",
        )
        .unwrap();
        db.finish_creation().unwrap();
        let before = db.load().unwrap().3;
        let id = uuid::Uuid::new_v4().to_string();
        let item = TranscriptItem {
            sequence: 1,
            cli_session: id.clone(),
            parent_tool_use_id: None,
            state: ItemState::Complete,
            content: TranscriptContent::System {
                kind: SystemKind::Result,
                text: "Turn completed".into(),
                detail_json: "{}".into(),
            },
        };
        let change = TranscriptChange::upsert(item.clone());
        let resume = Resume {
            session_id: id.clone(),
            has_conversation: true,
        };
        db.conn.execute_batch("CREATE TRIGGER fail_events BEFORE INSERT ON events BEGIN SELECT RAISE(ABORT,'injected history failure'); END;").unwrap();
        assert!(db
            .save_changes(Some(&resume), std::slice::from_ref(&change))
            .is_err());
        assert_eq!(db.load().unwrap().3.session_id, before.session_id);
        assert!(!db.load().unwrap().3.has_conversation);
        assert!(db.all_history().unwrap().is_empty());
        assert_eq!(
            db.conn
                .query_row("SELECT count(*) FROM cli_sessions", [], |row| row
                    .get::<_, u64>(0))
                .unwrap(),
            0
        );
        db.conn.execute_batch("DROP TRIGGER fail_events").unwrap();
        db.save_changes(Some(&resume), &[change]).unwrap();
        assert!(db.load().unwrap().3.has_conversation);
        assert_eq!(db.all_history().unwrap(), [item]);
        let writes = db.conn.total_changes();
        db.save_changes(
            None,
            &[TranscriptChange::AppendText {
                sequence: 1,
                text: "transient".into(),
            }],
        )
        .unwrap();
        assert_eq!(db.conn.total_changes(), writes);
        for sequence in 2..=4 {
            db.save_changes(
                None,
                &[TranscriptChange::upsert(TranscriptItem {
                    sequence,
                    cli_session: id.clone(),
                    parent_tool_use_id: None,
                    state: ItemState::Complete,
                    content: TranscriptContent::Assistant {
                        text: sequence.to_string(),
                    },
                })],
            )
            .unwrap();
        }
        assert_eq!(
            db.history(1, 2)
                .unwrap()
                .iter()
                .map(|i| i.sequence)
                .collect::<Vec<_>>(),
            [2, 3]
        );
        let other = TranscriptItem {
            sequence: 1,
            cli_session: uuid::Uuid::new_v4().to_string(),
            parent_tool_use_id: None,
            state: ItemState::Complete,
            content: TranscriptContent::Assistant {
                text: "wrong session".into(),
            },
        };
        assert!(db
            .save_changes(None, &[TranscriptChange::upsert(other)])
            .is_err());
        assert_eq!(db.all_history().unwrap().len(), 4);
    }
}

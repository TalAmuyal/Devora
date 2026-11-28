use crate::restart_notice::RestartNotice;
use crate::{CreationForm, Identity};
use devora_agent::session::{CliProcess, Resume};
use devora_agent::transcript::{Delivery, TranscriptChange, TranscriptContent, TranscriptItem};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
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
PRAGMA user_version = 1;";

const HISTORY_SCHEMA: &str = "
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
";

// Version 3: the restart notice (`crate::restart_notice`).
const RESTART_SCHEMA: &str = "
ALTER TABLE space ADD COLUMN launched_run TEXT;
ALTER TABLE space ADD COLUMN restart_notice TEXT;
";
// Version 4: the running CLI's identity, for stopping one an earlier Devora left behind.
const CLI_SCHEMA: &str = "
ALTER TABLE space ADD COLUMN cli_process TEXT;
";
// Version 5: the agent's own compaction threshold, over the profile's (`crate::compaction`).
const THRESHOLD_SCHEMA: &str = "
ALTER TABLE space ADD COLUMN compact_threshold INTEGER CHECK(compact_threshold BETWEEN 1 AND 100);
";
// Version 6: when each transcript item was first stored, in Unix milliseconds; null for items stored before.
const CREATED_AT_SCHEMA: &str = "
ALTER TABLE events ADD COLUMN created_at INTEGER;
";
const VERSION: u32 = 6;

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
        tx.execute_batch(HISTORY_SCHEMA).map_err(storage)?;
        tx.execute_batch(RESTART_SCHEMA).map_err(storage)?;
        tx.execute_batch(CLI_SCHEMA).map_err(storage)?;
        tx.execute_batch(THRESHOLD_SCHEMA).map_err(storage)?;
        tx.execute_batch(CREATED_AT_SCHEMA).map_err(storage)?;
        tx.pragma_update(None, "user_version", VERSION)
            .map_err(storage)?;
        tx.execute(
            "INSERT INTO space(id,name,role,form,instructions,session_id,has_conversation,ready) VALUES (0,?1,?2,?3,?4,?5,0,0)",
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
        let mut db = Self {
            conn: Connection::open_with_flags(path, flags).map_err(storage)?,
        };
        let version: u32 = db
            .conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .map_err(storage)?;
        if !(1..=VERSION).contains(&version) {
            return Err(format!("unsupported agent space schema version {version}"));
        }
        if !readonly {
            db.configure()?;
            if version < VERSION {
                db.migrate()?;
            }
        }
        Ok(db)
    }

    fn migrate(&mut self) -> Result<(), String> {
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(storage)?;
        let version: u32 = tx
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(storage)?;
        if version == 1 {
            tx.execute_batch(HISTORY_SCHEMA).map_err(storage)?;
            let id: String = tx
                .query_row("SELECT session_id FROM space WHERE id=0", [], |row| {
                    row.get(0)
                })
                .map_err(storage)?;
            register_session(&tx, &id)?;
        } else if !(2..=VERSION).contains(&version) {
            return Err(format!("unsupported agent space schema version {version}"));
        }
        if version < 3 {
            tx.execute_batch(RESTART_SCHEMA).map_err(storage)?;
        }
        if version < 4 {
            tx.execute_batch(CLI_SCHEMA).map_err(storage)?;
        }
        if version < 5 {
            tx.execute_batch(THRESHOLD_SCHEMA).map_err(storage)?;
        }
        if version < 6 {
            tx.execute_batch(CREATED_AT_SCHEMA).map_err(storage)?;
        }
        tx.pragma_update(None, "user_version", VERSION)
            .map_err(storage)?;
        tx.commit().map_err(storage)
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

    /// Lifecycle identity and whole transcript items share the same transaction, and each whole item gets the time it
    /// was first stored (`save_items`). Token append changes deliberately produce no database writes.
    pub fn save_changes(
        &mut self,
        resume: Option<&Resume>,
        changes: &mut [TranscriptChange],
    ) -> Result<(), String> {
        let mut whole: Vec<_> = changes
            .iter_mut()
            .filter_map(|change| match change {
                TranscriptChange::Upsert { item } => Some(&mut **item),
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
        save_items(&tx, &mut whole)?;
        tx.commit().map_err(storage)
    }

    /// The run that last launched this agent's CLI, and the restart notice it
    /// is owed, if any.
    pub fn launch_state(&self) -> Result<(Option<String>, Option<RestartNotice>), String> {
        let (run, notice) = self
            .conn
            .query_row(
                "SELECT launched_run,restart_notice FROM space WHERE id=0",
                [],
                |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, Option<String>>(1)?,
                    ))
                },
            )
            .map_err(storage)?;
        let notice = notice
            .map(|notice| serde_json::from_str(&notice))
            .transpose()
            .map_err(storage)?;
        Ok((run, notice))
    }

    /// The CLI last recorded running for this agent.
    pub fn cli_process(&self) -> Result<Option<CliProcess>, String> {
        let cli: Option<String> = self
            .conn
            .query_row("SELECT cli_process FROM space WHERE id=0", [], |row| {
                row.get(0)
            })
            .map_err(storage)?;
        cli.map(|cli| serde_json::from_str(&cli))
            .transpose()
            .map_err(storage)
    }

    /// The agent's own compaction threshold, when it has one.
    pub fn compact_threshold(&self) -> Result<Option<u8>, String> {
        self.conn
            .query_row(
                "SELECT compact_threshold FROM space WHERE id=0",
                [],
                |row| row.get(0),
            )
            .map_err(storage)
    }

    /// Sets the agent's own compaction threshold, or clears it to follow the profile's.
    pub fn set_compact_threshold(&mut self, percent: Option<u8>) -> Result<(), String> {
        self.conn
            .execute(
                "UPDATE space SET compact_threshold=?1 WHERE id=0",
                [percent],
            )
            .map_err(storage)?;
        Ok(())
    }

    /// Stores the model and effort the agent runs from its next launch on.
    pub fn set_model(&mut self, choice: &crate::ModelChoice) -> Result<(), String> {
        let form: String = self
            .conn
            .query_row("SELECT form FROM space WHERE id=0", [], |row| row.get(0))
            .map_err(storage)?;
        let mut form: CreationForm = serde_json::from_str(&form).map_err(storage)?;
        form.model = Some(choice.clone());
        self.conn
            .execute(
                "UPDATE space SET form=?1 WHERE id=0",
                [serde_json::to_string(&form).map_err(storage)?],
            )
            .map_err(storage)?;
        Ok(())
    }

    /// Records the CLI running for this agent, or that none is.
    pub fn record_cli(&mut self, cli: Option<&CliProcess>) -> Result<(), String> {
        let cli = cli
            .map(serde_json::to_string)
            .transpose()
            .map_err(storage)?;
        self.conn
            .execute("UPDATE space SET cli_process=?1 WHERE id=0", [cli])
            .map_err(storage)?;
        Ok(())
    }

    /// A launch in `run`, with what it owes, in one write.
    pub fn record_launch(
        &mut self,
        run: &str,
        notice: Option<&RestartNotice>,
    ) -> Result<(), String> {
        let notice = notice
            .map(serde_json::to_string)
            .transpose()
            .map_err(storage)?;
        self.conn
            .execute(
                "UPDATE space SET launched_run=?1, restart_notice=?2 WHERE id=0",
                params![run, notice],
            )
            .map_err(storage)?;
        Ok(())
    }

    /// The notice's transcript row and the attempt it is sent as, committed
    /// together before the send.
    pub fn save_notice_attempt(
        &mut self,
        change: &mut TranscriptChange,
        notice: &RestartNotice,
    ) -> Result<(), String> {
        let TranscriptChange::Upsert { item } = change else {
            return Err("a restart notice must be a whole item".into());
        };
        let tx = self.conn.transaction().map_err(storage)?;
        save_items(&tx, &mut [&mut **item])?;
        tx.execute(
            "UPDATE space SET restart_notice=?1 WHERE id=0",
            [serde_json::to_string(notice).map_err(storage)?],
        )
        .map_err(storage)?;
        tx.commit().map_err(storage)
    }

    /// Owes `notice`, merged into one already owed (`RestartNotice::merge`).
    pub fn owe_notice(&mut self, notice: RestartNotice) -> Result<(), String> {
        let tx = self.conn.transaction().map_err(storage)?;
        let stored: Option<String> = tx
            .query_row("SELECT restart_notice FROM space WHERE id=0", [], |row| {
                row.get(0)
            })
            .map_err(storage)?;
        let owed = match stored {
            Some(stored) => serde_json::from_str::<RestartNotice>(&stored)
                .map_err(storage)?
                .merge(notice),
            None => notice,
        };
        tx.execute(
            "UPDATE space SET restart_notice=?1 WHERE id=0",
            [serde_json::to_string(&owed).map_err(storage)?],
        )
        .map_err(storage)?;
        tx.commit().map_err(storage)
    }

    /// The CLI answered the notice sent as `attempt`.
    pub fn notice_delivered(&mut self, attempt: &str) -> Result<(), String> {
        self.conn
            .execute(
                "UPDATE space SET restart_notice=NULL WHERE id=0 AND json_extract(restart_notice,'$.attempt')=?1",
                [attempt],
            )
            .map_err(storage)?;
        Ok(())
    }

    /// Sets a stored user row's delivery, and returns the row as stored now; none for a row that isn't stored, or
    /// isn't a user's.
    pub fn set_delivery(
        &mut self,
        sequence: u64,
        value: Option<Delivery>,
    ) -> Result<Option<TranscriptItem>, String> {
        let Some(mut item) = self
            .history(sequence.saturating_sub(1), 1)?
            .into_iter()
            .find(|item| item.sequence == sequence)
        else {
            return Ok(None);
        };
        let TranscriptContent::User { delivery, .. } = &mut item.content else {
            return Ok(None);
        };
        *delivery = value;
        let tx = self.conn.transaction().map_err(storage)?;
        save_items(&tx, &mut [&mut item])?;
        tx.commit().map_err(storage)?;
        Ok(Some(item))
    }

    /// The user rows still waiting for the agent, by sequence.
    pub fn waiting(&self) -> Result<Vec<u64>, String> {
        let mut query = self
            .conn
            .prepare(
                "SELECT sequence FROM events WHERE kind='user'
                 AND json_extract(payload,'$.content.User.delivery')='Waiting' ORDER BY sequence",
            )
            .map_err(storage)?;
        let rows = query
            .query_map([], |row| row.get::<_, u64>(0))
            .map_err(storage)?;
        rows.map(|row| row.map_err(storage)).collect()
    }

    pub fn history(&self, after: u64, limit: u32) -> Result<Vec<TranscriptItem>, String> {
        self.history_query(
            "WHERE sequence > ?1 ORDER BY sequence LIMIT ?2",
            Some(after),
            limit,
        )
    }

    /// Exclusive backward cursor. None selects the newest page. Returned items
    /// are chronological, so callers can prepend each page without reversing it.
    pub fn history_before(
        &self,
        before: Option<u64>,
        limit: u32,
    ) -> Result<Vec<TranscriptItem>, String> {
        let upper = before
            .map(|sequence| sequence.saturating_sub(1))
            .unwrap_or(i64::MAX as u64);
        let mut items = self.history_query(
            "WHERE sequence <= ?1 ORDER BY sequence DESC LIMIT ?2",
            Some(upper),
            limit,
        )?;
        items.reverse();
        Ok(items)
    }

    /// The items `filter` selects (its `WHERE`, `ORDER BY` and `LIMIT`).
    fn history_query(
        &self,
        filter: &str,
        cursor: Option<u64>,
        limit: u32,
    ) -> Result<Vec<TranscriptItem>, String> {
        let version: u32 = self
            .conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(storage)?;
        if version == 1 {
            return Ok(vec![]);
        }
        // A read-only read leaves an older space unmigrated, and its items have no time.
        let created_at = if version < 6 { "NULL" } else { "created_at" };
        let mut query = self
            .conn
            .prepare(&format!(
                "SELECT sequence,cli_session,payload,{created_at} FROM events {filter}"
            ))
            .map_err(storage)?;
        let rows = query
            .query_map(params![cursor, limit], |row| {
                Ok((
                    row.get::<_, u64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<u64>>(3)?,
                ))
            })
            .map_err(storage)?;
        rows.map(|row| {
            let (sequence, session, payload, created_at) = row.map_err(storage)?;
            let mut item = stored_item(sequence, &session, &payload)?;
            item.created_at_ms = created_at;
            Ok(item)
        })
        .collect()
    }

    pub fn fold(&self, session: &str) -> Result<devora_agent::transcript::Transcript, String> {
        let next = self
            .conn
            .query_row(
                "SELECT COALESCE(MAX(sequence),0)+1 FROM events",
                [],
                |row| row.get::<_, u64>(0),
            )
            .map_err(storage)?;
        let (has_prompt, marked) = self
            .conn
            .query_row(
                "SELECT
               EXISTS(SELECT 1 FROM events WHERE cli_session=?1 AND kind='user'),
               EXISTS(SELECT 1 FROM events WHERE cli_session=?1
                 AND json_extract(payload,'$.content.System.kind')='UnfinishedFirstPrompt')",
                [session],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(storage)?;
        let mut query = self
            .conn
            .prepare(
                "SELECT sequence,cli_session,payload FROM events
             WHERE cli_session=?1 AND (
               json_extract(payload,'$.state') IN ('Pending','Streaming') OR (
                 kind='tool' AND json_extract(payload,'$.state')='Complete'
                 AND json_extract(payload,'$.content.Tool.result') IS NULL))
             ORDER BY sequence",
            )
            .map_err(storage)?;
        let rows = query
            .query_map([session], |row| {
                Ok((
                    row.get::<_, u64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(storage)?;
        let items = rows
            .map(|row| {
                let (sequence, session, payload) = row.map_err(storage)?;
                stored_item(sequence, &session, &payload)
            })
            .collect::<Result<Vec<_>, String>>()?;
        // The latest reply in the agent's own conversation, from any of its CLI sessions: the model it continues on.
        let latest_model: Option<String> = self
            .conn
            .query_row(
                "SELECT json_extract(payload,'$.content.Assistant.model') FROM events
             WHERE kind='assistant' AND json_extract(payload,'$.parent_tool_use_id') IS NULL
               AND json_extract(payload,'$.content.Assistant.model') IS NOT NULL
             ORDER BY sequence DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(storage)?;
        Ok(devora_agent::transcript::Transcript::restore(
            items,
            session.into(),
            next,
            has_prompt,
            marked,
            latest_model,
        ))
    }
}

fn stored_item(sequence: u64, session: &str, payload: &str) -> Result<TranscriptItem, String> {
    let item: TranscriptItem = serde_json::from_str(payload).map_err(storage)?;
    if item.sequence != sequence || item.cli_session != session {
        return Err("transcript payload identity disagrees with its row".into());
    }
    Ok(item)
}

/// Whole transcript items, each in its CLI session, in the caller's transaction. An item's first store stamps it with
/// the time, which its later updates keep, and each item gets its stored time back.
fn save_items(
    tx: &rusqlite::Transaction<'_>,
    whole: &mut [&mut TranscriptItem],
) -> Result<(), String> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(storage)?
        .as_millis() as u64;
    for item in whole {
        register_session(tx, &item.cli_session)?;
        let kind = match item.content {
            TranscriptContent::User { .. } => "user",
            TranscriptContent::Assistant { .. } => "assistant",
            TranscriptContent::Thinking { .. } => "thinking",
            TranscriptContent::Tool { .. } => "tool",
            TranscriptContent::System { .. } => "system",
            TranscriptContent::Unknown { .. } => "unknown",
        };
        let stored = tx.query_row("INSERT INTO events(sequence,cli_session,kind,payload,created_at) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(sequence) DO UPDATE SET kind=excluded.kind,payload=excluded.payload WHERE events.cli_session=excluded.cli_session RETURNING created_at",
            params![item.sequence, item.cli_session, kind, serde_json::to_string(&**item).map_err(storage)?, now], |row| row.get::<_, Option<u64>>(0)).optional().map_err(storage)?;
        let Some(created_at) = stored else {
            return Err("transcript sequence belongs to another CLI session".into());
        };
        item.created_at_ms = created_at;
    }
    Ok(())
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
    fn a_changed_model_is_stored_with_the_agent() {
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
                keep_coding_instructions: true,
                model: Some(crate::ModelChoice {
                    model: "opus".into(),
                    effort: Some("high".into()),
                }),
                command: Some("claude-gateway".into()),
            },
            "instructions",
        )
        .unwrap();
        db.finish_creation().unwrap();
        let choice = crate::ModelChoice {
            model: "sonnet".into(),
            effort: Some("high".into()),
        };
        db.set_model(&choice).unwrap();
        let form = db.load().unwrap().1;
        assert_eq!(form.model, Some(choice));
        assert_eq!(form.command.as_deref(), Some("claude-gateway"));
    }

    fn created(path: &Path) -> Db {
        let mut db = Db::create(
            path,
            &Identity {
                name: "Delta".into(),
                role: "Developer".into(),
            },
            &CreationForm {
                name: "Delta".into(),
                role: None,
                repos: vec![],
                inputs: BTreeMap::new(),
                keep_coding_instructions: true,
                model: None,
                command: None,
            },
            "instructions",
        )
        .unwrap();
        db.finish_creation().unwrap();
        db
    }

    fn reply(sequence: u64, session: &str, state: ItemState, text: &str) -> TranscriptChange {
        TranscriptChange::upsert(TranscriptItem {
            sequence,
            cli_session: session.into(),
            parent_tool_use_id: None,
            state,
            content: TranscriptContent::Assistant {
                text: text.into(),
                model: None,
            },
            created_at_ms: None,
        })
    }

    fn created_at(change: &TranscriptChange) -> Option<u64> {
        let TranscriptChange::Upsert { item } = change else {
            panic!("a whole item")
        };
        item.created_at_ms
    }

    #[test]
    fn an_item_keeps_the_time_it_was_first_stored_and_items_from_before_version_6_have_none() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("space.db");
        let session = uuid::Uuid::new_v4().to_string();
        let mut db = created(&path);
        db.save_changes(None, &mut [reply(1, &session, ItemState::Streaming, "old")])
            .unwrap();
        db.conn
            .execute_batch("ALTER TABLE events DROP COLUMN created_at; PRAGMA user_version = 5;")
            .unwrap();
        drop(db);
        let unmigrated = Db::open(&path, true).unwrap();
        assert_eq!(unmigrated.history(0, 500).unwrap()[0].created_at_ms, None);
        drop(unmigrated);

        let mut db = Db::open(&path, false).unwrap();
        assert_eq!(
            db.conn
                .pragma_query_value::<u32, _>(None, "user_version", |row| row.get(0))
                .unwrap(),
            6
        );
        assert_eq!(db.history(0, 500).unwrap()[0].created_at_ms, None);
        let mut old = [reply(1, &session, ItemState::Complete, "old, completed")];
        db.save_changes(None, &mut old).unwrap();
        assert_eq!(
            created_at(&old[0]),
            None,
            "an update doesn't date an old item"
        );

        let before = now_ms();
        let mut new = [reply(2, &session, ItemState::Streaming, "new")];
        db.save_changes(None, &mut new).unwrap();
        let stamped = created_at(&new[0]).expect("a new item is stamped");
        assert!((before..=now_ms()).contains(&stamped));
        std::thread::sleep(Duration::from_millis(5));
        let mut completed = [reply(2, &session, ItemState::Complete, "new, completed")];
        db.save_changes(None, &mut completed).unwrap();
        assert_eq!(created_at(&completed[0]), Some(stamped));

        let history = db.history(0, 500).unwrap();
        assert_eq!(
            history
                .iter()
                .map(|item| item.created_at_ms)
                .collect::<Vec<_>>(),
            [None, Some(stamped)]
        );
        assert_eq!(db.history_before(None, 500).unwrap(), history);
        let payload: String = db
            .conn
            .query_row("SELECT payload FROM events WHERE sequence=2", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert!(
            !payload.contains("created_at"),
            "the column alone keeps the time"
        );
    }

    fn now_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }

    #[test]
    fn lifecycle_and_whole_history_commit_atomically_and_deltas_never_write() {
        let root = tempfile::tempdir().unwrap();
        let mut db = created(&root.path().join("space.db"));
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
            created_at_ms: None,
        };
        let mut change = TranscriptChange::upsert(item.clone());
        let resume = Resume {
            session_id: id.clone(),
            has_conversation: true,
        };
        db.conn.execute_batch("CREATE TRIGGER fail_events BEFORE INSERT ON events BEGIN SELECT RAISE(ABORT,'injected history failure'); END;").unwrap();
        assert!(db
            .save_changes(Some(&resume), std::slice::from_mut(&mut change.clone()))
            .is_err());
        assert_eq!(db.load().unwrap().3.session_id, before.session_id);
        assert!(!db.load().unwrap().3.has_conversation);
        assert!(db.history(0, 500).unwrap().is_empty());
        assert_eq!(
            db.conn
                .query_row("SELECT count(*) FROM cli_sessions", [], |row| row
                    .get::<_, u64>(0))
                .unwrap(),
            0
        );
        db.conn.execute_batch("DROP TRIGGER fail_events").unwrap();
        db.save_changes(Some(&resume), std::slice::from_mut(&mut change))
            .unwrap();
        assert!(db.load().unwrap().3.has_conversation);
        let TranscriptChange::Upsert { item: saved } = &change else {
            panic!()
        };
        assert!(saved.created_at_ms.is_some());
        assert_eq!(db.history(0, 500).unwrap(), [*saved.clone()]);
        let writes = db.conn.total_changes();
        db.save_changes(
            None,
            &mut [TranscriptChange::AppendText {
                sequence: 1,
                text: "transient".into(),
            }],
        )
        .unwrap();
        assert_eq!(db.conn.total_changes(), writes);
        for sequence in 2..=4 {
            db.save_changes(
                None,
                &mut [TranscriptChange::upsert(TranscriptItem {
                    sequence,
                    cli_session: id.clone(),
                    parent_tool_use_id: None,
                    state: ItemState::Complete,
                    content: TranscriptContent::Assistant {
                        text: sequence.to_string(),
                        model: None,
                    },
                    created_at_ms: None,
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
                model: None,
            },
            created_at_ms: None,
        };
        assert!(db
            .save_changes(None, &mut [TranscriptChange::upsert(other)])
            .is_err());
        assert_eq!(db.history(0, 500).unwrap().len(), 4);
        let mut fold = db.fold(&id).unwrap();
        assert!(
            fold.items().is_empty(),
            "completed history must not seed the fold"
        );
        let change = fold.submit("pending".into());
        let TranscriptChange::Upsert { item } = &change else {
            panic!()
        };
        assert_eq!(item.sequence, 5);
        db.save_changes(None, &mut [change]).unwrap();
        let seeded = db.fold(&id).unwrap();
        assert_eq!(seeded.items().len(), 1);
        assert_eq!(seeded.items()[0].sequence, 5);
    }
}

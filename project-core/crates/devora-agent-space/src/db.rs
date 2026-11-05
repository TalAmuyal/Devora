use crate::{CreationForm, Identity};
use devora_agent::session::Resume;
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

    pub fn save_session(&mut self, resume: &Resume) -> Result<(), String> {
        let tx = self.conn.transaction().map_err(storage)?;
        let changed = tx
            .execute(
                "UPDATE space SET has_conversation=?1, session_id=?2 WHERE id=0 AND (session_id=?2 OR has_conversation=0) AND (has_conversation=0 OR ?1=1)",
                params![resume.has_conversation, resume.session_id],
            )
            .map_err(storage)?;
        if changed != 1 {
            return Err("agent space session identity changed".into());
        }
        tx.commit().map_err(storage)
    }
}

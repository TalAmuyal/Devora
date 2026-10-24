//! The on-disk side of Judgess: JSONL logs under `~/.claude/judgess/`. Nothing in `decide` touches this.

use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

pub const AUDIT: &str = "audit.jsonl";
pub const UNKNOWN: &str = "unknown.jsonl";
pub const LABELLED: &str = "labelled.jsonl";
const KEY: &str = "redaction.key";
const KEY_LEN: usize = 32;

pub struct Store {
    dir: PathBuf,
}

impl Store {
    pub fn at(dir: impl Into<PathBuf>) -> Store {
        Store { dir: dir.into() }
    }

    pub fn in_home(home: &str) -> Store {
        Store::at(Path::new(home).join(".claude").join("judgess"))
    }

    pub fn path(&self, file: &str) -> PathBuf {
        self.dir.join(file)
    }

    /// One `write` on an `O_APPEND` descriptor, so concurrent hooks never interleave lines.
    pub fn append(&self, file: &str, entry: &Value) -> io::Result<()> {
        fs::create_dir_all(&self.dir)?;
        let mut line = entry.to_string();
        line.push('\n');
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.path(file))?
            .write_all(line.as_bytes())
    }

    /// Unreadable files and malformed lines read as absent.
    pub fn read(&self, file: &str) -> Vec<Value> {
        let text = fs::read_to_string(self.path(file)).unwrap_or_default();
        text.lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect()
    }

    pub fn replace(&self, file: &str, entries: &[Value]) -> io::Result<()> {
        let temporary = self.path(&format!("{file}.tmp"));
        let text: String = entries.iter().map(|entry| format!("{entry}\n")).collect();
        fs::write(&temporary, text)?;
        fs::rename(temporary, self.path(file))
    }

    /// Generated on first use; keeps hashes stable on this machine without being brute-forceable elsewhere.
    pub fn redaction_key(&self) -> io::Result<Vec<u8>> {
        fs::create_dir_all(&self.dir)?;
        let path = self.path(KEY);
        match fs::read(&path) {
            Ok(key) => return valid_key(key, &path),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        let mut key = vec![0u8; KEY_LEN];
        fs::File::open("/dev/urandom")?.read_exact(&mut key)?;
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
        {
            Ok(mut file) => file.write_all(&key)?,
            // Lost a race with another process; use the key it wrote.
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                return valid_key(fs::read(&path)?, &path);
            }
            Err(error) => return Err(error),
        }
        Ok(key)
    }
}

/// A truncated key (a crash mid-write) would silently weaken every hash, so it is an error, not a key.
fn valid_key(key: Vec<u8>, path: &Path) -> io::Result<Vec<u8>> {
    if key.len() == KEY_LEN {
        Ok(key)
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{} is corrupt; delete it to generate a new key",
                path.display()
            ),
        ))
    }
}

/// RFC 3339 UTC with microseconds.
pub fn timestamp() -> String {
    let since_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let seconds = since_epoch.as_secs() as i64;
    let (days, second_of_day) = (seconds.div_euclid(86_400), seconds.rem_euclid(86_400));
    // Civil-from-days (Howard Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:06}Z",
        second_of_day / 3_600,
        second_of_day % 3_600 / 60,
        second_of_day % 60,
        since_epoch.subsec_micros()
    )
}

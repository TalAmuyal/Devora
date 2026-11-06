//! Agent Space directories and immutable creation-time instructions (ADR-012).
//! Each open space holds a plain OS lock until its session has stopped. Incomplete
//! creations remain visible as invalid entries after a process crash. Ordinary
//! failures synchronously deregister worktrees and clean up the reserved path.
//! The blocking catalogue lock covers short reservation/removal sections. Listing
//! is read-only, and each space retains its own fail-fast ownership lock while creating or open.
mod db;
mod inputs;
mod running;
pub mod template;
pub use inputs::{fields, Field, Source};
pub use running::{RunningSpace, SpaceEvent, SpaceHandle};

use db::{storage, Db};
use devora_agent::session::{Instructions, Plugin, Resume, SessionOptions};
use devora_app::log::Log;
use devora_config::config;
use devora_workspace::{
    creation::{self, CreationHandle, EventSink},
    workspace,
};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const DATABASE: &str = ".agent-space.db";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Identity {
    pub name: String,
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreationForm {
    pub name: String,
    pub role: Option<String>,
    pub repos: Vec<String>,
    pub inputs: BTreeMap<String, String>,
}

/// Host-owned executable, selected plugins, behavioral settings and scrubbed
/// transport environment. Configured tool env belongs in settings.env. Credentials
/// stay in environment. The containing profile supplies model, effort and alias values.
#[derive(Clone)]
pub struct LaunchContext {
    pub global_config: PathBuf,
    pub program: PathBuf,
    pub plugins: Vec<Plugin>,
    pub settings: Value,
    pub environment: BTreeMap<String, String>,
}

#[derive(Debug)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
    pub identity: Option<Identity>,
    pub error: Option<String>,
}

pub struct Space {
    path: PathBuf,
    profile: PathBuf,
    db: Db,
    _ownership: Lock,
    identity: Identity,
    form: CreationForm,
    instructions: String,
    resume: Resume,
}

pub fn validate_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name.trim() != name
        || name.starts_with('.')
        || name
            .chars()
            .any(|c| c.is_control() || c == '/' || c == '\\')
    {
        return Err(format!("invalid agent space name: {name:?}"));
    }
    Ok(())
}

struct Lock(File);

impl Drop for Lock {
    fn drop(&mut self) {
        // Explicit unlock also releases a descriptor briefly inherited by a
        // concurrent child spawn before close-on-exec takes effect.
        let _ = FileExt::unlock(&self.0);
    }
}

fn open_lock_file(path: &Path) -> Result<File, String> {
    if fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(format!("lock cannot be a symlink: {}", path.display()));
    }
    OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)
        .map_err(storage)
}

fn lock(path: &Path) -> Result<Lock, String> {
    let file = open_lock_file(path)?;
    file.try_lock_exclusive()
        .map_err(|e| format!("agent space is in use: {e}"))?;
    Ok(Lock(file))
}

fn catalogue(profile: &Path) -> Result<PathBuf, String> {
    fs::canonicalize(profile.join("agent-spaces")).map_err(storage)
}

fn catalogue_lock(root: &Path) -> Result<Lock, String> {
    let file = open_lock_file(&root.join(".lock"))?;
    file.lock_exclusive().map_err(storage)?;
    Ok(Lock(file))
}

fn reserve(root: &Path, path: &Path, name: &str) -> Result<Lock, String> {
    let _catalogue = catalogue_lock(root)?;
    match fs::create_dir(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(format!("agent space '{name}' already exists"));
        }
        Err(error) => return Err(storage(error)),
    }
    match lock(&path.join(".agent-space.lock")) {
        Ok(ownership) => Ok(ownership),
        Err(error) => {
            if let Err(cleanup) = fs::remove_dir_all(path) {
                return Err(format!("{error}; cleanup failed: {cleanup}"));
            }
            Err(error)
        }
    }
}

fn checked_path(root: &Path, name: &str) -> Result<PathBuf, String> {
    validate_name(name)?;
    let path = root.join(name);
    let metadata = fs::symlink_metadata(&path).map_err(storage)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("agent space path must be a real directory".into());
    }
    Ok(path)
}

fn remove_directory(root: &Path, path: &Path) -> Result<(), String> {
    // Spaces also contain memory and other agent-owned directories. Only linked
    // worktrees need Git deregistration before the whole space is removed.
    let repos = workspace::list_repo_subdirs(path)
        .into_iter()
        .filter(|name| path.join(name).join(".git").is_file())
        .collect();
    workspace::remove_worktrees(path, repos)?;
    let _catalogue = catalogue_lock(root)?;
    fs::remove_dir_all(path).map_err(storage)
}

impl Space {
    /// Blocking disk/git creation. Caller may run this on its existing operation
    /// thread and use the same cancellation/progress types as workspace creation.
    pub fn create(
        profile: &Path,
        form: CreationForm,
        launch_inputs: BTreeMap<String, String>,
        global_config: &Path,
        handle: &CreationHandle,
        events: &EventSink,
        warnings: &mut Vec<String>,
    ) -> Result<Self, String> {
        validate_name(&form.name)?;
        let profile = fs::canonicalize(profile).map_err(storage)?;
        fs::create_dir_all(profile.join("agent-spaces")).map_err(storage)?;
        let root = catalogue(&profile)?;
        let path = root.join(&form.name);
        {
            let _catalogue = catalogue_lock(&root)?;
            if fs::symlink_metadata(&path).is_ok() {
                return Err(format!("agent space '{}' already exists", form.name));
            }
        }
        let registered =
            workspace::get_registered_repos(profile.to_str().ok_or("profile path is not UTF-8")?)?;
        let mut names = HashSet::from([
            "memory".to_owned(),
            DATABASE.to_owned(),
            ".agent-space.lock".to_owned(),
        ]);
        for source in &form.repos {
            let expanded = config::expand_tilde(source)?;
            let source_path = fs::canonicalize(&expanded).map_err(storage)?;
            if !registered
                .iter()
                .any(|repo| fs::canonicalize(&repo.path).is_ok_and(|p| p == source_path))
            {
                return Err(format!("repo is not registered in this profile: {source}"));
            }
            let name = Path::new(&expanded)
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or("invalid repo name")?;
            validate_name(name)?;
            if !names.insert(name.to_lowercase()) {
                return Err(format!("duplicate or reserved repo name: {name}"));
            }
        }
        let prefilled = inputs::prefilled(&profile, global_config, launch_inputs)?;
        let mut inputs: BTreeMap<String, String> = prefilled
            .values
            .into_iter()
            .map(|(key, (value, _))| (key, value))
            .collect();
        inputs.extend(form.inputs.clone());
        if let Some(role) = &form.role {
            inputs.insert("agent.role".into(), role.clone());
        }
        // Derived entries describe the validated on-disk identity.
        for (key, value) in inputs::DERIVED_KEYS
            .into_iter()
            .zip(inputs::derived(&path, &form))
        {
            inputs.insert(key.into(), value);
        }
        let role = inputs
            .get("agent.role")
            .cloned()
            .ok_or("missing agent.role")?;
        if role.trim().is_empty() {
            return Err("agent role cannot be empty".into());
        }
        let instructions = template::render(&prefilled.template, &inputs)?;
        let identity = Identity {
            name: form.name.clone(),
            role,
        };
        let ownership = reserve(&root, &path, &form.name)?;
        let result = (|| {
            let mut db = Db::create(&path.join(DATABASE), &identity, &form, &instructions)?;
            fs::create_dir(path.join("memory")).map_err(storage)?;
            creation::create_worktrees(
                handle,
                profile.to_str().ok_or("profile path is not UTF-8")?,
                &path,
                &form.repos,
                events,
                warnings,
            )?;
            db.finish_creation()?;
            let (_, _, _, resume) = db.load()?;
            Ok((db, resume))
        })();
        match result {
            Ok((db, resume)) => Ok(Self {
                path,
                profile,
                db,
                _ownership: ownership,
                identity,
                form,
                instructions,
                resume,
            }),
            Err(error) => match remove_directory(&root, &path) {
                Ok(()) => Err(error),
                Err(cleanup) => Err(format!("{error}; cleanup failed: {cleanup}")),
            },
        }
    }

    pub fn open(profile: &Path, name: &str) -> Result<Self, String> {
        let profile = fs::canonicalize(profile).map_err(storage)?;
        let root = catalogue(&profile)?;
        let (path, ownership) = {
            let _catalogue = catalogue_lock(&root)?;
            let path = checked_path(&root, name)?;
            let ownership = lock(&path.join(".agent-space.lock"))?;
            (path, ownership)
        };
        let db = Db::open(&path.join(DATABASE), false)?;
        let (identity, form, instructions, resume) = db.load()?;
        if identity.name != name {
            return Err("agent space database name disagrees with its directory".into());
        }
        Ok(Self {
            path,
            profile,
            db,
            _ownership: ownership,
            identity,
            form,
            instructions,
            resume,
        })
    }

    pub fn list(profile: &Path) -> Result<Vec<Entry>, String> {
        let directory = match fs::read_dir(profile.join("agent-spaces")) {
            Ok(directory) => directory,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
            Err(error) => return Err(storage(error)),
        };
        let mut entries = vec![];
        for item in directory {
            let item = item.map_err(storage)?;
            let name = item.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            let result = checked_path(item.path().parent().ok_or("space has no parent")?, &name)
                .and_then(|path| Db::open(&path.join(DATABASE), true))
                .and_then(|db| db.load())
                .and_then(|(identity, _, _, _)| {
                    if identity.name == name {
                        Ok(identity)
                    } else {
                        Err("database name disagrees with directory".into())
                    }
                });
            let (identity, error) = match result {
                Ok(identity) => (Some(identity), None),
                Err(error) => (None, Some(error)),
            };
            entries.push(Entry {
                name,
                path: item.path(),
                identity,
                error,
            });
        }
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(entries)
    }

    pub fn delete(profile: &Path, name: &str) -> Result<(), String> {
        let root = catalogue(profile)?;
        let (path, _ownership) = {
            let _catalogue = catalogue_lock(&root)?;
            let path = checked_path(&root, name)?;
            let ownership = lock(&path.join(".agent-space.lock"))?;
            (path, ownership)
        };
        remove_directory(&root, &path)
    }

    /// The bounded startup tail. Read older items with history_before.
    pub fn history(&self) -> Result<Vec<devora_agent::transcript::TranscriptItem>, String> {
        self.db.history_before(None, 200)
    }

    /// Read-only history also works while the process owner holds the space.
    pub fn read_history(
        profile: &Path,
        name: &str,
        after: u64,
        limit: u32,
    ) -> Result<Vec<devora_agent::transcript::TranscriptItem>, String> {
        let root = catalogue(profile)?;
        let path = checked_path(&root, name)?;
        let db = Db::open(&path.join(DATABASE), true)?;
        db.load()?;
        db.history(after, limit.clamp(1, 500))
    }

    /// Read an older page in chronological order. None selects the latest page.
    pub fn history_before(
        profile: &Path,
        name: &str,
        before: Option<u64>,
        limit: u32,
    ) -> Result<Vec<devora_agent::transcript::TranscriptItem>, String> {
        let root = catalogue(profile)?;
        let path = checked_path(&root, name)?;
        let db = Db::open(&path.join(DATABASE), true)?;
        db.load()?;
        db.history_before(before, limit.clamp(1, 500))
    }

    pub fn identity(&self) -> &Identity {
        &self.identity
    }
    pub fn form(&self) -> &CreationForm {
        &self.form
    }
    pub fn instructions(&self) -> &str {
        &self.instructions
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn saved_session(&self) -> &Resume {
        &self.resume
    }

    pub fn session_options(&self, context: &LaunchContext) -> Result<SessionOptions, String> {
        let settings = config::read_claude_settings_in(
            &context.global_config,
            Some(self.profile.to_str().ok_or("profile path is not UTF-8")?),
        )?;
        let required = |key: &str| {
            settings
                .resolved
                .get(key)
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .ok_or_else(|| format!("agent session requires explicit {key}"))
        };
        let mut behavior = context.settings.clone();
        let env = behavior
            .as_object_mut()
            .ok_or("settings must be a JSON object")?
            .entry("env")
            .or_insert_with(|| serde_json::json!({}))
            .as_object_mut()
            .ok_or("settings.env must be a JSON object")?;
        for (key, value) in settings.model_alias_env() {
            env.entry(key).or_insert(Value::String(value));
        }
        Ok(SessionOptions {
            program: context.program.clone(),
            working_directory: self.path.clone(),
            instructions: Instructions::Append(self.instructions.clone()),
            model: required("default-model")?,
            effort: required("effort")?,
            permission_mode: "auto".into(),
            plugins: context.plugins.clone(),
            settings: behavior,
            environment: context.environment.clone(),
            resume: Some(self.resume.clone()),
        })
    }

    pub async fn start(
        self,
        context: &LaunchContext,
        log: Arc<Log>,
    ) -> Result<RunningSpace, String> {
        let options = self.session_options(context)?;
        running::start(self, options, log).await
    }
}

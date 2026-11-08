//! One app-owned periodic worker, with a host-provided global hold.
use devora_app::log::{Level, Log};
use devora_config::repo_update::Settings;
use devora_platform::clock::{ContinuousClock, MonotonicClock};
use devora_platform::priority::{SystemThreadPriority, ThreadPriority};
use devora_state::{Store, Subscription};
use devora_workspace::{repo_update, workspace};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;
use tokio::sync::{watch, Notify};

pub const LAUNCH_DELAY: Duration = Duration::from_secs(10);
/// Shutdown bounds the host's wait. A checkout that exceeds it keeps its worker
/// and cooperative lock until it finishes. It is never signalled or killed.
pub const SHUTDOWN_WAIT: Duration = Duration::from_secs(30);

/// Hosts own persistence. The queue type is shared transport, independent of
/// Store ownership, so a host can publish its own persisted flag through feed().
pub trait PauseSource: Send + Sync {
    fn paused(&self) -> bool;
    /// Register atomically with publication, yielding the current bool followed
    /// by committed changes, as ADR-013's Store contract does.
    fn subscribe(&self) -> Subscription<bool>;
}

impl PauseSource for Store {
    fn paused(&self) -> bool {
        self.global_paused()
    }
    fn subscribe(&self) -> Subscription<bool> {
        self.subscribe_global_pause()
    }
}

struct Schedule {
    settings: Settings,
    next: Option<Duration>,
}

impl Schedule {
    fn new(settings: Settings, now: Duration) -> Self {
        Self {
            settings,
            next: settings.enabled.then(|| now + LAUNCH_DELAY),
        }
    }
    fn configure(&mut self, settings: Settings, now: Duration) {
        if self.settings == settings {
            return;
        }
        let enabling = !self.settings.enabled && settings.enabled;
        self.settings = settings;
        self.next = if enabling {
            Some(now + LAUNCH_DELAY)
        } else {
            self.deadline(now)
        };
    }
    fn deadline(&self, now: Duration) -> Option<Duration> {
        self.settings
            .enabled
            .then(|| {
                now.checked_add(Duration::from_secs(
                    u64::from(self.settings.interval_minutes) * 60,
                ))
            })
            .flatten()
    }
    fn completed(&mut self, now: Duration) {
        self.next = self.deadline(now);
    }
    fn due(&self, now: Duration) -> bool {
        self.next.is_some_and(|next| next <= now)
    }
}

type Update = Arc<dyn Fn(&Path, &AtomicBool) -> Result<repo_update::Outcome, String> + Send + Sync>;

struct Work {
    config_path: PathBuf,
    pause: Arc<dyn PauseSource>,
    log: Arc<Log>,
    update: Update,
    clock: Arc<dyn MonotonicClock>,
    requested: AtomicBool,
    stop: AtomicBool,
    wake: Notify,
    done: (Mutex<bool>, Condvar),
}

pub struct RepoUpdater {
    work: Arc<Work>,
    settings: watch::Sender<Settings>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl RepoUpdater {
    pub fn start(
        config_path: PathBuf,
        settings: Settings,
        pause: Arc<dyn PauseSource>,
        log: Arc<Log>,
    ) -> Result<Self, String> {
        Self::with_update(
            config_path,
            settings,
            pause,
            log,
            Arc::new(repo_update::update),
            Arc::new(ContinuousClock),
        )
    }

    fn with_update(
        config_path: PathBuf,
        settings: Settings,
        pause: Arc<dyn PauseSource>,
        log: Arc<Log>,
        update: Update,
        clock: Arc<dyn MonotonicClock>,
    ) -> Result<Self, String> {
        let work = Arc::new(Work {
            config_path,
            pause,
            log,
            update,
            clock,
            requested: AtomicBool::new(false),
            stop: AtomicBool::new(false),
            wake: Notify::new(),
            done: (Mutex::new(false), Condvar::new()),
        });
        let (settings, changes) = watch::channel(settings);
        let run = work.clone();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?;
        let thread = std::thread::Builder::new()
            .name("devora-repo-updater".into())
            .spawn(move || {
                if let Err(error) = SystemThreadPriority.background() {
                    run.log.write(Level::Warn, &error);
                }
                if let Err(error) = runtime.block_on(run_worker(run.clone(), changes)) {
                    run.log
                        .write(Level::Warn, &format!("repo-update clock failed: {error}"));
                }
                // Never wait indefinitely for a checkout on runtime destruction.
                runtime.shutdown_timeout(Duration::ZERO);
                *run.done.0.lock().unwrap_or_else(|e| e.into_inner()) = true;
                run.done.1.notify_all();
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            work,
            settings,
            thread: Mutex::new(Some(thread)),
        })
    }

    pub fn run_now(&self) {
        if !self.work.stop.load(Ordering::Acquire) {
            self.work.requested.store(true, Ordering::Release);
            self.work
                .log
                .write(Level::Info, "repo-update manual round requested");
            self.work.wake.notify_one();
        }
    }

    /// Wake notifications re-evaluate a continuous-clock deadline after sleep.
    pub fn woke(&self) {
        self.work.wake.notify_one();
    }

    pub fn configure(&self, settings: Settings) {
        self.settings.send_replace(settings);
    }

    pub fn shutdown(&self) {
        self.work.stop.store(true, Ordering::Release);
        self.work.wake.notify_one();
        let done = self.work.done.0.lock().unwrap_or_else(|e| e.into_inner());
        let (done, _) = self
            .work
            .done
            .1
            .wait_timeout_while(done, SHUTDOWN_WAIT, |done| !*done)
            .unwrap_or_else(|e| e.into_inner());
        if !*done {
            self.work.log.write(Level::Warn, "repo-update shutdown wait expired after 30 seconds; admitted checkout will finish without interruption");
            return;
        }
        if let Some(thread) = self.thread.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = thread.join();
        }
    }
}

impl Drop for RepoUpdater {
    fn drop(&mut self) {
        if !self.work.stop.load(Ordering::Acquire) {
            self.shutdown();
        }
    }
}

async fn wait_deadline(next: Option<Duration>, now: Duration) {
    match next {
        Some(next) => tokio::time::sleep(next.saturating_sub(now)).await,
        None => std::future::pending().await,
    }
}

async fn run_worker(
    work: Arc<Work>,
    mut settings: watch::Receiver<Settings>,
) -> std::io::Result<()> {
    let changes = work.pause.subscribe();
    let mut schedule = Schedule::new(*settings.borrow_and_update(), work.clock.now()?);
    loop {
        let now = work.clock.now()?;
        if work.stop.load(Ordering::Acquire) {
            return Ok(());
        }
        if !work.pause.paused()
            && (work.requested.swap(false, Ordering::AcqRel) || schedule.due(now))
        {
            run_round(work.clone(), &changes).await;
            schedule.settings = *settings.borrow_and_update();
            schedule.completed(work.clock.now()?);
            continue;
        }
        let deadline = if work.pause.paused() {
            None
        } else {
            schedule.next
        };
        tokio::select! {
            _ = work.wake.notified() => {},
            _ = changes.next_batch(32) => {},
            changed = settings.changed() => {
                if changed.is_err() { return Ok(()); }
                schedule.configure(*settings.borrow_and_update(), work.clock.now()?);
            },
            _ = wait_deadline(deadline, now) => {},
        }
    }
}

fn repos(config_path: &Path) -> Result<BTreeSet<PathBuf>, String> {
    let mut repos = BTreeSet::new();
    for profile in workspace::list_profiles_in(config_path)? {
        let profile = std::fs::canonicalize(profile.path).map_err(|e| e.to_string())?;
        let Ok(root) = std::fs::canonicalize(profile.join("repos")) else {
            continue;
        };
        if !root.starts_with(&profile) {
            continue;
        }
        for entry in std::fs::read_dir(&root).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let Ok(repo) = entry.path().canonicalize() else {
                continue;
            };
            if repo.parent() == Some(&root) && repo.join(".git").exists() {
                repos.insert(repo);
            }
        }
    }
    Ok(repos)
}

async fn run_round(work: Arc<Work>, changes: &Subscription<bool>) {
    let repos = match repos(&work.config_path) {
        Ok(repos) => repos,
        Err(error) => {
            work.log.write(
                Level::Warn,
                &format!("repo-update enumeration failed: {error}"),
            );
            return;
        }
    };
    let mut completed = 0;
    for repo in &repos {
        while work.pause.paused() && !work.stop.load(Ordering::Acquire) {
            tokio::select! { _ = changes.next_batch(32) => {}, _ = work.wake.notified() => {} }
        }
        if work.stop.load(Ordering::Acquire) {
            break;
        }
        let update = work.clone();
        let path = repo.clone();
        let outcome = tokio::task::spawn_blocking(move || {
            if let Err(error) = SystemThreadPriority.background() {
                update.log.write(Level::Warn, &error);
            }
            (update.update)(&path, &update.stop)
        })
        .await;
        let (level, detail) = match outcome {
            Ok(Ok(repo_update::Outcome::Updated(target))) => {
                (Level::Info, format!("updated to {target}"))
            }
            Ok(Ok(repo_update::Outcome::Skipped(reason))) => {
                (Level::Info, format!("skipped: {reason}"))
            }
            Ok(Err(error)) => (Level::Warn, format!("failed: {error}")),
            Err(error) => (Level::Warn, format!("worker failed: {error}")),
        };
        work.log
            .write(level, &format!("repo-update {} {detail}", repo.display()));
        completed += 1;
    }
    work.log.write(
        Level::Info,
        &format!(
            "repo-update round completed {completed}/{} repos",
            repos.len()
        ),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use devora_state::Command;
    use std::sync::mpsc;

    #[test]
    fn clock_controls_launch_completion_wake_and_reconfiguration() {
        let now = Duration::from_secs(100);
        let on = Settings {
            enabled: true,
            interval_minutes: 60,
        };
        let mut schedule = Schedule::new(on, now);
        assert!(!schedule.due(now + Duration::from_secs(9)));
        assert!(schedule.due(now + LAUNCH_DELAY));
        let wake = now + Duration::from_secs(86_400);
        assert!(schedule.due(wake));
        schedule.completed(wake);
        assert_eq!(schedule.next, Some(wake + Duration::from_secs(3600)));
        assert!(!schedule.due(wake));
        schedule.configure(
            Settings {
                interval_minutes: 5,
                ..on
            },
            wake,
        );
        assert_eq!(schedule.next, Some(wake + Duration::from_secs(300)));
        schedule.configure(
            Settings {
                enabled: false,
                ..on
            },
            wake,
        );
        assert_eq!(schedule.next, None);
        schedule.configure(on, wake);
        assert_eq!(schedule.next, Some(wake + LAUNCH_DELAY));
    }

    fn fixture() -> (tempfile::TempDir, PathBuf, BTreeSet<PathBuf>) {
        let root = tempfile::tempdir().unwrap();
        let profiles = [root.path().join("A"), root.path().join("B")];
        let mut expected = BTreeSet::new();
        for profile in &profiles {
            std::fs::create_dir_all(profile.join("repos/clone/.git")).unwrap();
            std::fs::write(profile.join("config.json"), r#"{"name":"Work"}"#).unwrap();
            expected.insert(profile.join("repos/clone").canonicalize().unwrap());
        }
        let external = root.path().join("external");
        std::fs::create_dir_all(external.join(".git")).unwrap();
        std::os::unix::fs::symlink(&external, profiles[0].join("repos/escaped")).unwrap();
        std::os::unix::fs::symlink(
            profiles[0].join("repos/clone"),
            profiles[0].join("repos/alias"),
        )
        .unwrap();
        let config = root.path().join("config.json");
        std::fs::write(
            &config,
            serde_json::json!({"profiles": profiles}).to_string(),
        )
        .unwrap();
        (root, config, expected)
    }

    struct ObservedPause {
        state: Store,
        held: mpsc::Sender<()>,
    }
    impl PauseSource for ObservedPause {
        fn paused(&self) -> bool {
            let paused = self.state.global_paused();
            if paused {
                let _ = self.held.send(());
            }
            paused
        }
        fn subscribe(&self) -> Subscription<bool> {
            self.state.subscribe_global_pause()
        }
    }

    #[test]
    fn manual_rounds_coalesce_while_disabled_and_pause_at_the_next_repo() {
        let (root, config, expected) = fixture();
        assert_eq!(repos(&config).unwrap(), expected);
        let (held, observed) = mpsc::channel();
        let pause = Arc::new(ObservedPause {
            state: Store::new(),
            held,
        });
        pause
            .state
            .dispatch(Command::SetGlobalPaused { paused: true })
            .unwrap();
        let (started, received) = mpsc::channel();
        let gate = Arc::new((Mutex::new(false), Condvar::new()));
        let unblock = gate.clone();
        let calls = std::sync::atomic::AtomicUsize::new(0);
        let update: Update = Arc::new(move |path, _| {
            started.send(path.to_owned()).unwrap();
            if calls.fetch_add(1, Ordering::SeqCst) == 0 {
                let open = gate.0.lock().unwrap();
                drop(gate.1.wait_while(open, |open| !*open).unwrap());
            }
            Ok(repo_update::Outcome::Updated("test".into()))
        });
        let log = Arc::new(Log::open(&root.path().join("app.log")).unwrap());
        let worker = RepoUpdater::with_update(
            config,
            Settings {
                enabled: false,
                interval_minutes: 60,
            },
            pause.clone(),
            log,
            update,
            Arc::new(ContinuousClock),
        )
        .unwrap();
        worker.run_now();
        worker.run_now();
        observed.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(received.try_recv().is_err());
        pause
            .state
            .dispatch(Command::SetGlobalPaused { paused: false })
            .unwrap();
        let first = received.recv_timeout(Duration::from_secs(5)).unwrap();
        worker.run_now();
        worker.run_now();
        worker.run_now();
        while observed.try_recv().is_ok() {}
        pause
            .state
            .dispatch(Command::SetGlobalPaused { paused: true })
            .unwrap();
        *unblock.0.lock().unwrap() = true;
        unblock.1.notify_all();
        observed.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(
            received.try_recv().is_err(),
            "the admitted repo finishes and the next stays held"
        );
        pause
            .state
            .dispatch(Command::SetGlobalPaused { paused: false })
            .unwrap();
        let second = received.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_ne!(first, second);
        assert_eq!(
            received.recv_timeout(Duration::from_secs(5)).unwrap(),
            first
        );
        assert_eq!(
            received.recv_timeout(Duration::from_secs(5)).unwrap(),
            second
        );
        worker.shutdown();
        assert!(
            received.try_recv().is_err(),
            "overlapping requests produce one pending round"
        );
    }
    #[test]
    fn wake_rechecks_an_overdue_continuous_deadline_without_catch_up_rounds() {
        struct Clock {
            now: Mutex<Duration>,
            reads: mpsc::Sender<()>,
        }
        impl MonotonicClock for Clock {
            fn now(&self) -> std::io::Result<Duration> {
                let now = *self.now.lock().unwrap();
                let _ = self.reads.send(());
                Ok(now)
            }
        }
        let (root, config, expected) = fixture();
        let (read, observed) = mpsc::channel();
        let clock = Arc::new(Clock {
            now: Mutex::new(Duration::from_secs(100)),
            reads: read,
        });
        let (updated, received) = mpsc::channel();
        let update: Update = Arc::new(move |repo, _| {
            updated.send(repo.to_owned()).unwrap();
            Ok(repo_update::Outcome::Updated("test".into()))
        });
        let worker = RepoUpdater::with_update(
            config,
            Settings {
                enabled: true,
                interval_minutes: 60,
            },
            Arc::new(Store::new()),
            Arc::new(Log::open(&root.path().join("wake.log")).unwrap()),
            update,
            clock.clone(),
        )
        .unwrap();
        // The scheduler has read its initial deadline and entered the wait loop.
        observed.recv_timeout(Duration::from_secs(5)).unwrap();
        observed.recv_timeout(Duration::from_secs(5)).unwrap();
        *clock.now.lock().unwrap() = Duration::from_secs(86_500);
        worker.woke();
        let actual = [
            received.recv_timeout(Duration::from_secs(5)).unwrap(),
            received.recv_timeout(Duration::from_secs(5)).unwrap(),
        ]
        .into_iter()
        .collect();
        assert_eq!(expected, actual);
        worker.shutdown();
        assert!(received.try_recv().is_err());
    }
}

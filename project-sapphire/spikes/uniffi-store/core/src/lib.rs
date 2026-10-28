//! Spike 2 (#206): the smallest core that exercises the UniFFI boundary Sapphire will depend on.
//!
//! - an async command with a typed error (`open_workspace`)
//! - an event stream delivered two ways: (a) a foreign-implemented `EventSink`, (b) an async pull loop
//! - probes for cancellation and panics crossing the boundary

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

uniffi::setup_scaffolding!();

/// Nanoseconds on the same clock as Swift's `DispatchTime.now().uptimeNanoseconds`, so latency can be measured
/// across the boundary.
#[uniffi::export]
pub fn now_ns() -> u64 {
    unsafe extern "C" {
        fn clock_gettime_nsec_np(clock_id: libc::clockid_t) -> u64;
    }
    unsafe { clock_gettime_nsec_np(libc::CLOCK_UPTIME_RAW) }
}

#[derive(uniffi::Record)]
pub struct WorkspaceInfo {
    pub path: String,
    pub name: String,
    pub entry_count: u32,
}

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum CoreError {
    #[error("no such workspace: {path}")]
    NotFound { path: String },
    #[error("not a directory: {path}")]
    NotADirectory { path: String },
    #[error("I/O error: {message}")]
    Io { message: String },
    #[error("no event sink is set")]
    NoSink,
    #[error("cancelled")]
    Cancelled,
}

/// Explicit cancellation: UniFFI's Swift bindings don't propagate `Task` cancellation into the Rust future.
#[derive(uniffi::Object)]
pub struct CancelToken {
    tx: async_channel::Sender<()>,
    rx: async_channel::Receiver<()>,
}

#[uniffi::export]
impl CancelToken {
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        let (tx, rx) = async_channel::bounded(1);
        Arc::new(Self { tx, rx })
    }

    pub fn cancel(&self) {
        self.tx.close();
    }
}

#[derive(uniffi::Enum, Clone, Debug)]
pub enum CoreEvent {
    Delta { seq: u64, text: String, produced_ns: u64 },
    StreamEnded { stream_id: u64, count: u64 },
}

#[derive(uniffi::Enum, Clone, Copy, Debug)]
pub enum Delivery {
    /// (a) one foreign call per event
    Callback,
    /// (a) one foreign call per producer tick (~1 ms)
    CallbackBatched,
    /// (b) events are queued; Swift pulls with `next_event` or `next_events`
    Pull,
}

#[derive(uniffi::Record, Clone, Copy, Debug)]
pub struct StreamConfig {
    /// 0 means as fast as possible.
    pub rate_per_sec: u32,
    pub count: u64,
    pub delta_len: u32,
    pub delivery: Delivery,
}

#[uniffi::export(with_foreign)]
pub trait EventSink: Send + Sync {
    fn on_event(&self, event: CoreEvent);
    fn on_events(&self, events: Vec<CoreEvent>);
}

#[derive(uniffi::Record, Debug, PartialEq, Eq)]
pub struct CancellationStats {
    pub started: u64,
    pub completed: u64,
    pub dropped_before_completion: u64,
}

#[derive(uniffi::Object)]
pub struct SpikeCore {
    sink: Mutex<Option<Arc<dyn EventSink>>>,
    tx: async_channel::Sender<CoreEvent>,
    rx: async_channel::Receiver<CoreEvent>,
    next_stream_id: AtomicU64,
    sleeps_started: AtomicU64,
    sleeps_completed: AtomicU64,
    sleeps_dropped: AtomicU64,
}

/// Counts a future that was dropped before it finished, which is how a cancelled Rust future ends.
struct DropTracker<'a> {
    dropped: &'a AtomicU64,
    finished: bool,
}

impl Drop for DropTracker<'_> {
    fn drop(&mut self) {
        if !self.finished {
            self.dropped.fetch_add(1, Ordering::SeqCst);
        }
    }
}

const BATCH_CAP: usize = 1024;
const PULL_BATCH_DEFAULT: usize = 4096;

#[uniffi::export]
impl SpikeCore {
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        let (tx, rx) = async_channel::unbounded();
        Arc::new(Self {
            sink: Mutex::new(None),
            tx,
            rx,
            next_stream_id: AtomicU64::new(1),
            sleeps_started: AtomicU64::new(0),
            sleeps_completed: AtomicU64::new(0),
            sleeps_dropped: AtomicU64::new(0),
        })
    }

    pub async fn open_workspace(&self, path: String) -> Result<WorkspaceInfo, CoreError> {
        let meta = std::fs::metadata(&path).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => CoreError::NotFound { path: path.clone() },
            _ => CoreError::Io { message: e.to_string() },
        })?;
        if !meta.is_dir() {
            return Err(CoreError::NotADirectory { path });
        }
        let entry_count = std::fs::read_dir(&path)
            .map_err(|e| CoreError::Io { message: e.to_string() })?
            .count() as u32;
        let name = std::path::Path::new(&path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        Ok(WorkspaceInfo { path, name, entry_count })
    }

    /// Sleeps, recording whether the future ran to completion or was dropped (cancelled) first.
    pub async fn sleep(&self, millis: u64) -> u64 {
        self.sleeps_started.fetch_add(1, Ordering::SeqCst);
        let mut tracker = DropTracker { dropped: &self.sleeps_dropped, finished: false };
        async_io::Timer::after(Duration::from_millis(millis)).await;
        tracker.finished = true;
        self.sleeps_completed.fetch_add(1, Ordering::SeqCst);
        millis
    }

    pub fn cancellation_stats(&self) -> CancellationStats {
        CancellationStats {
            started: self.sleeps_started.load(Ordering::SeqCst),
            completed: self.sleeps_completed.load(Ordering::SeqCst),
            dropped_before_completion: self.sleeps_dropped.load(Ordering::SeqCst),
        }
    }

    /// Like `sleep`, but ends early with `CoreError::Cancelled` once `token` is cancelled.
    pub async fn sleep_cancellable(&self, millis: u64, token: Arc<CancelToken>) -> Result<u64, CoreError> {
        let timer = async {
            async_io::Timer::after(Duration::from_millis(millis)).await;
            Ok(millis)
        };
        let cancelled = async {
            let _ = token.rx.recv().await;
            Err(CoreError::Cancelled)
        };
        futures_lite::future::or(timer, cancelled).await
    }

    /// Infallible signature: the generated Swift uses `try!`, so this panic aborts the Swift process.
    pub fn panic_sync(&self) -> u32 {
        panic!("deliberate sync panic")
    }

    pub async fn panic_async(&self) -> u32 {
        async_io::Timer::after(Duration::from_millis(1)).await;
        panic!("deliberate async panic")
    }

    /// Fallible signature: the panic surfaces in Swift as a thrown error.
    pub fn panic_sync_fallible(&self) -> Result<u32, CoreError> {
        panic!("deliberate sync panic")
    }

    pub async fn panic_async_fallible(&self) -> Result<u32, CoreError> {
        async_io::Timer::after(Duration::from_millis(1)).await;
        panic!("deliberate async panic")
    }

    pub fn set_sink(&self, sink: Arc<dyn EventSink>) {
        *self.sink.lock().unwrap() = Some(sink);
    }

    pub fn clear_sink(&self) {
        *self.sink.lock().unwrap() = None;
    }

    /// Starts a producer thread that emits `config.count` deltas followed by `StreamEnded`.
    pub fn start_stream(&self, config: StreamConfig) -> Result<u64, CoreError> {
        let stream_id = self.next_stream_id.fetch_add(1, Ordering::SeqCst);
        let out = match config.delivery {
            Delivery::Pull => Output::Channel(self.tx.clone()),
            Delivery::Callback | Delivery::CallbackBatched => {
                let sink = self.sink.lock().unwrap().clone().ok_or(CoreError::NoSink)?;
                Output::Sink { sink, batched: matches!(config.delivery, Delivery::CallbackBatched) }
            }
        };
        std::thread::Builder::new()
            .name(format!("spike-stream-{stream_id}"))
            .spawn(move || produce(stream_id, config, out))
            .map_err(|e| CoreError::Io { message: e.to_string() })?;
        Ok(stream_id)
    }

    pub async fn next_event(&self) -> Option<CoreEvent> {
        self.rx.recv().await.ok()
    }

    /// Waits for at least one event, then drains up to `max` without waiting further.
    pub async fn next_events(&self, max: u32) -> Vec<CoreEvent> {
        let max = if max == 0 { PULL_BATCH_DEFAULT } else { max as usize };
        let Ok(first) = self.rx.recv().await else { return Vec::new() };
        let mut events = Vec::with_capacity(max.min(self.rx.len() + 1));
        events.push(first);
        while events.len() < max {
            match self.rx.try_recv() {
                Ok(e) => events.push(e),
                Err(_) => break,
            }
        }
        events
    }
}

enum Output {
    Channel(async_channel::Sender<CoreEvent>),
    Sink { sink: Arc<dyn EventSink>, batched: bool },
}

impl Output {
    fn emit(&self, pending: &mut Vec<CoreEvent>, event: CoreEvent) {
        match self {
            Output::Channel(tx) => {
                let _ = tx.send_blocking(event);
            }
            Output::Sink { sink, batched: false } => sink.on_event(event),
            Output::Sink { sink, batched: true } => {
                pending.push(event);
                if pending.len() >= BATCH_CAP {
                    sink.on_events(std::mem::take(pending));
                }
            }
        }
    }

    fn flush(&self, pending: &mut Vec<CoreEvent>) {
        if let Output::Sink { sink, batched: true } = self
            && !pending.is_empty()
        {
            sink.on_events(std::mem::take(pending));
        }
    }
}

/// Deadline-paced producer: each ~1 ms tick emits every event that is due, like a bursty token stream.
fn produce(stream_id: u64, config: StreamConfig, out: Output) {
    let words = ["the ", "agent ", "streams ", "token ", "deltas ", "into ", "a ", "native ", "store. "];
    let start = Instant::now();
    let mut pending = Vec::new();
    let mut sent = 0u64;
    while sent < config.count {
        let due = if config.rate_per_sec == 0 {
            config.count
        } else {
            let elapsed = start.elapsed().as_secs_f64();
            ((elapsed * config.rate_per_sec as f64) as u64 + 1).min(config.count)
        };
        while sent < due {
            let mut text = String::with_capacity(config.delta_len as usize);
            while text.len() < config.delta_len as usize {
                text.push_str(words[(sent as usize + text.len()) % words.len()]);
            }
            text.truncate(config.delta_len as usize);
            out.emit(&mut pending, CoreEvent::Delta { seq: sent, text, produced_ns: now_ns() });
            sent += 1;
        }
        out.flush(&mut pending);
        if config.rate_per_sec != 0 && sent < config.count {
            std::thread::sleep(Duration::from_micros(500));
        }
    }
    out.emit(&mut pending, CoreEvent::StreamEnded { stream_id, count: sent });
    out.flush(&mut pending);
}

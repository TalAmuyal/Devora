//! A persistent Claude session over the typed protocol (ADR-009, ADR-012).
//!
//! The owner supplies all launch inputs and drains events independently of
//! command replies. Dropping the owner initiates shutdown. Explicit pause waits
//! for shutdown and returns the ID a later start can resume.
//!
//! One turn runs at a time. Concurrent submissions receive an error. Control
//! replies and stdin writes are bounded to 30 seconds. Pause gives an interrupted
//! turn five seconds to finish before terminating and reaping the process.
//!
//! Unexpected exit or control timeout gets one automatic restart. A successful turn re-arms
//! that recovery budget. Failed initialization and repeated crashes surface a
//! Failed event. Idle sessions have no timer. Before a conversation exists, a
//! restart reuses --session-id. After a result or successful resume, it uses --resume.
//!
//! The caller's event loop owns history and context policy. Usage and compaction
//! boundaries are delivered directly from the CLI. Space-specific state stays
//! with the caller.

mod judging;
mod launch;
mod worker;

use crate::protocol::{
    CompactMetadata, ControlRequest, InitializeResponse, Message, PermissionRequest,
};
use devora_app::log::Log;
pub use launch::{Instructions, Plugin, Resume, SessionOptions};
use serde_json::{Map, Value};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

/// Which CLI message supplied a token-usage update.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsageSource {
    Assistant,
    MessageStart,
    MessageDelta,
    Result,
}

/// Events for a session's owner. Wire messages remain available for the caller's
/// transcript/history policy, alongside lifecycle and interactive-control events.
#[derive(Debug, Clone)]
pub enum SessionEvent {
    Ready {
        session_id: String,
        resumed: bool,
        capabilities: InitializeResponse,
    },
    Message(Message),
    PermissionRequested {
        request_id: String,
        question: PermissionRequest,
    },
    PermissionCancelled {
        request_id: String,
    },
    McpRequested {
        request_id: String,
        server_name: String,
        message: Value,
    },
    ToolJudged {
        tool_use_id: Option<String>,
        verdict: judgess::Verdict,
    },
    Usage {
        source: UsageSource,
        usage: Value,
    },
    Compacted(CompactMetadata),
    Warning(String),
    Exited {
        code: Option<i32>,
        restarting: bool,
    },
    Stopped(StopInfo),
    Failed(String),
}

/// Whether the CLI completed clean shutdown or required forced termination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StopInfo {
    pub session_id: String,
    /// True after a result or successful resume. Persist alongside the ID so
    /// an unused session can reopen with --session-id.
    pub has_conversation: bool,
    pub graceful: bool,
}

/// The answers supported by an inline permission question. Suggested rules are
/// taken from that question's CLI payload, preserving the intended scope.
#[derive(Debug, Clone)]
pub enum PermissionAnswer {
    Allow,
    Deny { message: String },
    AllowWithSuggestedRule,
}

type Reply<T> = oneshot::Sender<Result<T, String>>;

pub(super) enum Command {
    Control(ControlRequest, Reply<Value>),
    Turn(String, Reply<()>),
    Permission(String, PermissionAnswer, Reply<()>),
    Mcp(String, Value, Reply<()>),
    Pause(Option<Reply<StopInfo>>),
}

/// A cloneable command handle, independent of the task draining session events.
#[derive(Clone)]
pub struct SessionHandle {
    commands: mpsc::UnboundedSender<Command>,
}

impl SessionHandle {
    async fn call<T>(&self, command: impl FnOnce(Reply<T>) -> Command) -> Result<T, String> {
        let (reply, received) = oneshot::channel();
        self.commands
            .send(command(reply))
            .map_err(|_| "session has stopped")?;
        received
            .await
            .map_err(|_| "session stopped before answering")?
    }

    /// Pause while another task drains events, retaining the resumable ID.
    pub async fn pause(&self) -> Result<StopInfo, String> {
        self.call(|reply| Command::Pause(Some(reply))).await
    }

    /// Submit one turn. A session accepts the next turn after the current result.
    pub async fn send_turn(&self, text: String) -> Result<(), String> {
        self.call(|reply| Command::Turn(text, reply)).await
    }

    /// Interrupt a running turn while keeping the process available.
    pub async fn interrupt(&self) -> Result<Value, String> {
        self.call(|reply| Command::Control(ControlRequest::Interrupt { extra: Map::new() }, reply))
            .await
    }

    /// Change permission mode through the CLI's control channel.
    pub async fn set_permission_mode(&self, mode: String) -> Result<Value, String> {
        self.call(|reply| {
            Command::Control(
                ControlRequest::SetPermissionMode {
                    mode,
                    extra: Map::new(),
                },
                reply,
            )
        })
        .await
    }

    /// Change the model for subsequent turns.
    pub async fn set_model(&self, model: String) -> Result<Value, String> {
        self.call(|reply| {
            Command::Control(
                ControlRequest::SetModel {
                    model: crate::protocol::Nullable::Value(model),
                    extra: Map::new(),
                },
                reply,
            )
        })
        .await
    }

    /// Answer a currently pending question. A late answer to a withdrawn question
    /// fails locally instead of sending a stale permission decision.
    pub async fn answer_permission(
        &self,
        request_id: String,
        answer: PermissionAnswer,
    ) -> Result<(), String> {
        self.call(|reply| Command::Permission(request_id, answer, reply))
            .await
    }

    /// Answer a pending in-process MCP request with a JSON-RPC response.
    /// The worker supplies the CLI control protocol's mcp_response wrapper.
    pub async fn answer_mcp(&self, request_id: String, response: Value) -> Result<(), String> {
        self.call(|reply| Command::Mcp(request_id, response, reply))
            .await
    }
}

/// Owns one session's worker and event stream. Commands are serialized by the
/// worker, with stdout/stderr drained while requests await their control replies.
pub struct Session {
    handle: SessionHandle,
    events: mpsc::UnboundedReceiver<SessionEvent>,
    worker: Option<JoinHandle<()>>,
    session_id: String,
}

impl Session {
    /// Validate the CLI version, launch it and wait for initialize to succeed.
    pub async fn start(options: SessionOptions, log: Arc<Log>) -> Result<Self, String> {
        let session_id = options
            .resume
            .as_ref()
            .map(|resume| resume.session_id.clone())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        worker::check_version(&options).await?;
        let (commands, received) = mpsc::unbounded_channel();
        let (events, event_stream) = mpsc::unbounded_channel();
        let (ready, initialized) = oneshot::channel();
        let worker_id = session_id.clone();
        let worker = tokio::spawn(worker::run(
            options, worker_id, log, received, events, ready,
        ));
        let session = Self {
            handle: SessionHandle { commands },
            events: event_stream,
            worker: Some(worker),
            session_id,
        };
        initialized
            .await
            .map_err(|_| "session stopped during initialize")??;
        Ok(session)
    }

    pub fn handle(&self) -> SessionHandle {
        self.handle.clone()
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub async fn next_event(&mut self) -> Option<SessionEvent> {
        self.events.recv().await
    }

    /// Interrupt an active turn, stop the process, and retain its resumable ID.
    /// Final events remain readable until next_event returns None.
    pub async fn pause(&mut self) -> Result<StopInfo, String> {
        let result = self.handle.pause().await;
        if let Some(worker) = self.worker.take() {
            worker
                .await
                .map_err(|error| format!("session worker stopped unexpectedly: {error}"))?;
        }
        result
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if self.worker.is_some() {
            let _ = self.handle.commands.send(Command::Pause(None));
        }
    }
}

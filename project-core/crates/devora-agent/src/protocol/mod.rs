//! Claude Code's two stream-json channels (ADR-009).
//!
//! Each line is one [`Message`], in either direction. Known wire shapes are typed;
//! every unrecognized shape remains [`Extensible::Raw`], including nested content,
//! deltas and requests. Invalid JSON is still a transport error. Extra fields stay
//! in each record's `extra` map so decoding and encoding preserve the full JSON.
//! Session identity and other optional envelope metadata remain in those maps.

mod control;
mod conversation;

pub use control::*;
pub use conversation::*;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// A known wire shape or the entire JSON value of an unknown shape.
///
/// A changed required field also falls back to `Raw`, so a CLI upgrade cannot
/// silently discard a message. Consumers must handle that variant explicitly.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Extensible<T> {
    Known(T),
    Raw(Value),
}

/// A required wire field that explicitly permits JSON null. Unlike `Option`,
/// a missing field fails typed decoding and preserves its whole shape as Raw.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Nullable<T> {
    Value(T),
    Null(()),
}

/// A complete conversation or control envelope on stdin or stdout.
pub type Message = Extensible<KnownMessage>;

/// The envelope kinds understood by this protocol version.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum KnownMessage {
    User {
        message: Turn,
        #[serde(flatten)]
        extra: Map<String, Value>,
    },
    Assistant {
        message: Turn,
        #[serde(flatten)]
        extra: Map<String, Value>,
    },
    System(System),
    Result(TurnResult),
    StreamEvent {
        event: Extensible<StreamEvent>,
        #[serde(flatten)]
        extra: Map<String, Value>,
    },
    ControlRequest {
        request_id: String,
        request: Extensible<ControlRequest>,
        #[serde(flatten)]
        extra: Map<String, Value>,
    },
    ControlResponse {
        response: Extensible<ControlResponse>,
        #[serde(flatten)]
        extra: Map<String, Value>,
    },
    ControlCancelRequest {
        request_id: String,
        #[serde(flatten)]
        extra: Map<String, Value>,
    },
}

impl Message {
    /// Decode one complete JSONL line. Unknown valid JSON passes through as `Raw`.
    pub fn parse(line: &str) -> serde_json::Result<Self> {
        serde_json::from_str(line)
    }
}

impl Message {
    /// A host control request, correlated by an ID chosen by its caller.
    pub fn request(request_id: String, request: ControlRequest) -> Self {
        Self::Known(KnownMessage::ControlRequest {
            request_id,
            request: Extensible::Known(request),
            extra: Map::new(),
        })
    }

    /// A user text turn sent to a persistent CLI process.
    pub fn user_text(session_id: String, text: String) -> Self {
        Self::Known(KnownMessage::User {
            message: Turn {
                role: "user".into(),
                content: Content::Text(text),
                extra: Map::new(),
            },
            extra: Map::from_iter([
                ("session_id".into(), Value::String(session_id)),
                ("parent_tool_use_id".into(), Value::Null),
            ]),
        })
    }

    /// A successful permission, hook or MCP answer, or a CLI control response.
    pub fn success(request_id: String, response: Value) -> Self {
        Self::Known(KnownMessage::ControlResponse {
            response: Extensible::Known(ControlResponse::Success {
                request_id,
                response,
                extra: Map::new(),
            }),
            extra: Map::new(),
        })
    }
}

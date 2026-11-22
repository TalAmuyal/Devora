use super::{Extensible, Nullable};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// A user or assistant turn. IDs, usage, model and other CLI metadata are in `extra`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Turn {
    pub role: String,
    pub content: Content,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Text input, structured message content, or an unfamiliar content representation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Content {
    Text(String),
    Blocks(Vec<Extensible<ContentBlock>>),
    Raw(Value),
}

/// Whole content blocks. A tool result normally arrives inside a `user` envelope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text {
        text: String,
        #[serde(flatten)]
        extra: Map<String, Value>,
    },
    Thinking {
        thinking: String,
        #[serde(flatten)]
        extra: Map<String, Value>,
    },
    ToolUse {
        id: String,
        name: String,
        input: Value,
        #[serde(flatten)]
        extra: Map<String, Value>,
    },
    ToolResult {
        tool_use_id: String,
        content: Content,
        #[serde(flatten)]
        extra: Map<String, Value>,
    },
}

/// CLI lifecycle messages. Unknown system subtypes make the full envelope `Raw`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "subtype", rename_all = "snake_case")]
pub enum System {
    Init {
        session_id: String,
        model: String,
        tools: Vec<String>,
        #[serde(flatten)]
        extra: Map<String, Value>,
    },
    Status {
        status: Value,
        #[serde(flatten)]
        extra: Map<String, Value>,
    },
    CompactBoundary {
        compact_metadata: CompactMetadata,
        #[serde(flatten)]
        extra: Map<String, Value>,
    },
    HookStarted {
        hook_id: String,
        hook_name: String,
        hook_event: String,
        #[serde(flatten)]
        extra: Map<String, Value>,
    },
    HookResponse {
        hook_id: String,
        hook_name: String,
        hook_event: String,
        #[serde(flatten)]
        extra: Map<String, Value>,
    },
    ThinkingTokens {
        estimated_tokens: u64,
        #[serde(flatten)]
        extra: Map<String, Value>,
    },
    /// The live list of the agent's background work, whenever it changes.
    BackgroundTasksChanged {
        tasks: Vec<BackgroundTask>,
        #[serde(flatten)]
        extra: Map<String, Value>,
    },
    /// The model the agent is set to was unavailable (overloaded), so this turn's request went to the fallback
    /// model instead. Sent again on every turn that falls back; `content` is the CLI's own sentence for it.
    ModelFallback {
        trigger: String,
        original_model: String,
        fallback_model: String,
        content: String,
        #[serde(flatten)]
        extra: Map<String, Value>,
    },
    /// The CLI is retrying a failed request; it reports the outcome through the turn itself.
    ApiRetry {
        #[serde(flatten)]
        extra: Map<String, Value>,
    },
}

/// One piece of background work the CLI tracks: a background shell
/// (`local_bash`) or a background subagent (`local_agent`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackgroundTask {
    pub task_id: String,
    pub task_type: String,
    pub description: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// The compaction boundary supplied by the CLI. Token counts beyond the original
/// `pre_tokens` contract remain in `extra` when a CLI version supplies them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompactMetadata {
    pub trigger: String,
    pub pre_tokens: u64,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A completed or failed turn, including interruption. Its subtype is kept open
/// because CLI error subtypes evolve independently of the result envelope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TurnResult {
    pub subtype: String,
    pub session_id: String,
    pub is_error: bool,
    pub num_turns: u64,
    pub usage: Usage,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl TurnResult {
    /// Whether this result ends the user turn sent with `id`. The CLI also
    /// produces results nobody asked for: a turn it starts itself when
    /// background work finishes, and a zero-turn one on resume. Those name no
    /// user message, so they never end a turn we sent.
    pub fn answers(&self, id: &str) -> bool {
        self.extra
            .get("user_message_uuids")
            .and_then(Value::as_array)
            .is_some_and(|ids| ids.iter().any(|value| value.as_str() == Some(id)))
    }
}

/// Token accounting shared by completed messages and streaming message updates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A streaming usage update. Input/cache counters are optional on the wire and
/// remain in extra when present, while output_tokens is always supplied.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PartialUsage {
    pub output_tokens: u64,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Partial conversation events. Token deltas are protocol events, independent of
/// any UI-facing state or durable history representation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEvent {
    MessageStart {
        message: Turn,
        #[serde(flatten)]
        extra: Map<String, Value>,
    },
    ContentBlockStart {
        index: u64,
        content_block: Extensible<ContentBlock>,
        #[serde(flatten)]
        extra: Map<String, Value>,
    },
    ContentBlockDelta {
        index: u64,
        delta: Extensible<ContentDelta>,
        #[serde(flatten)]
        extra: Map<String, Value>,
    },
    ContentBlockStop {
        index: u64,
        #[serde(flatten)]
        extra: Map<String, Value>,
    },
    MessageDelta {
        delta: MessageDelta,
        usage: PartialUsage,
        #[serde(flatten)]
        extra: Map<String, Value>,
    },
    MessageStop {
        #[serde(flatten)]
        extra: Map<String, Value>,
    },
}

/// Pieces of a text, thinking, signature or tool-input block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentDelta {
    TextDelta {
        text: String,
        #[serde(flatten)]
        extra: Map<String, Value>,
    },
    InputJsonDelta {
        partial_json: String,
        #[serde(flatten)]
        extra: Map<String, Value>,
    },
    ThinkingDelta {
        thinking: String,
        #[serde(flatten)]
        extra: Map<String, Value>,
    },
    SignatureDelta {
        signature: String,
        #[serde(flatten)]
        extra: Map<String, Value>,
    },
}

/// A message-level update, whose stop reason may explicitly be null.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessageDelta {
    pub stop_reason: Nullable<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

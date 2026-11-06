//! UI-neutral conversation items folded from the typed Claude protocol. Partial
//! deltas update an existing item. Whole items are the persistence boundary.
mod fold;
pub use fold::Transcript;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemState {
    Pending,
    Streaming,
    Complete,
    Interrupted,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SystemKind {
    Resume,
    Compaction,
    Interruption,
    Exit,
    Failure,
    Warning,
    PermissionRequested,
    PermissionAnswer,
    PermissionCancelled,
    Judgement,
    Result,
    UnfinishedFirstPrompt,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolVerdict {
    pub kind: String,
    pub rule: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolResult {
    pub text: String,
    pub content_json: String,
    pub is_error: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TranscriptContent {
    User {
        text: String,
        error: Option<String>,
    },
    Assistant {
        text: String,
    },
    Thinking {
        text: String,
        signature: String,
    },
    Tool {
        tool_use_id: String,
        name: String,
        input_json: String,
        verdict: Option<ToolVerdict>,
        result: Option<ToolResult>,
    },
    System {
        kind: SystemKind,
        text: String,
        detail_json: String,
    },
    Unknown {
        json: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranscriptItem {
    pub sequence: u64,
    pub cli_session: String,
    pub parent_tool_use_id: Option<String>,
    pub state: ItemState,
    pub content: TranscriptContent,
}

/// Whole items commit before publication. Append changes are transient, bounded
/// by the text arriving in this frame, and fold into the same stable sequence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TranscriptChange {
    Upsert { item: Box<TranscriptItem> },
    AppendText { sequence: u64, text: String },
    AppendThinking { sequence: u64, text: String },
    AppendSignature { sequence: u64, signature: String },
    AppendToolInput { sequence: u64, json: String },
}

impl TranscriptChange {
    pub fn upsert(item: TranscriptItem) -> Self {
        Self::Upsert {
            item: Box::new(item),
        }
    }
}

impl TranscriptItem {
    pub fn apply(&mut self, change: &TranscriptChange) -> bool {
        match (change, &mut self.content) {
            (TranscriptChange::Upsert { item }, _) if self.sequence == item.sequence => {
                *self = item.as_ref().clone();
            }
            (
                TranscriptChange::AppendText { sequence, text },
                TranscriptContent::Assistant { text: current },
            ) if self.sequence == *sequence => current.push_str(text),
            (
                TranscriptChange::AppendThinking { sequence, text },
                TranscriptContent::Thinking { text: current, .. },
            ) if self.sequence == *sequence => current.push_str(text),
            (
                TranscriptChange::AppendSignature {
                    sequence,
                    signature,
                },
                TranscriptContent::Thinking {
                    signature: current, ..
                },
            ) if self.sequence == *sequence => current.push_str(signature),
            (
                TranscriptChange::AppendToolInput { sequence, json },
                TranscriptContent::Tool { input_json, .. },
            ) if self.sequence == *sequence => {
                if input_json == "{}" {
                    input_json.clear();
                }
                input_json.push_str(json);
            }
            _ => return false,
        }
        true
    }
}

#[cfg(test)]
mod tests;

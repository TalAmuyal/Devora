use super::*;
use crate::protocol::{
    Content, ContentBlock, ContentDelta, Extensible, KnownMessage, Message, StreamEvent, Turn,
};
use crate::session::SessionEvent;
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Default)]
struct StreamingMessage {
    id: String,
    blocks: BTreeMap<u64, u64>,
}

/// One ordered conversation, including earlier CLI identities. Only the current
/// message per parent retains stream bookkeeping. No completed frames are kept.
pub struct Transcript {
    items: Vec<TranscriptItem>,
    session: String,
    next_sequence: u64,
    streams: BTreeMap<Option<String>, StreamingMessage>,
}

impl Transcript {
    pub fn new(items: Vec<TranscriptItem>, session: String) -> Self {
        let next_sequence = items.last().map_or(1, |item| item.sequence + 1);
        Self {
            items,
            session,
            next_sequence,
            streams: BTreeMap::new(),
        }
    }
    pub fn items(&self) -> &[TranscriptItem] {
        &self.items
    }
    fn item_mut(&mut self, sequence: u64) -> Option<&mut TranscriptItem> {
        let index = self
            .items
            .binary_search_by_key(&sequence, |item| item.sequence)
            .ok()?;
        self.items.get_mut(index)
    }
    pub fn set_session(&mut self, session: String) {
        self.session = session;
        self.streams.clear();
    }

    fn insert(
        &mut self,
        content: TranscriptContent,
        state: ItemState,
        parent: Option<String>,
    ) -> TranscriptChange {
        let item = TranscriptItem {
            sequence: self.next_sequence,
            cli_session: self.session.clone(),
            parent_tool_use_id: parent,
            state,
            content,
        };
        self.next_sequence += 1;
        self.items.push(item.clone());
        TranscriptChange::upsert(item)
    }

    pub fn system(
        &mut self,
        kind: SystemKind,
        text: impl Into<String>,
        detail: Value,
    ) -> TranscriptChange {
        self.insert(
            TranscriptContent::System {
                kind,
                text: text.into(),
                detail_json: detail.to_string(),
            },
            ItemState::Complete,
            None,
        )
    }

    pub fn submit(&mut self, text: String) -> TranscriptChange {
        self.insert(
            TranscriptContent::User { text, error: None },
            ItemState::Pending,
            None,
        )
    }

    pub fn intention(&mut self, kind: SystemKind, text: String, detail: Value) -> TranscriptChange {
        self.insert(
            TranscriptContent::System {
                kind,
                text,
                detail_json: detail.to_string(),
            },
            ItemState::Pending,
            None,
        )
    }

    pub fn settle(&mut self, sequence: u64, error: Option<String>) -> Option<TranscriptChange> {
        let item = self.item_mut(sequence)?;
        item.state = if error.is_some() {
            ItemState::Failed
        } else {
            ItemState::Complete
        };
        match &mut item.content {
            TranscriptContent::User { error: current, .. } => *current = error,
            TranscriptContent::System { detail_json, .. } => {
                let mut detail: Value =
                    serde_json::from_str(detail_json).unwrap_or_else(|_| json!({}));
                detail["error"] = json!(error);
                *detail_json = detail.to_string();
            }
            _ => {}
        }
        Some(TranscriptChange::upsert(item.clone()))
    }

    /// Seal unfinished UI items on pause/crash. A fresh first conversation also
    /// gets an explicit marker identifying the prompt absent from the new CLI.
    pub fn interrupt(&mut self, fresh_first: bool) -> Vec<TranscriptChange> {
        let mut changes = vec![];
        for item in &mut self.items {
            if item.cli_session == self.session
                && matches!(item.state, ItemState::Pending | ItemState::Streaming)
            {
                item.state = ItemState::Interrupted;
                changes.push(TranscriptChange::upsert(item.clone()));
            }
        }
        if fresh_first
            && self.items.iter().any(|i| {
                i.cli_session == self.session && matches!(i.content, TranscriptContent::User { .. })
            })
            && !self.items.iter().any(|i| {
                i.cli_session == self.session
                    && matches!(
                        i.content,
                        TranscriptContent::System {
                            kind: SystemKind::UnfinishedFirstPrompt,
                            ..
                        }
                    )
            })
        {
            changes.push(self.system(SystemKind::UnfinishedFirstPrompt, "The unfinished first turn is retained here. Its prompt is absent from the replacement CLI conversation.", json!({})));
        }
        self.streams.clear();
        changes
    }

    pub fn event(&mut self, event: &SessionEvent) -> Vec<TranscriptChange> {
        match event {
            SessionEvent::Message(message) => self.message(message),
            SessionEvent::Ready {
                session_id,
                resumed,
                ..
            } => {
                self.set_session(session_id.clone());
                vec![self.system(
                    SystemKind::Resume,
                    if *resumed {
                        "Conversation resumed"
                    } else {
                        "Conversation started"
                    },
                    json!({"session_id":session_id,"resumed":resumed}),
                )]
            }
            SessionEvent::ToolJudged {
                tool_use_id,
                verdict,
            } => {
                let value = ToolVerdict {
                    kind: verdict.kind().as_str().into(),
                    rule: verdict.rule().map(|r| r.as_str().into()),
                    detail: format!("{verdict:?}"),
                };
                if let Some(item) = self.items.iter_mut().rev().find(|item| item.cli_session == self.session && matches!(&item.content, TranscriptContent::Tool { tool_use_id: id, .. } if Some(id) == tool_use_id.as_ref())) {
                    if let TranscriptContent::Tool { verdict, .. } = &mut item.content { *verdict = Some(value); }
                    vec![TranscriptChange::upsert(item.clone())]
                } else { vec![self.system(SystemKind::Judgement, "Tool judgement", json!({"tool_use_id":tool_use_id,"verdict":value}))] }
            }
            SessionEvent::PermissionRequested {
                request_id,
                question,
            } => vec![self.system(
                SystemKind::PermissionRequested,
                format!("Permission requested for {}", question.tool_name),
                json!({"request_id":request_id,"question":question}),
            )],
            SessionEvent::PermissionCancelled { request_id } => vec![self.system(
                SystemKind::PermissionCancelled,
                "Permission question withdrawn",
                json!({"request_id":request_id}),
            )],
            SessionEvent::Compacted(metadata) => vec![self.system(
                SystemKind::Compaction,
                "Conversation compacted",
                json!(metadata),
            )],
            SessionEvent::Warning(text) => vec![self.system(SystemKind::Warning, text, json!({}))],
            SessionEvent::Exited { code, restarting } => vec![self.system(
                SystemKind::Exit,
                "Agent process exited",
                json!({"code":code,"restarting":restarting}),
            )],
            SessionEvent::Failed(text) => vec![self.system(SystemKind::Failure, text, json!({}))],
            SessionEvent::Stopped(info) => vec![self.system(
                SystemKind::Interruption,
                "Agent paused",
                json!({"graceful":info.graceful}),
            )],
            SessionEvent::McpRequested {
                request_id,
                server_name,
                message,
            } => vec![self.unknown(
                json!({"request_id":request_id,"server_name":server_name,"message":message}),
                None,
            )],
            SessionEvent::Usage { .. } => vec![],
        }
    }

    fn unknown(&mut self, value: Value, parent: Option<String>) -> TranscriptChange {
        self.insert(
            TranscriptContent::Unknown {
                json: value.to_string(),
            },
            ItemState::Complete,
            parent,
        )
    }

    fn message(&mut self, message: &Message) -> Vec<TranscriptChange> {
        match message {
            Message::Known(KnownMessage::Assistant { message, extra }) => {
                let parent = extra
                    .get("parent_tool_use_id")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                self.whole(message, parent, false)
            }
            Message::Known(KnownMessage::User { message, extra }) => {
                let parent = extra
                    .get("parent_tool_use_id")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                self.whole(message, parent, true)
            }
            Message::Known(KnownMessage::StreamEvent {
                event: Extensible::Known(event),
                extra,
            }) => {
                let parent = extra
                    .get("parent_tool_use_id")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                self.stream(event, parent)
            }
            Message::Known(KnownMessage::Result(result)) => vec![self.system(
                SystemKind::Result,
                if result.is_error {
                    "Turn ended with an error"
                } else {
                    "Turn completed"
                },
                json!(result),
            )],
            Message::Known(KnownMessage::System(_)) => vec![],
            _ => vec![self.unknown(json!(message), None)],
        }
    }

    fn whole(&mut self, turn: &Turn, parent: Option<String>, user: bool) -> Vec<TranscriptChange> {
        if !user {
            if let Some(id) = turn.extra.get("id").and_then(Value::as_str) {
                if self
                    .streams
                    .get(&parent)
                    .is_none_or(|stream| stream.id != id)
                {
                    self.streams.insert(
                        parent.clone(),
                        StreamingMessage {
                            id: id.into(),
                            blocks: BTreeMap::new(),
                        },
                    );
                }
            }
        }
        match &turn.content {
            Content::Text(text) if user => vec![self.insert(
                TranscriptContent::User {
                    text: text.clone(),
                    error: None,
                },
                ItemState::Complete,
                parent,
            )],
            Content::Text(text) => vec![self.block(
                0,
                TranscriptContent::Assistant { text: text.clone() },
                parent,
                ItemState::Complete,
            )],
            Content::Blocks(blocks) => {
                let mut changes = vec![];
                for (index, block) in blocks.iter().enumerate() {
                    match block {
                        Extensible::Known(ContentBlock::ToolResult {
                            tool_use_id,
                            content,
                            extra,
                        }) => {
                            let result = ToolResult {
                                text: content_text(content),
                                content_json: json!(content).to_string(),
                                is_error: extra
                                    .get("is_error")
                                    .and_then(Value::as_bool)
                                    .unwrap_or(false),
                            };
                            if let Some(item) = self.items.iter_mut().rev().find(|i| i.cli_session == self.session && matches!(&i.content, TranscriptContent::Tool { tool_use_id: id, .. } if id == tool_use_id)) {
                                if let TranscriptContent::Tool { result: current, .. } = &mut item.content { *current = Some(result); }
                                changes.push(TranscriptChange::upsert(item.clone()));
                            } else { changes.push(self.unknown(json!(block), parent.clone())); }
                        }
                        Extensible::Known(ContentBlock::Text { text, .. }) if user => {
                            changes.push(self.insert(
                                TranscriptContent::User {
                                    text: text.clone(),
                                    error: None,
                                },
                                ItemState::Complete,
                                parent.clone(),
                            ))
                        }
                        _ => {
                            let content = block_content(block);
                            let index = if blocks.len() == 1 {
                                self.streams
                                    .get(&parent)
                                    .and_then(|stream| {
                                        stream
                                            .blocks
                                            .iter()
                                            .find(|(_, seq)| {
                                                self.items.iter().any(|item| {
                                                    item.sequence == **seq
                                                        && item.state == ItemState::Streaming
                                                        && same_block(&item.content, &content)
                                                })
                                            })
                                            .map(|(index, _)| *index)
                                    })
                                    .unwrap_or(index as u64)
                            } else {
                                index as u64
                            };
                            changes.push(self.block(
                                index,
                                content,
                                parent.clone(),
                                ItemState::Complete,
                            ));
                        }
                    }
                }
                changes
            }
            Content::Raw(value) => vec![self.unknown(value.clone(), parent)],
        }
    }

    fn block(
        &mut self,
        index: u64,
        content: TranscriptContent,
        parent: Option<String>,
        state: ItemState,
    ) -> TranscriptChange {
        let existing = self
            .streams
            .get(&parent)
            .and_then(|s| s.blocks.get(&index))
            .copied();
        if let Some(sequence) = existing {
            if let Some(item) = self.item_mut(sequence) {
                // Preserve tool annotations when the final message replaces its streamed input.
                let content = match (content, &item.content) {
                    (
                        TranscriptContent::Tool {
                            tool_use_id,
                            name,
                            input_json,
                            ..
                        },
                        TranscriptContent::Tool {
                            verdict, result, ..
                        },
                    ) => TranscriptContent::Tool {
                        tool_use_id,
                        name,
                        input_json,
                        verdict: verdict.clone(),
                        result: result.clone(),
                    },
                    (content, _) => content,
                };
                item.content = content;
                item.state = state;
                return TranscriptChange::upsert(item.clone());
            }
        }
        let change = self.insert(content, state, parent.clone());
        if let TranscriptChange::Upsert { item } = &change {
            self.streams
                .entry(parent)
                .or_default()
                .blocks
                .insert(index, item.sequence);
        }
        change
    }

    fn stream(&mut self, event: &StreamEvent, parent: Option<String>) -> Vec<TranscriptChange> {
        match event {
            StreamEvent::MessageStart { message, .. } => {
                self.streams.insert(
                    parent,
                    StreamingMessage {
                        id: message
                            .extra
                            .get("id")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .into(),
                        blocks: BTreeMap::new(),
                    },
                );
                vec![]
            }
            StreamEvent::ContentBlockStart {
                index,
                content_block,
                ..
            } => vec![self.block(
                *index,
                block_content(content_block),
                parent,
                ItemState::Streaming,
            )],
            StreamEvent::ContentBlockDelta { index, delta, .. } => {
                let Some(sequence) = self
                    .streams
                    .get(&parent)
                    .and_then(|s| s.blocks.get(index))
                    .copied()
                else {
                    return vec![self.unknown(json!(event), parent)];
                };
                let change = match delta {
                    Extensible::Known(ContentDelta::TextDelta { text, .. }) => {
                        TranscriptChange::AppendText {
                            sequence,
                            text: text.clone(),
                        }
                    }
                    Extensible::Known(ContentDelta::ThinkingDelta { thinking, .. }) => {
                        TranscriptChange::AppendThinking {
                            sequence,
                            text: thinking.clone(),
                        }
                    }
                    Extensible::Known(ContentDelta::SignatureDelta { signature, .. }) => {
                        TranscriptChange::AppendSignature {
                            sequence,
                            signature: signature.clone(),
                        }
                    }
                    Extensible::Known(ContentDelta::InputJsonDelta { partial_json, .. }) => {
                        TranscriptChange::AppendToolInput {
                            sequence,
                            json: partial_json.clone(),
                        }
                    }
                    Extensible::Raw(value) => return vec![self.unknown(value.clone(), parent)],
                };
                if !self
                    .item_mut(sequence)
                    .is_some_and(|item| item.apply(&change))
                {
                    return vec![self.unknown(json!(event), parent)];
                }
                vec![change]
            }
            StreamEvent::ContentBlockStop { index, .. } => self
                .streams
                .get(&parent)
                .and_then(|s| s.blocks.get(index))
                .and_then(|sequence| self.items.iter().find(|i| i.sequence == *sequence))
                .map(|item| vec![TranscriptChange::upsert(item.clone())])
                .unwrap_or_default(),
            StreamEvent::MessageDelta { .. } | StreamEvent::MessageStop { .. } => vec![],
        }
    }
}

fn block_content(block: &Extensible<ContentBlock>) -> TranscriptContent {
    match block {
        Extensible::Known(ContentBlock::Text { text, .. }) => {
            TranscriptContent::Assistant { text: text.clone() }
        }
        Extensible::Known(ContentBlock::Thinking { thinking, extra }) => {
            TranscriptContent::Thinking {
                text: thinking.clone(),
                signature: extra
                    .get("signature")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .into(),
            }
        }
        Extensible::Known(ContentBlock::ToolUse {
            id, name, input, ..
        }) => TranscriptContent::Tool {
            tool_use_id: id.clone(),
            name: name.clone(),
            input_json: input.to_string(),
            verdict: None,
            result: None,
        },
        _ => TranscriptContent::Unknown {
            json: json!(block).to_string(),
        },
    }
}

fn content_text(content: &Content) -> String {
    match content {
        Content::Text(text) => text.clone(),
        Content::Blocks(blocks) => blocks
            .iter()
            .filter_map(|b| match b {
                Extensible::Known(ContentBlock::Text { text, .. }) => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Content::Raw(_) => String::new(),
    }
}

fn same_block(left: &TranscriptContent, right: &TranscriptContent) -> bool {
    match (left, right) {
        (TranscriptContent::Assistant { .. }, TranscriptContent::Assistant { .. })
        | (TranscriptContent::Thinking { .. }, TranscriptContent::Thinking { .. }) => true,
        (
            TranscriptContent::Tool {
                tool_use_id: left, ..
            },
            TranscriptContent::Tool {
                tool_use_id: right, ..
            },
        ) => left == right,
        _ => false,
    }
}

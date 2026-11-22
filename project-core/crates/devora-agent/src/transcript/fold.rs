use super::*;
use crate::protocol::{
    Content, ContentBlock, ContentDelta, Extensible, KnownMessage, Message, StreamEvent, System,
    Turn,
};
use crate::session::SessionEvent;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
struct StreamingMessage {
    id: String,
    /// The model writing it, from its `message_start`.
    model: Option<String>,
    active: bool,
    blocks: BTreeMap<u64, u64>,
}

/// Mutable working items for the current CLI identity. Durable history and the
/// bounded UI tail are owned separately. Release settled items after persistence.
pub struct Transcript {
    items: Vec<TranscriptItem>,
    session: String,
    next_sequence: u64,
    streams: BTreeMap<Option<String>, StreamingMessage>,
    pending_commands: BTreeSet<u64>,
    has_prompt: bool,
    marked_first_prompt: bool,
    /// The model of the latest reply in the agent's own conversation (not a subagent's).
    latest_model: Option<String>,
    /// A fallback the CLI announced this turn, waiting for the reply it names: `(fallback_model, content)`.
    fallback: Option<(String, String)>,
    /// The CLI's sentence for the fallback the latest model came from, while replies still come from it.
    fallback_note: Option<String>,
    /// Devora asked the CLI to interrupt the running turn (the user's Stop, a pause), so the result ending it is a stop,
    /// which Claude Code reports as an error (`error_during_execution`), not a failure. The next result consumes it.
    stop_requested: bool,
}

impl Transcript {
    pub fn new(items: Vec<TranscriptItem>, session: String) -> Self {
        let next_sequence = items.last().map_or(1, |item| item.sequence + 1);
        let has_prompt = items.iter().any(|item| {
            item.cli_session == session && matches!(item.content, TranscriptContent::User { .. })
        });
        let marked_first_prompt = items.iter().any(|item| {
            item.cli_session == session
                && matches!(
                    item.content,
                    TranscriptContent::System {
                        kind: SystemKind::UnfinishedFirstPrompt,
                        ..
                    }
                )
        });
        let latest_model = latest_model(&items);
        Self::restore(
            items,
            session,
            next_sequence,
            has_prompt,
            marked_first_prompt,
            latest_model,
        )
    }

    pub fn restore(
        items: Vec<TranscriptItem>,
        session: String,
        next_sequence: u64,
        has_prompt: bool,
        marked_first_prompt: bool,
        latest_model: Option<String>,
    ) -> Self {
        Self {
            items,
            session,
            next_sequence,
            streams: BTreeMap::new(),
            pending_commands: BTreeSet::new(),
            has_prompt,
            marked_first_prompt,
            latest_model,
            fallback: None,
            stop_requested: false,
            fallback_note: None,
        }
    }

    /// Call only after whole-item changes have committed. In-flight command
    /// outcomes can still settle after interruption, so retain those explicitly.
    pub fn release_completed(&mut self) {
        self.items.retain(|item| {
            self.pending_commands.contains(&item.sequence)
                || matches!(item.state, ItemState::Pending | ItemState::Streaming)
                || matches!(
                    (&item.state, &item.content),
                    (
                        ItemState::Complete,
                        TranscriptContent::Tool { result: None, .. }
                    )
                )
        });
        let items = &self.items;
        self.streams.retain(|_, stream| {
            stream.blocks.retain(|_, sequence| {
                items
                    .binary_search_by_key(sequence, |item| item.sequence)
                    .is_ok()
            });
            stream.active || !stream.blocks.is_empty()
        });
    }

    pub fn items(&self) -> &[TranscriptItem] {
        &self.items
    }

    /// The model of the latest reply in the agent's own conversation, if one is known.
    pub fn latest_model(&self) -> Option<&str> {
        self.latest_model.as_deref()
    }

    /// Continues a conversation whose stored replies name no model (they predate keeping it) from `model`, the latest
    /// reply's as Claude's saved session has it.
    pub fn continue_from(&mut self, model: String) {
        self.latest_model.get_or_insert(model);
    }

    /// The CLI's sentence for the fallback the latest model came from, while replies still come from it.
    pub fn fallback_note(&self) -> Option<&str> {
        self.fallback_note.as_deref()
    }
    fn item_mut(&mut self, sequence: u64) -> Option<&mut TranscriptItem> {
        let index = self
            .items
            .binary_search_by_key(&sequence, |item| item.sequence)
            .ok()?;
        self.items.get_mut(index)
    }
    fn item(&self, sequence: u64) -> Option<&TranscriptItem> {
        let index = self
            .items
            .binary_search_by_key(&sequence, |item| item.sequence)
            .ok()?;
        self.items.get(index)
    }

    fn tool_mut(&mut self, expected: &str) -> Option<&mut TranscriptItem> {
        let session = self.session.as_str();
        self.items.iter_mut().rev().find(|item| {
            item.cli_session == session
                && match &item.content {
                    TranscriptContent::Tool {
                        tool_use_id,
                        result: None,
                        ..
                    } => tool_use_id == expected,
                    _ => false,
                }
        })
    }
    pub fn set_session(&mut self, session: String) {
        if self.session != session {
            self.has_prompt = false;
            self.marked_first_prompt = false;
            self.items
                .retain(|item| self.pending_commands.contains(&item.sequence));
        }
        self.session = session;
        self.streams.clear();
        self.fallback = None;
    }

    fn insert(
        &mut self,
        content: TranscriptContent,
        state: ItemState,
        parent: Option<String>,
    ) -> TranscriptChange {
        self.has_prompt |= matches!(content, TranscriptContent::User { .. });
        self.marked_first_prompt |= matches!(
            content,
            TranscriptContent::System {
                kind: SystemKind::UnfinishedFirstPrompt,
                ..
            }
        );
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
        self.pending_commands.insert(self.next_sequence);
        self.insert(
            TranscriptContent::User { text, error: None },
            ItemState::Pending,
            None,
        )
    }

    pub fn intention(&mut self, kind: SystemKind, text: String, detail: Value) -> TranscriptChange {
        if kind == SystemKind::Interruption {
            self.expect_stop();
        }
        self.pending_commands.insert(self.next_sequence);
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

    /// Devora is interrupting the running turn itself: the error result that ends it is a stop ("Turn stopped", with
    /// `stopped` in its detail), not a failure. Recording an interruption's intention does this too.
    pub fn expect_stop(&mut self) {
        self.stop_requested = true;
    }

    pub fn settle(&mut self, sequence: u64, error: Option<String>) -> Option<TranscriptChange> {
        self.pending_commands.remove(&sequence);
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
        self.fallback = None;
        let mut changes = vec![];
        for item in &mut self.items {
            if item.cli_session == self.session
                && (matches!(item.state, ItemState::Pending | ItemState::Streaming)
                    || matches!(item.content, TranscriptContent::Tool { result: None, .. }))
            {
                item.state = ItemState::Interrupted;
                changes.push(TranscriptChange::upsert(item.clone()));
            }
        }
        if fresh_first && self.has_prompt && !self.marked_first_prompt {
            changes.push(self.system(SystemKind::UnfinishedFirstPrompt, "The unfinished first turn is retained here. Its prompt is absent from the replacement CLI conversation.", json!({})));
        }
        self.streams.clear();
        changes
    }

    pub fn event(&mut self, event: &SessionEvent) -> Vec<TranscriptChange> {
        // A waiting fallback belongs to the turn that announced it; a session ending ends that turn.
        if matches!(
            event,
            SessionEvent::Exited { .. } | SessionEvent::Failed(_) | SessionEvent::Stopped(_)
        ) {
            self.fallback = None;
        }
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
                if let Some(item) = tool_use_id.as_deref().and_then(|id| self.tool_mut(id)) {
                    if let TranscriptContent::Tool { verdict, .. } = &mut item.content {
                        *verdict = Some(value);
                    }
                    vec![TranscriptChange::upsert(item.clone())]
                } else {
                    vec![self.system(
                        SystemKind::Judgement,
                        "Tool judgement",
                        json!({"tool_use_id":tool_use_id,"verdict":value}),
                    )]
                }
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
            SessionEvent::Usage { .. } | SessionEvent::Spawned(_) => vec![],
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
            Message::Known(KnownMessage::System(System::ModelFallback {
                fallback_model,
                content,
                ..
            })) => {
                self.fallback = Some((fallback_model.clone(), content.clone()));
                vec![]
            }
            Message::Known(KnownMessage::Result(result)) => {
                self.fallback = None;
                let stopped = std::mem::take(&mut self.stop_requested) && result.is_error;
                let mut detail = json!(result);
                if stopped {
                    detail["stopped"] = json!(true);
                }
                let text = match (stopped, result.is_error) {
                    (true, _) => "Turn stopped",
                    (false, true) => "Turn ended with an error",
                    (false, false) => "Turn completed",
                };
                vec![self.system(SystemKind::Result, text, detail)]
            }
            Message::Known(KnownMessage::System(_))
            | Message::Known(KnownMessage::ControlRequest { .. })
            | Message::Known(KnownMessage::ControlResponse { .. })
            | Message::Known(KnownMessage::ControlCancelRequest { .. }) => vec![],
            _ => vec![self.unknown(json!(message), None)],
        }
    }

    /// Notes the model of a reply in the agent's own conversation, and returns the row saying it changed when
    /// it differs from the previous reply's. A fallback the CLI announced this turn words the row when it names
    /// this model; a repeated fallback to the model already replying adds nothing.
    fn reply_model(&mut self, model: Option<&str>) -> Option<TranscriptChange> {
        let model = model.filter(|model| crate::protocol::is_model(model))?;
        let previous = self.latest_model.replace(model.to_owned());
        if previous.as_deref() == Some(model) {
            return None;
        }
        // The first reply has no row, but a fallback it came from is still what the widget explains.
        let fallback = self.fallback.take().filter(|(to, _)| to == model);
        self.fallback_note = fallback.as_ref().map(|(_, content)| content.clone());
        let previous = previous?;
        let text = match &fallback {
            Some((_, content)) => content.clone(),
            None => format!("Model changed: {previous} → {model}"),
        };
        Some(self.system(
            SystemKind::ModelChanged,
            text,
            json!({"from": previous, "to": model, "fallback": fallback.is_some()}),
        ))
    }

    fn whole(&mut self, turn: &Turn, parent: Option<String>, user: bool) -> Vec<TranscriptChange> {
        let model = (!user)
            .then(|| turn.extra.get("model").and_then(Value::as_str))
            .flatten()
            .map(str::to_owned);
        let mut changed = None;
        if !user {
            let id = turn.extra.get("id").and_then(Value::as_str);
            let matches_stream = self.streams.get(&parent).is_some_and(|stream| match id {
                Some(id) => stream.id == id,
                None => stream.blocks.values().any(|sequence| {
                    self.item(*sequence)
                        .is_some_and(|item| item.state == ItemState::Streaming)
                }),
            });
            if !matches_stream {
                if parent.is_none() {
                    changed = self.reply_model(model.as_deref());
                }
                self.streams.insert(
                    parent.clone(),
                    StreamingMessage {
                        id: id.unwrap_or("").into(),
                        model: model.clone(),
                        active: false,
                        blocks: BTreeMap::new(),
                    },
                );
            }
        }
        let made = match &turn.content {
            Content::Text(text) if user => vec![self.insert(
                TranscriptContent::User {
                    text: text.clone(),
                    error: None,
                },
                ItemState::Complete,
                parent,
            )],
            Content::Text(text) => vec![self.complete_block(
                0,
                TranscriptContent::Assistant {
                    text: text.clone(),
                    model: model.clone(),
                },
                parent,
                true,
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
                            if let Some(item) = self.tool_mut(tool_use_id) {
                                if let TranscriptContent::Tool {
                                    result: current, ..
                                } = &mut item.content
                                {
                                    *current = Some(result);
                                }
                                changes.push(TranscriptChange::upsert(item.clone()));
                            } else {
                                changes.push(self.unknown(json!(block), parent.clone()));
                            }
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
                            let content = block_content(block, model.clone());
                            changes.push(self.complete_block(
                                index as u64,
                                content,
                                parent.clone(),
                                blocks.len() == 1,
                            ));
                        }
                    }
                }
                changes
            }
            Content::Raw(value) => vec![self.unknown(value.clone(), parent)],
        };
        changed.into_iter().chain(made).collect()
    }

    fn complete_block(
        &mut self,
        index: u64,
        content: TranscriptContent,
        parent: Option<String>,
        single_block: bool,
    ) -> TranscriptChange {
        let streamed_index = self.streams.get(&parent).and_then(|stream| {
            stream
                .blocks
                .iter()
                .find(|(_, sequence)| {
                    self.item(**sequence).is_some_and(|item| {
                        item.state == ItemState::Streaming && same_block(&item.content, &content)
                    })
                })
                .map(|(index, _)| *index)
        });
        let index = if single_block {
            streamed_index.unwrap_or(index)
        } else {
            index
        };
        self.block(index, content, parent, ItemState::Complete)
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
                let model = message.extra.get("model").and_then(Value::as_str);
                let changed = if parent.is_none() {
                    self.reply_model(model)
                } else {
                    None
                };
                self.streams.insert(
                    parent,
                    StreamingMessage {
                        id: message
                            .extra
                            .get("id")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .into(),
                        model: model.map(str::to_owned),
                        active: true,
                        blocks: BTreeMap::new(),
                    },
                );
                changed.into_iter().collect()
            }
            StreamEvent::ContentBlockStart {
                index,
                content_block,
                ..
            } => vec![self.block(
                *index,
                block_content(
                    content_block,
                    self.streams.get(&parent).and_then(|s| s.model.clone()),
                ),
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
                .and_then(|sequence| self.item(*sequence))
                .map(|item| vec![TranscriptChange::upsert(item.clone())])
                .unwrap_or_default(),
            StreamEvent::MessageStop { .. } => {
                if let Some(stream) = self.streams.get_mut(&parent) {
                    stream.active = false;
                }
                vec![]
            }
            StreamEvent::MessageDelta { .. } => vec![],
        }
    }
}

/// The model of the latest reply in the agent's own conversation among `items`.
fn latest_model(items: &[TranscriptItem]) -> Option<String> {
    items.iter().rev().find_map(|item| match &item.content {
        TranscriptContent::Assistant { model, .. } if item.parent_tool_use_id.is_none() => {
            model.clone()
        }
        _ => None,
    })
}

/// A block's item; `model` is the model writing the message it belongs to.
fn block_content(block: &Extensible<ContentBlock>, model: Option<String>) -> TranscriptContent {
    match block {
        Extensible::Known(ContentBlock::Text { text, .. }) => TranscriptContent::Assistant {
            text: text.clone(),
            model,
        },
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

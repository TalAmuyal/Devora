//! Devora owns compaction (ADR-012): near the threshold, the agent saves what
//! matters to `memory/`, then Devora compacts with `/compact`. The CLI's own
//! auto-compaction is off; if it compacts anyway, the agent refreshes `memory/`.
//!
//! A pure state machine over the session's usage, results and compaction
//! boundaries; the space actor sends the turns it asks for.
use devora_agent::protocol::{CompactMetadata, TurnResult};
use serde_json::Value;

pub const DEFAULT_THRESHOLD: u8 = 80;

/// How full the context is, as the chat's context bar shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Context {
    pub used_tokens: u64,
    /// Zero until the first result reports the model's context window.
    pub window_tokens: u64,
    pub threshold_percent: u8,
}

/// A turn Devora sends on the agent's behalf.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    SaveMemory(String),
    Compact,
    RefreshMemory(String),
}

impl Step {
    pub fn prompt(&self) -> String {
        match self {
            Self::SaveMemory(text) | Self::RefreshMemory(text) => text.clone(),
            Self::Compact => "/compact".into(),
        }
    }

    /// What the transcript says Devora did, before the turn it sends.
    pub fn announcement(&self, context: &Context) -> String {
        match self {
            Self::SaveMemory(_) => format!(
                "The context is {}% full: Devora asks the agent to save its memory, then compacts.",
                context.percent()
            ),
            Self::Compact => "Devora compacts the conversation.".into(),
            Self::RefreshMemory(_) => {
                "Claude compacted the conversation itself: Devora asks the agent to refresh its memory."
                    .into()
            }
        }
    }
}

impl Context {
    pub fn percent(&self) -> u64 {
        (self.used_tokens * 100)
            .checked_div(self.window_tokens)
            .unwrap_or(0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    Idle,
    SavingMemory,
    Compacting,
    /// The CLI compacted on its own; the refresh goes after the current turn.
    NeedsRefresh,
    RefreshingMemory,
}

pub struct Compaction {
    context: Context,
    stage: Stage,
}

impl Compaction {
    pub fn new(threshold_percent: u8) -> Self {
        Self {
            context: Context {
                used_tokens: 0,
                window_tokens: 0,
                threshold_percent,
            },
            stage: Stage::Idle,
        }
    }

    pub fn context(&self) -> Context {
        self.context
    }

    /// A message's usage: everything the request sent, plus what it produced,
    /// is what the next request carries.
    pub fn usage(&mut self, usage: &Value) -> bool {
        let tokens = |key: &str| usage.get(key).and_then(Value::as_u64).unwrap_or(0);
        let used = tokens("input_tokens")
            + tokens("cache_creation_input_tokens")
            + tokens("cache_read_input_tokens")
            + tokens("output_tokens");
        if used == 0 || used == self.context.used_tokens {
            return false;
        }
        self.context.used_tokens = used;
        true
    }

    /// The end of a turn: learns the context window, and moves the compaction on.
    /// Returns the turn to send next, if any.
    pub fn result(&mut self, result: &TurnResult) -> Option<Step> {
        if let Some(window) = context_window(result) {
            self.context.window_tokens = window;
        }
        if result.is_error {
            // An interrupted or failed step abandons the compaction; the next
            // result over the threshold starts it again.
            self.stage = Stage::Idle;
            return None;
        }
        match self.stage {
            Stage::Idle if self.over_threshold() => {
                self.stage = Stage::SavingMemory;
                Some(Step::SaveMemory(SAVE_MEMORY.into()))
            }
            Stage::SavingMemory => {
                self.stage = Stage::Compacting;
                Some(Step::Compact)
            }
            Stage::NeedsRefresh => {
                self.stage = Stage::RefreshingMemory;
                Some(Step::RefreshMemory(REFRESH_MEMORY.into()))
            }
            Stage::Compacting | Stage::RefreshingMemory => {
                self.stage = Stage::Idle;
                None
            }
            Stage::Idle => None,
        }
    }

    /// A compaction boundary: Devora's own `/compact`, or one the CLI made itself,
    /// after which the agent refreshes its memory once the current turn ends.
    /// `post_tokens` is how full the context is now.
    pub fn compacted(&mut self, metadata: &CompactMetadata) {
        self.context.used_tokens = metadata
            .extra
            .get("post_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        if self.stage != Stage::Compacting {
            self.stage = Stage::NeedsRefresh;
        }
    }

    /// The process was replaced or stopped: any step in flight is gone.
    pub fn reset(&mut self) {
        self.stage = Stage::Idle;
    }

    fn over_threshold(&self) -> bool {
        self.context.window_tokens > 0
            && self.context.used_tokens * 100
                >= self.context.window_tokens * u64::from(self.context.threshold_percent)
    }
}

const SAVE_MEMORY: &str = "Devora: your context is nearly full, and Devora will compact the conversation after this turn. Before that, save to memory/ everything you will need afterwards: the current task and its state, decisions and their reasons, open questions, and your next steps. Reply briefly once it is saved.";

const REFRESH_MEMORY: &str = "Devora: the conversation was just compacted. Update memory/ from what you still know, so that nothing important is lost. Reply briefly once it is updated.";

/// The largest window among the models the turn used (`modelUsage`).
fn context_window(result: &TurnResult) -> Option<u64> {
    result
        .extra
        .get("modelUsage")?
        .as_object()?
        .values()
        .filter_map(|model| model.get("contextWindow")?.as_u64())
        .max()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn result(is_error: bool) -> TurnResult {
        serde_json::from_value(json!({
            "subtype": if is_error { "error_during_execution" } else { "success" },
            "session_id": "s", "is_error": is_error, "num_turns": 1,
            "usage": {"input_tokens": 1, "output_tokens": 1},
            "modelUsage": {"haiku": {"contextWindow": 1000}, "opus": {"contextWindow": 200}},
        }))
        .unwrap()
    }

    fn boundary(trigger: &str) -> CompactMetadata {
        serde_json::from_value(json!({"trigger": trigger, "pre_tokens": 900, "post_tokens": 100}))
            .unwrap()
    }

    #[test]
    fn over_the_threshold_saves_memory_then_compacts_then_rests() {
        let mut compaction = Compaction::new(80);
        compaction.usage(
            &json!({"input_tokens": 10, "cache_read_input_tokens": 700, "output_tokens": 50}),
        );
        assert_eq!(
            compaction.result(&result(false)),
            None,
            "760 of 1000 is under 80%"
        );
        assert_eq!(compaction.context().window_tokens, 1000);

        compaction.usage(
            &json!({"input_tokens": 10, "cache_read_input_tokens": 790, "output_tokens": 5}),
        );
        assert!(matches!(
            compaction.result(&result(false)),
            Some(Step::SaveMemory(_))
        ));
        assert_eq!(compaction.result(&result(false)), Some(Step::Compact));
        compaction.compacted(&boundary("manual"));
        assert_eq!(compaction.context().used_tokens, 100);
        assert_eq!(
            compaction.result(&result(false)),
            None,
            "Devora's own compaction needs no refresh"
        );
    }

    #[test]
    fn an_interrupted_step_abandons_the_compaction() {
        let mut compaction = Compaction::new(50);
        compaction.usage(&json!({"input_tokens": 600}));
        assert!(matches!(
            compaction.result(&result(false)),
            Some(Step::SaveMemory(_))
        ));
        assert_eq!(compaction.result(&result(true)), None);
        assert!(
            matches!(compaction.result(&result(false)), Some(Step::SaveMemory(_))),
            "still over the threshold, it starts again"
        );
    }

    #[test]
    fn a_compaction_the_cli_made_itself_refreshes_memory_after_its_turn() {
        let mut compaction = Compaction::new(80);
        compaction.compacted(&boundary("auto"));
        assert!(
            matches!(
                compaction.result(&result(false)),
                Some(Step::RefreshMemory(_))
            ),
            "after the turn it happened in"
        );
        assert_eq!(
            compaction.result(&result(false)),
            None,
            "the refresh turn's own result ends it"
        );
    }
}

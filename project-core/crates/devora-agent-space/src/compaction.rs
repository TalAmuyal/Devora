//! Devora owns compaction (ADR-012): near the threshold, the agent saves what
//! matters to `memory/`, then Devora compacts with `/compact`. The CLI's own
//! auto-compaction is off; if it compacts anyway, the agent refreshes `memory/`.
//! When the context is already too full for the save-memory turn, Devora
//! compacts without it, and the agent refreshes `memory/` afterwards.
//!
//! A pure state machine over the session's usage, results and compaction
//! boundaries; the space actor sends the turns it asks for.
use devora_agent::protocol::{CompactMetadata, TurnResult};
use devora_agent::transcript::SystemKind;
use serde_json::Value;

pub use devora_config::agent::DEFAULT_COMPACT_THRESHOLD as DEFAULT_THRESHOLD;

/// How full the context is, as the chat's context bar shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Context {
    pub used_tokens: u64,
    /// Zero until the first result reports the model's context window.
    pub window_tokens: u64,
    /// This agent's threshold: its own, when it has one, else the profile's.
    pub threshold_percent: u8,
    /// The profile's threshold, which the agent follows without one of its own.
    pub default_threshold_percent: u8,
    /// The agent's own threshold, as stored, even when it equals the profile's.
    pub own_threshold_percent: Option<u8>,
    pub progress: Progress,
}

/// Where a compaction is, as far as the user can act on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Progress {
    Idle,
    /// The user asked for one, which starts when the running turn ends.
    Requested,
    /// Its turns are running (saving memory, compacting, refreshing memory).
    Underway,
}

/// Why the agent saves its memory before a compaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    Threshold,
    Requested,
}

/// Whether the agent saved its memory before Devora compacts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Memory {
    Saved,
    /// The save-memory turn was refused: the context was too full for it.
    TooFullToSave,
}

/// Why the agent refreshes its memory after a compaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refresh {
    /// Claude Code compacted the conversation itself.
    ClaudeCompacted,
    /// Devora compacted it before the agent could save its memory.
    NotSavedFirst,
}

/// A turn Devora sends on the agent's behalf.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    SaveMemory(Trigger),
    Compact(Memory),
    RefreshMemory(Refresh),
}

/// What the end of a turn asks of the space.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Next {
    Send(Step),
    /// Devora's `/compact` failed: the transcript says so, and what to try, in a
    /// failure row, which the chat always shows.
    CompactionFailed,
}

/// What the transcript says when Devora's `/compact` failed.
pub const COMPACTION_FAILED: &str = "Devora couldn't compact the conversation. Press Compact Now to try again, or send /compact as a prompt.";

impl Step {
    pub fn prompt(&self) -> String {
        match self {
            Self::SaveMemory(Trigger::Threshold) => SAVE_MEMORY.into(),
            Self::SaveMemory(Trigger::Requested) => SAVE_MEMORY_REQUESTED.into(),
            Self::RefreshMemory(_) => REFRESH_MEMORY.into(),
            Self::Compact(_) => "/compact".into(),
        }
    }

    /// The kind of row its announcement is: a system message, except that compacting
    /// without the save is a warning, which the chat shows whether or not it shows
    /// system messages, right after the error that caused it.
    pub fn row_kind(&self) -> SystemKind {
        match self {
            Self::Compact(Memory::TooFullToSave) => SystemKind::Warning,
            _ => SystemKind::Compaction,
        }
    }

    /// What the transcript says Devora did, before the turn it sends.
    pub fn announcement(&self, context: &Context) -> String {
        match self {
            Self::SaveMemory(Trigger::Threshold) => format!(
                "The context is {}% full: Devora asks the agent to save its memory, then compacts.",
                context.percent()
            ),
            Self::SaveMemory(Trigger::Requested) => format!(
                "Compaction requested at {}% full: Devora asks the agent to save its memory, then compacts.",
                context.percent()
            ),
            Self::Compact(Memory::Saved) => "Devora compacts the conversation.".into(),
            Self::Compact(Memory::TooFullToSave) => "The context is too full for the agent to save its memory first: Devora compacts the conversation now, and asks the agent to refresh its memory afterwards.".into(),
            Self::RefreshMemory(Refresh::ClaudeCompacted) => {
                "Claude compacted the conversation itself: Devora asks the agent to refresh its memory."
                    .into()
            }
            Self::RefreshMemory(Refresh::NotSavedFirst) => {
                "The conversation was compacted before the agent could save its memory: the agent is asked to refresh it."
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
    Compacting(Memory),
    /// The CLI compacted on its own; the refresh goes after the current turn.
    NeedsRefresh,
    RefreshingMemory,
}

pub struct Compaction {
    context: Context,
    stage: Stage,
    /// The user asked for a compaction while a turn ran; it starts at that turn's result.
    requested: bool,
}

impl Compaction {
    /// The agent's own threshold (`own`), if it has one, over its profile's.
    pub fn new(own: Option<u8>, default_threshold_percent: u8) -> Self {
        Self {
            context: Context {
                used_tokens: 0,
                window_tokens: 0,
                threshold_percent: own.unwrap_or(default_threshold_percent),
                default_threshold_percent,
                own_threshold_percent: own,
                progress: Progress::Idle,
            },
            stage: Stage::Idle,
            requested: false,
        }
    }

    pub fn context(&self) -> Context {
        Context {
            progress: match (self.stage, self.requested) {
                (Stage::Idle, false) => Progress::Idle,
                (Stage::Idle, true) => Progress::Requested,
                _ => Progress::Underway,
            },
            ..self.context
        }
    }

    /// The agent's own threshold, or none to follow the profile's; it is checked
    /// when the next turn ends, so lowering it below the current usage starts
    /// nothing by itself.
    pub fn set_threshold(&mut self, own: Option<u8>) {
        self.context.own_threshold_percent = own;
        self.context.threshold_percent = own.unwrap_or(self.context.default_threshold_percent);
    }

    /// The user asks for a compaction with no turn running: its first step, to
    /// send now, unless one is already under way.
    pub fn begin(&mut self) -> Option<Step> {
        if self.stage != Stage::Idle {
            return None;
        }
        self.requested = false;
        self.stage = Stage::SavingMemory;
        Some(Step::SaveMemory(Trigger::Requested))
    }

    /// The user asks for a compaction while a turn runs: it starts when that
    /// turn's result arrives, as reaching the threshold does. False when one is
    /// already under way.
    pub fn request(&mut self) -> bool {
        if self.stage != Stage::Idle {
            return false;
        }
        self.requested = true;
        true
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

    /// The end of a turn Devora sent: learns the context window, and moves the
    /// compaction on. `stopped` says Devora interrupted the turn itself (the user's
    /// Stop), so its error is no failure to report.
    pub fn result(&mut self, result: &TurnResult, stopped: bool) -> Option<Next> {
        if let Some(window) = context_window(result) {
            self.context.window_tokens = window;
        }
        if result.is_error {
            let stage = std::mem::replace(&mut self.stage, Stage::Idle);
            self.requested = false;
            return match stage {
                // The context is too full for any ordinary turn, while `/compact`
                // still works: compact without the save.
                Stage::SavingMemory if result.prompt_too_long() => {
                    self.stage = Stage::Compacting(Memory::TooFullToSave);
                    Some(Next::Send(Step::Compact(Memory::TooFullToSave)))
                }
                Stage::Compacting(_) if !stopped => Some(Next::CompactionFailed),
                // Any other interrupted or failed turn abandons the compaction, and
                // a request with it; the next result over the threshold starts it again.
                _ => None,
            };
        }
        let step = match self.stage {
            Stage::Idle if self.requested => {
                self.requested = false;
                self.stage = Stage::SavingMemory;
                Some(Step::SaveMemory(Trigger::Requested))
            }
            Stage::Idle if self.over_threshold() => {
                self.stage = Stage::SavingMemory;
                Some(Step::SaveMemory(Trigger::Threshold))
            }
            Stage::SavingMemory => {
                self.stage = Stage::Compacting(Memory::Saved);
                Some(Step::Compact(Memory::Saved))
            }
            Stage::NeedsRefresh => {
                self.stage = Stage::RefreshingMemory;
                Some(Step::RefreshMemory(Refresh::ClaudeCompacted))
            }
            Stage::Compacting(Memory::TooFullToSave) => {
                self.stage = Stage::RefreshingMemory;
                Some(Step::RefreshMemory(Refresh::NotSavedFirst))
            }
            Stage::Compacting(Memory::Saved) | Stage::RefreshingMemory => {
                self.stage = Stage::Idle;
                None
            }
            Stage::Idle => None,
        };
        step.map(Next::Send)
    }

    /// The end of a turn the agent's CLI started itself (`ActiveTurn::Agents`): a compaction the user asked for during
    /// it starts now, and an interrupted or failed one drops the request, as at the end of a turn Devora sent. The
    /// threshold isn't checked here yet, so such turns alone never start a compaction (`compact` in `running.rs`).
    pub fn agents_turn_ended(&mut self, result: &TurnResult) -> Option<Step> {
        if let Some(window) = context_window(result) {
            self.context.window_tokens = window;
        }
        if result.is_error {
            self.requested = false;
            return None;
        }
        if self.stage == Stage::Idle && self.requested {
            self.requested = false;
            self.stage = Stage::SavingMemory;
            return Some(Step::SaveMemory(Trigger::Requested));
        }
        None
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
        if !matches!(self.stage, Stage::Compacting(_)) {
            self.stage = Stage::NeedsRefresh;
        }
    }

    /// The process was replaced or stopped: any step in flight is gone.
    pub fn reset(&mut self) {
        self.stage = Stage::Idle;
        self.requested = false;
    }

    fn over_threshold(&self) -> bool {
        self.context.window_tokens > 0
            && self.context.used_tokens * 100
                >= self.context.window_tokens * u64::from(self.context.threshold_percent)
    }
}

const SAVE_MEMORY: &str = "Devora: your context is nearly full, and Devora will compact the conversation after this turn. Before that, save to memory/ everything you will need afterwards: the current task and its state, decisions and their reasons, open questions, and your next steps. Reply briefly once it is saved.";

const SAVE_MEMORY_REQUESTED: &str = "Devora: the user asked to compact the conversation, and Devora will compact it after this turn. Before that, save to memory/ everything you will need afterwards: the current task and its state, decisions and their reasons, open questions, and your next steps. Reply briefly once it is saved.";

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

    /// As the pinned CLI ends a turn the API refused as too long (`session_cli.rs`
    /// asserts `terminal_reason` on it).
    fn too_long() -> TurnResult {
        serde_json::from_value(json!({
            "subtype": "success", "session_id": "s", "is_error": true, "num_turns": 1,
            "usage": {"input_tokens": 0, "output_tokens": 0},
            "result": "Prompt is too long", "terminal_reason": "prompt_too_long",
            "api_error_status": 400, "api_error_type": "invalid_request_error",
            "modelUsage": {"haiku": {"contextWindow": 1000}},
        }))
        .unwrap()
    }

    fn boundary(trigger: &str) -> CompactMetadata {
        serde_json::from_value(json!({"trigger": trigger, "pre_tokens": 900, "post_tokens": 100}))
            .unwrap()
    }

    #[test]
    fn over_the_threshold_saves_memory_then_compacts_then_rests() {
        let mut compaction = Compaction::new(None, 80);
        compaction.usage(
            &json!({"input_tokens": 10, "cache_read_input_tokens": 700, "output_tokens": 50}),
        );
        assert_eq!(
            compaction.result(&result(false), false),
            None,
            "760 of 1000 is under 80%"
        );
        assert_eq!(compaction.context().window_tokens, 1000);

        compaction.usage(
            &json!({"input_tokens": 10, "cache_read_input_tokens": 790, "output_tokens": 5}),
        );
        assert!(matches!(
            compaction.result(&result(false), false),
            Some(Next::Send(Step::SaveMemory(_)))
        ));
        assert_eq!(
            compaction.result(&result(false), false),
            Some(Next::Send(Step::Compact(Memory::Saved)))
        );
        compaction.compacted(&boundary("manual"));
        assert_eq!(compaction.context().used_tokens, 100);
        assert_eq!(
            compaction.result(&result(false), false),
            None,
            "Devora's own compaction needs no refresh"
        );
    }

    #[test]
    fn an_interrupted_step_abandons_the_compaction() {
        let mut compaction = Compaction::new(Some(50), 80);
        compaction.usage(&json!({"input_tokens": 600}));
        assert!(matches!(
            compaction.result(&result(false), false),
            Some(Next::Send(Step::SaveMemory(_)))
        ));
        assert_eq!(compaction.result(&result(true), false), None);
        assert!(
            matches!(
                compaction.result(&result(false), false),
                Some(Next::Send(Step::SaveMemory(_)))
            ),
            "still over the threshold, it starts again"
        );
    }

    #[test]
    fn a_compaction_the_cli_made_itself_refreshes_memory_after_its_turn() {
        let mut compaction = Compaction::new(None, 80);
        compaction.compacted(&boundary("auto"));
        assert!(
            matches!(
                compaction.result(&result(false), false),
                Some(Next::Send(Step::RefreshMemory(Refresh::ClaudeCompacted)))
            ),
            "after the turn it happened in"
        );
        assert_eq!(
            compaction.result(&result(false), false),
            None,
            "the refresh turn's own result ends it"
        );
    }

    #[test]
    fn a_request_with_no_turn_running_starts_at_once_and_says_why() {
        let mut compaction = Compaction::new(None, 80);
        compaction.usage(&json!({"input_tokens": 100}));
        assert_eq!(
            compaction.result(&result(false), false),
            None,
            "10% is under 80%"
        );
        let step = compaction.begin();
        assert_eq!(step, Some(Step::SaveMemory(Trigger::Requested)));
        let step = step.unwrap();
        assert!(step.prompt().contains("the user asked to compact"));
        assert!(step
            .announcement(&compaction.context())
            .starts_with("Compaction requested at 10% full"));
        assert_eq!(compaction.context().progress, Progress::Underway);
        assert_eq!(compaction.begin(), None, "one compaction at a time");
        assert!(!compaction.request(), "nor a second one queued behind it");
        assert_eq!(
            compaction.result(&result(false), false),
            Some(Next::Send(Step::Compact(Memory::Saved)))
        );
        assert_eq!(compaction.result(&result(false), false), None);
        assert_eq!(compaction.context().progress, Progress::Idle);
    }

    #[test]
    fn a_request_during_a_turn_starts_at_its_result_and_an_error_drops_it() {
        let mut compaction = Compaction::new(None, 80);
        assert!(compaction.request());
        assert_eq!(compaction.context().progress, Progress::Requested);
        assert_eq!(
            compaction.result(&result(false), false),
            Some(Next::Send(Step::SaveMemory(Trigger::Requested))),
            "under the threshold, but asked for"
        );

        let mut compaction = Compaction::new(None, 80);
        assert!(compaction.request());
        assert_eq!(compaction.result(&result(true), false), None);
        assert_eq!(compaction.context().progress, Progress::Idle);
        assert_eq!(
            compaction.result(&result(false), false),
            None,
            "the interrupted turn took the request with it"
        );
    }

    #[test]
    fn a_new_threshold_is_checked_when_the_next_turn_ends() {
        let mut compaction = Compaction::new(None, 80);
        compaction.usage(&json!({"input_tokens": 600}));
        assert_eq!(
            compaction.result(&result(false), false),
            None,
            "60% is under 80%"
        );
        compaction.set_threshold(Some(50));
        assert_eq!(compaction.context().threshold_percent, 50);
        assert_eq!(compaction.context().default_threshold_percent, 80);
        assert_eq!(compaction.context().own_threshold_percent, Some(50));
        assert_eq!(
            compaction.context().progress,
            Progress::Idle,
            "lowering it starts nothing by itself"
        );
        assert_eq!(
            compaction.result(&result(false), false),
            Some(Next::Send(Step::SaveMemory(Trigger::Threshold)))
        );
    }

    #[test]
    fn an_own_threshold_equal_to_the_profiles_is_still_the_agents_own() {
        let mut compaction = Compaction::new(None, 80);
        compaction.set_threshold(Some(80));
        let context = compaction.context();
        assert_eq!(context.threshold_percent, 80);
        assert_eq!(
            context.own_threshold_percent,
            Some(80),
            "pinned at the profile's value, not following it"
        );
        compaction.set_threshold(None);
        assert_eq!(compaction.context().own_threshold_percent, None);
        assert_eq!(compaction.context().threshold_percent, 80);
    }

    /// From a save-memory turn the API refused: `/compact` without the save, then the refresh.
    fn falls_back(compaction: &mut Compaction) {
        assert_eq!(
            compaction.result(&too_long(), false),
            Some(Next::Send(Step::Compact(Memory::TooFullToSave)))
        );
        assert_eq!(compaction.context().progress, Progress::Underway);
        compaction.compacted(&boundary("manual"));
        assert_eq!(
            compaction.result(&result(false), false),
            Some(Next::Send(Step::RefreshMemory(Refresh::NotSavedFirst)))
        );
    }

    #[test]
    fn a_save_refused_as_too_long_at_the_threshold_compacts_without_it_then_refreshes() {
        let mut compaction = Compaction::new(None, 80);
        compaction.usage(&json!({"input_tokens": 900}));
        assert_eq!(
            compaction.result(&result(false), false),
            Some(Next::Send(Step::SaveMemory(Trigger::Threshold)))
        );
        falls_back(&mut compaction);
        assert_eq!(compaction.result(&result(false), false), None);
        assert_eq!(compaction.context().progress, Progress::Idle);
    }

    #[test]
    fn a_requested_save_refused_as_too_long_compacts_without_it_whichever_turn_it_waited_for() {
        let mut compaction = Compaction::new(None, 80);
        assert_eq!(
            compaction.begin(),
            Some(Step::SaveMemory(Trigger::Requested))
        );
        falls_back(&mut compaction);

        let mut compaction = Compaction::new(None, 80);
        assert!(
            compaction.request(),
            "flagged during a turn the agent's CLI started itself"
        );
        assert_eq!(
            compaction.agents_turn_ended(&result(false)),
            Some(Step::SaveMemory(Trigger::Requested))
        );
        falls_back(&mut compaction);
    }

    #[test]
    fn the_fallback_happens_once_a_refresh_refused_as_too_long_ends_it() {
        let mut compaction = Compaction::new(None, 80);
        compaction.begin();
        falls_back(&mut compaction);
        assert_eq!(compaction.result(&too_long(), false), None);
        assert_eq!(compaction.context().progress, Progress::Idle);
    }

    #[test]
    fn a_compaction_after_a_crashed_fallback_needs_no_refresh() {
        let mut compaction = Compaction::new(None, 80);
        compaction.begin();
        assert!(matches!(
            compaction.result(&too_long(), false),
            Some(Next::Send(Step::Compact(Memory::TooFullToSave)))
        ));
        compaction.reset();
        compaction.begin();
        assert_eq!(
            compaction.result(&result(false), false),
            Some(Next::Send(Step::Compact(Memory::Saved)))
        );
        compaction.compacted(&boundary("manual"));
        assert_eq!(
            compaction.result(&result(false), false),
            None,
            "the memory was saved this time"
        );
    }

    #[test]
    fn only_a_save_refused_as_too_long_falls_back() {
        let mut compaction = Compaction::new(None, 80);
        compaction.begin();
        assert_eq!(
            compaction.result(&result(true), false),
            None,
            "another failure abandons the compaction"
        );
        assert_eq!(compaction.context().progress, Progress::Idle);
        assert_eq!(
            compaction.result(&too_long(), false),
            None,
            "an ordinary turn refused as too long starts nothing"
        );
    }

    #[test]
    fn a_failed_compact_is_reported_unless_the_user_stopped_it() {
        for memory in [Memory::Saved, Memory::TooFullToSave] {
            let mut compaction = Compaction::new(None, 80);
            compaction.begin();
            let save = match memory {
                Memory::Saved => result(false),
                Memory::TooFullToSave => too_long(),
            };
            assert_eq!(
                compaction.result(&save, false),
                Some(Next::Send(Step::Compact(memory)))
            );
            assert_eq!(
                compaction.result(&result(true), false),
                Some(Next::CompactionFailed)
            );
            assert_eq!(compaction.context().progress, Progress::Idle);
            assert_eq!(
                compaction.begin(),
                Some(Step::SaveMemory(Trigger::Requested)),
                "Compact Now works again"
            );
        }

        let mut compaction = Compaction::new(None, 80);
        compaction.begin();
        compaction.result(&result(false), false);
        assert_eq!(
            compaction.result(&result(true), true),
            None,
            "a stop isn't a failure"
        );
    }
}

//! Agent Space exports. Every async operation runs on the facade-owned agent
//! loop. Subscriptions are observations and never own the running process.
use crate::{CoreError, DevoraCore};
use devora_core::agents::{
    self, CreationForm, Field, LaunchContext, ModelChoice, PermissionAnswer, Scope, Source,
};
use devora_core::state::{
    AgentActivity, AgentPause, Chat, ChatContext, ChatEffort, ChatEvent, ChatModel, ChatPhase,
    ChatStep, ChatUpdate, CompactionProgress, Delivery, ItemState, PauseView, PermissionQuestion,
    RunningAgent, RunningAgents, StepActivity, Subscription, SystemKind, ToolResult, ToolVerdict,
    TranscriptChange, TranscriptContent, TranscriptItem,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(uniffi::Record)]
pub struct AgentCreationForm {
    pub name: String,
    pub role: Option<String>,
    pub repos: Vec<String>,
    pub inputs: HashMap<String, String>,
    pub keep_coding_instructions: bool,
    /// The model and effort for every launch; `None` follows the profile's Claude settings.
    pub model: Option<AgentModelChoice>,
}

/// An agent's model, and its effort; no effort leaves it to Claude Code.
#[derive(uniffi::Record)]
pub struct AgentModelChoice {
    pub model: String,
    pub effort: Option<String>,
}

/// What the New Agent form's model and effort start with, and the efforts it offers (low → max).
#[derive(uniffi::Record)]
pub struct AgentModelDefaults {
    pub choice: AgentModelChoice,
    pub effort_levels: Vec<String>,
}

#[derive(uniffi::Record)]
pub struct AgentPlugin {
    pub name: String,
    pub directory: String,
}

/// Explicit host launch inputs. Tool settings belong in settings_json.env.
/// Environment holds scrubbed platform/auth/proxy transport and stays out of argv.
#[derive(uniffi::Record)]
pub struct AgentLaunch {
    pub plugins: Vec<AgentPlugin>,
    pub settings_json: String,
    pub environment: HashMap<String, String>,
}

impl AgentCreationForm {
    fn core(self) -> CreationForm {
        CreationForm {
            name: self.name,
            role: self.role,
            repos: self.repos,
            inputs: self.inputs.into_iter().collect(),
            keep_coding_instructions: self.keep_coding_instructions,
            model: self.model.map(|choice| ModelChoice {
                model: choice.model,
                effort: choice.effort,
            }),
            command: None,
        }
    }
}

/// One row of a model field (`devora_agent::models::ModelRow`): `value` is what picking it stores (an alias for a tier, a
/// full ID for a model), `model_id` the model it runs today.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ModelRow {
    pub value: String,
    pub label: String,
    pub model_id: String,
}

/// The models Claude Code offers, as every model field shows them (`ModelList::sections`), with the version of the CLI
/// that listed them: the tiers its aliases name, then every model by its version.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct AgentModelList {
    pub cli_version: Option<String>,
    pub aliases: Vec<ModelRow>,
    pub model_ids: Vec<ModelRow>,
}

/// The rows of `list` that match what's `typed` into a model field, by the core's one rule for every field
/// (`ModelSections::narrowed`): a tier by its own name, a model by its name or ID; every row while the field holds exactly
/// a value one of them stores.
#[uniffi::export]
pub fn narrow_models(list: AgentModelList, typed: String) -> Result<AgentModelList, CoreError> {
    let back = |rows: Vec<devora_core::agents::ModelRow>| -> Vec<ModelRow> {
        rows.into_iter()
            .map(|row| ModelRow {
                value: row.value,
                label: row.label,
                model_id: row.model_id,
            })
            .collect()
    };
    let narrowed = sections(list.aliases, list.model_ids).narrowed(&typed);
    Ok(AgentModelList {
        cli_version: list.cli_version,
        aliases: back(narrowed.aliases),
        model_ids: back(narrowed.model_ids),
    })
}

/// The value of the row in `list` that stores exactly what's `typed` into a model field, by the same comparison that makes
/// `narrow_models` show every row (`ModelSections::exact`), so a picker highlights that row. None when it names no row.
#[uniffi::export]
pub fn exact_model(list: AgentModelList, typed: String) -> Result<Option<String>, CoreError> {
    Ok(sections(list.aliases, list.model_ids)
        .exact(&typed)
        .map(|row| row.value.clone()))
}

/// The percentages a compaction threshold takes, inclusive (`agent_config::COMPACT_THRESHOLD_RANGE`), for
/// the controls that set one, so no UI keeps a copy of the range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct PercentRange {
    pub min: u8,
    pub max: u8,
}

#[uniffi::export]
pub fn compact_threshold_range() -> Result<PercentRange, CoreError> {
    let range = devora_core::agent_config::COMPACT_THRESHOLD_RANGE;
    Ok(PercentRange {
        min: *range.start(),
        max: *range.end(),
    })
}

fn sections(
    aliases: Vec<ModelRow>,
    model_ids: Vec<ModelRow>,
) -> devora_core::agents::ModelSections {
    let core = |rows: Vec<ModelRow>| -> Vec<devora_core::agents::ModelRow> {
        rows.into_iter()
            .map(|row| devora_core::agents::ModelRow {
                value: row.value,
                label: row.label,
                model_id: row.model_id,
            })
            .collect()
    };
    devora_core::agents::ModelSections {
        aliases: core(aliases),
        model_ids: core(model_ids),
    }
}

/// The name of the model `id` among `models` (a list's `model_ids`, each model once), by the core's rule for model IDs:
/// a dated snapshot is its undated model (`claude-sonnet-4-5-20250929` is `claude-sonnet-4-5`). None when it isn't listed.
#[uniffi::export]
pub fn model_name(models: Vec<ModelRow>, id: String) -> Result<Option<String>, CoreError> {
    Ok(models
        .into_iter()
        .find(|model| devora_core::agents::same_model(&model.model_id, &id))
        .map(|model| model.label))
}

impl AgentLaunch {
    fn context(self, global_config: PathBuf) -> Result<LaunchContext, CoreError> {
        let settings = serde_json::from_str(&self.settings_json)
            .map_err(|e| format!("invalid agent settings: {e}"))?;
        Ok(LaunchContext {
            global_config,
            settings,
            plugins: self
                .plugins
                .into_iter()
                .map(|p| agents::Plugin {
                    name: p.name,
                    directory: p.directory.into(),
                })
                .collect(),
            environment: self.environment.into_iter().collect(),
        })
    }
}

#[uniffi::remote(Enum)]
#[uniffi(name = "AgentFieldSource")]
pub enum Source {
    Launch,
    Default,
    Global,
    Profile,
    Derived,
}

/// One placeholder of the agent template, for the creation form.
#[uniffi::remote(Record)]
#[uniffi(name = "AgentField")]
pub struct Field {
    pub key: String,
    pub value: Option<String>,
    pub source: Option<Source>,
}

#[uniffi::remote(Enum)]
#[uniffi(name = "AgentConfigScope")]
pub enum Scope {
    Global,
    Profile,
}

#[derive(uniffi::Record)]
pub struct AgentSpace {
    pub name: String,
    pub path: String,
    pub role: Option<String>,
    pub error: Option<String>,
}

#[uniffi::remote(Enum)]
#[uniffi(name = "AgentCreationEvent")]
pub enum CreationEvent {
    Step { label: String },
    Log { line: String },
    Done { name: String, path: String },
    Failed { message: String },
    Cancelled,
}
use agents::CreationEvent;

#[derive(uniffi::Object)]
pub struct AgentCreationSubscription {
    inner: Subscription<CreationEvent>,
}
#[uniffi::export]
impl AgentCreationSubscription {
    pub async fn next_batch(&self, max: u32) -> Result<Vec<CreationEvent>, CoreError> {
        Ok(self.inner.next_batch(max).await)
    }
    pub fn close(&self) -> Result<(), CoreError> {
        self.inner.close();
        Ok(())
    }
}

#[derive(uniffi::Record)]
pub struct AgentCreation {
    pub id: u64,
    pub events: Arc<AgentCreationSubscription>,
}

#[uniffi::remote(Enum)]
pub enum ChatPhase {
    Opening,
    Running,
    Pausing,
    Paused,
    Failed,
}
#[uniffi::remote(Enum)]
pub enum ItemState {
    Pending,
    Streaming,
    Complete,
    Interrupted,
    Failed,
}
#[uniffi::remote(Enum)]
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
    RestartNotice,
    ModelChanged,
}
#[uniffi::remote(Record)]
pub struct ToolVerdict {
    pub kind: String,
    pub rule: Option<String>,
    pub detail: String,
}
#[uniffi::remote(Record)]
pub struct ToolResult {
    pub text: String,
    pub content_json: String,
    pub is_error: bool,
}
#[uniffi::remote(Enum)]
pub enum Delivery {
    Waiting,
    NotDelivered,
}
#[uniffi::remote(Enum)]
pub enum TranscriptContent {
    User {
        text: String,
        error: Option<String>,
        delivery: Option<Delivery>,
    },
    Assistant {
        text: String,
        model: Option<String>,
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
#[uniffi::remote(Record)]
pub struct TranscriptItem {
    pub sequence: u64,
    pub cli_session: String,
    pub parent_tool_use_id: Option<String>,
    pub state: ItemState,
    pub content: TranscriptContent,
    pub created_at_ms: Option<u64>,
}
#[uniffi::remote(Enum)]
pub enum TranscriptChange {
    Upsert { item: Box<TranscriptItem> },
    AppendText { sequence: u64, text: String },
    AppendThinking { sequence: u64, text: String },
    AppendSignature { sequence: u64, signature: String },
    AppendToolInput { sequence: u64, json: String },
}
#[uniffi::remote(Record)]
pub struct PermissionQuestion {
    pub request_id: String,
    pub tool_name: String,
    pub tool_use_id: String,
    pub input_json: String,
    pub decision_reason: Option<String>,
    pub can_always_allow: bool,
    pub suggested_rules_json: Option<String>,
}
#[uniffi::remote(Record)]
pub struct Chat {
    pub tab: u64,
    pub generation: u64,
    pub phase: ChatPhase,
    pub turn_running: bool,
    pub agent_turn: bool,
    pub pause: AgentPause,
    pub session_id: Option<String>,
    pub items: Vec<TranscriptItem>,
    pub error: Option<String>,
    pub questions: Vec<PermissionQuestion>,
    pub usage_json: Option<String>,
    pub context: Option<ChatContext>,
    pub model: Option<ChatModel>,
    pub effort: Option<ChatEffort>,
    pub step: Option<ChatStep>,
}
#[uniffi::remote(Enum)]
pub enum AgentPause {
    None,
    ByUser,
    PausedAtOpen,
    Held { keep_paused: bool, released: bool },
}
#[uniffi::remote(Record)]
pub struct PauseView {
    pub pause_requested: bool,
    pub globally_held: bool,
    pub keep_paused: bool,
}
/// The pause widget's view of an agent's pause (`devora_state::pause_view`).
#[uniffi::export]
pub fn pause_view(pause: AgentPause, phase: ChatPhase) -> PauseView {
    devora_core::state::pause_view(&pause, &phase)
}
#[uniffi::remote(Record)]
pub struct ChatStep {
    pub activity: StepActivity,
    pub since_ms: u64,
}
#[uniffi::remote(Enum)]
pub enum StepActivity {
    Waiting {
        tool: String,
        request_id: String,
    },
    Running {
        tool: String,
        tool_use_id: String,
        subagent: bool,
    },
    Thinking {
        subagent: bool,
    },
    Writing {
        subagent: bool,
    },
    Working,
}
#[uniffi::remote(Record)]
pub struct ChatContext {
    pub used_tokens: u64,
    pub window_tokens: u64,
    pub threshold_percent: u8,
    pub default_threshold_percent: u8,
    pub own_threshold_percent: Option<u8>,
    pub compaction: CompactionProgress,
}
#[uniffi::remote(Record)]
pub struct ChatEffort {
    pub set_to: Option<String>,
    pub levels: Option<Vec<String>>,
    pub model_unknown: bool,
    pub runs_as: Option<String>,
    pub pending: bool,
    pub not_taken: Option<String>,
}
#[uniffi::remote(Record)]
pub struct ChatModel {
    pub latest: Option<String>,
    pub label: Option<String>,
    pub set_to: String,
    pub set_to_resolved: Option<String>,
    pub differs: bool,
    pub fallback: Option<String>,
    pub awaiting_change: bool,
    pub change_not_taken: Option<String>,
}
#[uniffi::remote(Enum)]
pub enum CompactionProgress {
    Idle,
    Requested,
    Underway,
}
#[uniffi::remote(Enum)]
pub enum ChatUpdate {
    Pause { pause: AgentPause },
    Turn { running: bool, by_agent: bool },
    Generation { generation: u64 },
    Phase { phase: ChatPhase },
    Ready { session_id: String },
    Restore { items: Vec<TranscriptItem> },
    Transcript { change: TranscriptChange },
    Failure { message: String },
    PermissionRequested { question: PermissionQuestion },
    PermissionRemoved { request_id: String },
    Usage { payload_json: String },
    Context { context: ChatContext },
    Effort { effort: ChatEffort },
    Model { model: ChatModel },
    Step { step: Option<ChatStep> },
}
#[uniffi::remote(Enum)]
pub enum AgentActivity {
    Turn,
    Question,
    Idle,
    Stopping,
}
#[uniffi::remote(Record)]
pub struct RunningAgent {
    pub profile_path: String,
    pub name: String,
    pub tab: Option<u64>,
    pub activity: AgentActivity,
}
#[uniffi::remote(Record)]
pub struct RunningAgents {
    pub total: u32,
    pub running: Vec<RunningAgent>,
}
#[uniffi::remote(Enum)]
pub enum ChatEvent {
    Snapshot {
        chats: Vec<Chat>,
        running: RunningAgents,
    },
    Started {
        chat: Chat,
    },
    Updated {
        tab: u64,
        generation: u64,
        update: ChatUpdate,
    },
    Closed {
        tab: u64,
    },
    Running {
        running: RunningAgents,
    },
}
#[uniffi::remote(Enum)]
pub enum PermissionAnswer {
    Allow,
    Deny { message: String },
    AllowWithSuggestedRule,
}

#[derive(uniffi::Object)]
pub struct ChatSubscription {
    inner: Subscription<ChatEvent>,
}
#[uniffi::export]
impl ChatSubscription {
    pub async fn next_batch(&self, max: u32) -> Result<Vec<ChatEvent>, CoreError> {
        Ok(self.inner.next_batch(max).await)
    }
    pub fn close(&self) -> Result<(), CoreError> {
        self.inner.close();
        Ok(())
    }
}

#[uniffi::export]
impl DevoraCore {
    pub fn create_agent_space(
        &self,
        profile_path: String,
        form: AgentCreationForm,
        launch_inputs: HashMap<String, String>,
    ) -> Result<AgentCreation, CoreError> {
        let creation = self.agents.create(
            profile_path.into(),
            form.core(),
            launch_inputs.into_iter().collect(),
            self.config_path.clone(),
        )?;
        Ok(AgentCreation {
            id: creation.id,
            events: Arc::new(AgentCreationSubscription {
                inner: creation.events,
            }),
        })
    }
    pub fn cancel_agent_creation(&self, id: u64) -> Result<(), CoreError> {
        self.agents.cancel_creation(id);
        Ok(())
    }
    /// The template's fields as `form` would render them: the ones only the form
    /// fills first, then the pre-filled ones, then the derived ones.
    pub fn agent_creation_fields(
        &self,
        profile_path: String,
        form: AgentCreationForm,
        launch_inputs: HashMap<String, String>,
    ) -> Result<Vec<Field>, CoreError> {
        Ok(self.agents.creation_fields(
            Path::new(&profile_path),
            &self.config_path,
            launch_inputs.into_iter().collect(),
            &form.core(),
        )?)
    }
    /// The instructions `form` would create, rendered as creation renders them;
    /// an error says why creation would fail (a missing value, a bad template).
    pub fn agent_creation_preview(
        &self,
        profile_path: String,
        form: AgentCreationForm,
        launch_inputs: HashMap<String, String>,
    ) -> Result<String, CoreError> {
        Ok(self.agents.creation_preview(
            Path::new(&profile_path),
            &self.config_path,
            launch_inputs.into_iter().collect(),
            &form.core(),
        )?)
    }
    /// The model and effort the New Agent form starts with: `opus`, and the
    /// effort set for the profile or in User Defaults, if any.
    pub fn agent_model_defaults(
        &self,
        profile_path: String,
    ) -> Result<AgentModelDefaults, CoreError> {
        let choice = self
            .agents
            .model_defaults(Path::new(&profile_path), &self.config_path)?;
        Ok(AgentModelDefaults {
            choice: AgentModelChoice {
                model: choice.model,
                effort: choice.effort,
            },
            effort_levels: devora_core::config::CLAUDE_EFFORT_LEVELS
                .map(str::to_owned)
                .into(),
        })
    }
    /// Saves a creation-form value to its config, as the Settings Hub would.
    pub fn save_agent_field(
        &self,
        profile_path: String,
        scope: Scope,
        key: String,
        value: String,
    ) -> Result<(), CoreError> {
        self.agents.save_field(
            Path::new(&profile_path),
            &self.config_path,
            scope,
            &key,
            &value,
        )?;
        Ok(())
    }
    pub fn list_agent_spaces(&self, profile_path: String) -> Result<Vec<AgentSpace>, CoreError> {
        Ok(self
            .agents
            .list(Path::new(&profile_path))?
            .into_iter()
            .map(|e| AgentSpace {
                name: e.name,
                path: e.path.to_string_lossy().into_owned(),
                role: e.identity.map(|i| i.role),
                error: e.error,
            })
            .collect())
    }
    /// The launch Devora resolves itself: Claude and the environment from the
    /// user's login shell, with `resources_dir`'s tools and plugins.
    pub async fn resolve_agent_launch(
        &self,
        resources_dir: Option<String>,
    ) -> Result<AgentLaunch, CoreError> {
        let context = self
            .agents
            .resolve_launch(self.config_path.clone(), resources_dir.map(PathBuf::from))
            .await?;
        Ok(AgentLaunch {
            plugins: context
                .plugins
                .into_iter()
                .map(|p| AgentPlugin {
                    name: p.name,
                    directory: p.directory.to_string_lossy().into_owned(),
                })
                .collect(),
            settings_json: context.settings.to_string(),
            environment: context.environment.into_iter().collect(),
        })
    }
    pub async fn open_agent_space(
        &self,
        profile_path: String,
        name: String,
        launch: AgentLaunch,
    ) -> Result<u64, CoreError> {
        Ok(self
            .agents
            .open(
                Path::new(&profile_path),
                &name,
                launch.context(self.config_path.clone())?,
            )
            .await?)
    }
    pub fn pause_agent_after_turn(&self, tab: u64, generation: u64) -> Result<(), CoreError> {
        self.agents.pause_after_turn(tab, generation)?;
        Ok(())
    }
    /// During a global hold: whether the agent stays paused when Devora resumes. Unticking it on an agent
    /// paused before the hold reopens it, held, so that it resumes with the others.
    pub async fn set_agent_keep_paused(
        &self,
        tab: u64,
        generation: u64,
        keep_paused: bool,
    ) -> Result<(), CoreError> {
        self.agents
            .set_keep_paused(tab, generation, keep_paused)
            .await?;
        Ok(())
    }
    pub fn close_agent_space(&self, tab: u64) -> Result<(), CoreError> {
        self.agents.close_tab(tab)?;
        Ok(())
    }
    pub async fn delete_agent_space(
        &self,
        profile_path: String,
        name: String,
    ) -> Result<(), CoreError> {
        self.agents.delete(Path::new(&profile_path), &name).await?;
        Ok(())
    }
    pub async fn send_agent_turn(
        &self,
        tab: u64,
        generation: u64,
        text: String,
    ) -> Result<(), CoreError> {
        self.agents.send_turn(tab, generation, text).await?;
        Ok(())
    }
    /// The models an agent of the profile is offered, from the CLI launched with `launch` (cached per profile and CLI
    /// version, and refreshed daily): its aliases with what each resolves to, then any other models its login offers, with
    /// the CLI's version.
    pub async fn agent_model_options(
        &self,
        profile_path: String,
        launch: AgentLaunch,
    ) -> Result<AgentModelList, CoreError> {
        let context = launch.context(self.config_path.clone())?;
        let list = self
            .agents
            .model_options(Path::new(&profile_path), context)
            .await?;
        let sections = list.sections();
        let rows = |rows: Vec<devora_core::agents::ModelRow>| {
            rows.into_iter()
                .map(|row| ModelRow {
                    value: row.value,
                    label: row.label,
                    model_id: row.model_id,
                })
                .collect()
        };
        Ok(AgentModelList {
            cli_version: list.cli_version,
            aliases: rows(sections.aliases),
            model_ids: rows(sections.model_ids),
        })
    }
    pub async fn interrupt_agent(&self, tab: u64, generation: u64) -> Result<(), CoreError> {
        self.agents.interrupt(tab, generation).await?;
        Ok(())
    }
    /// The agent's own compaction threshold (1 to 100), or nil to follow its profile's.
    pub async fn set_agent_compact_threshold(
        &self,
        tab: u64,
        generation: u64,
        percent: Option<u8>,
    ) -> Result<(), CoreError> {
        self.agents
            .set_compact_threshold(tab, generation, percent)
            .await?;
        Ok(())
    }
    /// Sets the agent's effort (none is Claude Code's default) to a level its model takes; a running agent's CLI takes it
    /// between turns.
    pub async fn set_agent_effort(
        &self,
        tab: u64,
        generation: u64,
        effort: Option<String>,
    ) -> Result<(), CoreError> {
        self.agents.set_effort(tab, generation, effort).await?;
        Ok(())
    }
    /// Sets the agent's model to one its latest launch was offered; a running agent's CLI takes it between turns.
    pub async fn set_agent_model(
        &self,
        tab: u64,
        generation: u64,
        model: String,
    ) -> Result<(), CoreError> {
        self.agents.set_model(tab, generation, model).await?;
        Ok(())
    }
    pub async fn compact_agent_now(&self, tab: u64, generation: u64) -> Result<(), CoreError> {
        self.agents.compact_now(tab, generation).await?;
        Ok(())
    }
    pub async fn answer_agent_permission(
        &self,
        tab: u64,
        generation: u64,
        request_id: String,
        answer: PermissionAnswer,
    ) -> Result<(), CoreError> {
        self.agents
            .answer_permission(tab, generation, request_id, answer)
            .await?;
        Ok(())
    }
    pub async fn agent_history(
        &self,
        profile_path: String,
        name: String,
        after: u64,
        limit: u32,
    ) -> Result<Vec<TranscriptItem>, CoreError> {
        Ok(self
            .agents
            .history(Path::new(&profile_path), &name, after, limit)
            .await?)
    }
    pub async fn agent_history_before(
        &self,
        profile_path: String,
        name: String,
        before: Option<u64>,
        limit: u32,
    ) -> Result<Vec<TranscriptItem>, CoreError> {
        Ok(self
            .agents
            .history_before(Path::new(&profile_path), &name, before, limit)
            .await?)
    }
    pub fn agent_chat_snapshot(&self) -> Result<Vec<Chat>, CoreError> {
        Ok(self.state.chat_snapshot())
    }
    pub fn subscribe_agent_chats(&self) -> Result<Arc<ChatSubscription>, CoreError> {
        Ok(Arc::new(ChatSubscription {
            inner: self.state.subscribe_chats(),
        }))
    }
    /// Stops every agent, interrupting live turns; fails with what went wrong, once every agent has stopped.
    pub async fn shutdown_agents(&self) -> Result<(), CoreError> {
        let problems = self.agents.shutdown().await;
        if problems.is_empty() {
            Ok(())
        } else {
            Err(problems.join("\n").into())
        }
    }
    /// Call after supplying the host launch context on relaunch, before rendering
    /// restored agent tabs. Each failed launch is projected on its existing tab.
    pub async fn restore_agent_spaces(&self, launch: AgentLaunch) -> Result<(), CoreError> {
        let context = launch.context(self.config_path.clone())?;
        self.agents.restore(context).await?;
        Ok(())
    }
}

#[cfg(all(test, unix))]
mod tests;

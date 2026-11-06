//! Agent Space exports. Every async operation runs on the facade-owned agent
//! loop. Subscriptions are observations and never own the running process.
use crate::{CoreError, DevoraCore};
use devora_core::agents::{self, CreationForm, LaunchContext, PermissionAnswer};
use devora_core::state::{
    Chat, ChatEvent, ChatItem, ChatPhase, ChatUpdate, PermissionQuestion, Subscription,
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
    pub program: String,
    pub plugins: Vec<AgentPlugin>,
    pub settings_json: String,
    pub environment: HashMap<String, String>,
}

impl AgentLaunch {
    fn context(self, global_config: PathBuf) -> Result<LaunchContext, CoreError> {
        let settings = serde_json::from_str(&self.settings_json)
            .map_err(|e| format!("invalid agent settings: {e}"))?;
        Ok(LaunchContext {
            global_config,
            program: self.program.into(),
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
#[uniffi::remote(Record)]
pub struct ChatItem {
    pub kind: String,
    pub payload_json: String,
}
#[uniffi::remote(Record)]
pub struct PermissionQuestion {
    pub request_id: String,
    pub payload_json: String,
}
#[uniffi::remote(Record)]
pub struct Chat {
    pub tab: u64,
    pub generation: u64,
    pub phase: ChatPhase,
    pub turn_running: bool,
    pub session_id: Option<String>,
    pub items: Vec<ChatItem>,
    pub questions: Vec<PermissionQuestion>,
    pub usage_json: Option<String>,
}
#[uniffi::remote(Enum)]
pub enum ChatUpdate {
    Turn { running: bool },
    Generation { generation: u64 },
    Phase { phase: ChatPhase },
    Ready { session_id: String },
    Item { item: ChatItem },
    PermissionRequested { question: PermissionQuestion },
    PermissionRemoved { request_id: String },
    Usage { payload_json: String },
}
#[uniffi::remote(Enum)]
pub enum ChatEvent {
    Snapshot {
        chats: Vec<Chat>,
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
            CreationForm {
                name: form.name,
                role: form.role,
                repos: form.repos,
                inputs: form.inputs.into_iter().collect(),
            },
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
    pub async fn pause_agent_space(&self, tab: u64) -> Result<(), CoreError> {
        self.agents.pause(tab).await?;
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
    pub async fn interrupt_agent(&self, tab: u64, generation: u64) -> Result<(), CoreError> {
        self.agents.interrupt(tab, generation).await?;
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
    pub fn agent_chat_snapshot(&self) -> Result<Vec<Chat>, CoreError> {
        Ok(self.state.chat_snapshot())
    }
    pub fn subscribe_agent_chats(&self) -> Result<Arc<ChatSubscription>, CoreError> {
        Ok(Arc::new(ChatSubscription {
            inner: self.state.subscribe_chats(),
        }))
    }
    pub async fn shutdown_agents(&self) -> Result<(), CoreError> {
        self.agents.shutdown().await;
        Ok(())
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

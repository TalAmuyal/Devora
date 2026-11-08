//! The facade every Devora UI binds (ADR-007): a UI depends on this crate alone, never on the crates behind it.
//!
//! For now it re-exports the modules extracted from Ember as-is.
//! The `DevoraCore` object (commands in, events out) replaces these re-exports as the core grows.

pub use devora_app::{bundle, log};
pub use devora_config::{
    agent as agent_config, config, profile, repo_update as repo_update_config,
};
pub use devora_health as health;
pub use devora_ipc as ipc;
pub use devora_platform::credentials;
pub use devora_pty as pty;
pub use devora_state as state;
pub use devora_terminal as terminal;
pub use devora_theme as theme;
pub use devora_workspace::creation as workspace_creation;
pub use devora_workspace::{repo_clone, workspace};

/// Agent Space lifecycle and live chat orchestration behind the facade.
pub mod agents;

pub mod repo_updater;

//! The facade every Devora UI binds (ADR-007): a UI depends on this crate alone, never on the crates behind it.
//!
//! For now it re-exports the modules extracted from Ember as-is.
//! The `DevoraCore` object (commands in, events out) replaces these re-exports as the core grows.

pub use devora_config::{config, profile};
pub use devora_health as health;
pub use devora_platform::credentials;
pub use devora_pty as pty;
pub use devora_terminal as terminal;
pub use devora_theme as theme;
pub use devora_workspace::creation as workspace_creation;
pub use devora_workspace::{repo_clone, workspace};

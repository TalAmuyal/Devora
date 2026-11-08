//! Sapphire's owner of the shared updater. Ember binds it in its Rust adapter.
use crate::{CoreError, DevoraCore};
use devora_core::repo_update_config::{AppKind, Settings};
use devora_core::repo_updater::RepoUpdater;
use devora_core::state::Store;
use std::sync::Arc;

#[uniffi::export]
impl DevoraCore {
    pub fn start_repo_updates(&self) -> Result<(), CoreError> {
        let mut updater = self.updater.lock().unwrap_or_else(|e| e.into_inner());
        if updater.is_none() {
            let settings = Settings::read(&self.config_path, AppKind::Sapphire)?;
            let pause: Arc<Store> = self.state.clone();
            *updater = Some(RepoUpdater::start(
                self.config_path.clone(),
                settings,
                pause,
                self.log.shared(),
            )?);
        }
        Ok(())
    }

    pub fn run_repo_updates(&self) -> Result<(), CoreError> {
        self.start_repo_updates()?;
        if let Some(updater) = &*self.updater.lock().unwrap_or_else(|e| e.into_inner()) {
            updater.run_now();
        }
        Ok(())
    }

    pub fn repo_updates_woke(&self) -> Result<(), CoreError> {
        if let Some(updater) = &*self.updater.lock().unwrap_or_else(|e| e.into_inner()) {
            updater.woke();
        }
        Ok(())
    }

    pub fn shutdown_repo_updates(&self) -> Result<(), CoreError> {
        if let Some(updater) = &*self.updater.lock().unwrap_or_else(|e| e.into_inner()) {
            updater.shutdown();
        }
        Ok(())
    }
}

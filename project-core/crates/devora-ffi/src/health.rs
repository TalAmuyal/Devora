//! Cancellable health check and the shared report over UniFFI.

use crate::{CoreError, DevoraCore};
use devora_core::health::{self, HealthItem, HealthReport, HealthSection, HealthState, HealthTile};
use std::path::Path;
use std::sync::Arc;

#[uniffi::remote(Enum)]
pub enum HealthState {
    Ok,
    Warn,
    Error,
    Info,
}

#[uniffi::remote(Record)]
pub struct HealthTile {
    pub label: String,
    pub met: u32,
    pub total: u32,
    pub state: HealthState,
}

#[uniffi::remote(Record)]
pub struct HealthItem {
    pub name: String,
    pub state: HealthState,
    pub message: String,
    pub version: Option<String>,
    pub path: Option<String>,
    pub fix_hint: Option<String>,
}

#[uniffi::remote(Record)]
pub struct HealthSection {
    pub title: String,
    pub count: Option<String>,
    pub state: HealthState,
    pub items: Vec<HealthItem>,
}

#[uniffi::remote(Record)]
pub struct HealthReport {
    pub version: String,
    pub overall_state: HealthState,
    pub overall_label: String,
    pub tiles: Vec<HealthTile>,
    pub sections: Vec<HealthSection>,
}

/// One health run. Call run on a worker and cancel explicitly on close or re-run.
#[derive(uniffi::Object)]
pub struct HealthRun {
    cancellation: health::HealthCancellation,
}

#[uniffi::export]
impl HealthRun {
    pub fn run(
        &self,
        resource_dir: Option<String>,
        profile_path: Option<String>,
    ) -> Result<health::HealthReport, CoreError> {
        Ok(health::run_report(
            resource_dir.as_deref().map(Path::new),
            profile_path,
            &self.cancellation,
        )?)
    }

    pub fn cancel(&self) -> Result<(), CoreError> {
        self.cancellation.cancel();
        Ok(())
    }
}

#[uniffi::export]
impl DevoraCore {
    pub fn new_health_run(&self) -> Result<Arc<HealthRun>, CoreError> {
        Ok(Arc::new(HealthRun {
            cancellation: health::HealthCancellation::new(),
        }))
    }
}

use crate::CoreError;

/// `seconds` in Devora's shorthand for a duration ("45s", "3m 5s", "1h 4m"; `devora_core::format::duration`).
#[uniffi::export]
pub fn format_duration(seconds: u64) -> Result<String, CoreError> {
    Ok(devora_core::format::duration(seconds))
}

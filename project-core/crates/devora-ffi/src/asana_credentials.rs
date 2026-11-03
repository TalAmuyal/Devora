//! The Asana token is one user credential shared with Debi; its secret never crosses back into Swift.

use crate::{CoreError, DevoraCore};

#[uniffi::export]
impl DevoraCore {
    pub fn get_asana_token_status(&self) -> Result<bool, CoreError> {
        Ok(self.credentials.get()?.is_some())
    }

    pub fn set_asana_token(&self, token: String) -> Result<(), CoreError> {
        Ok(self.credentials.set(&token)?)
    }

    pub fn clear_asana_token(&self) -> Result<(), CoreError> {
        Ok(self.credentials.clear()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use devora_core::credentials::{MemoryTokenStore, TokenStore};
    use std::sync::Arc;

    #[test]
    fn injected_credentials_are_shared_and_empty_saves_preserve_the_token() {
        let root = tempfile::tempdir().unwrap();
        let store = Arc::new(MemoryTokenStore::default());
        let core = DevoraCore::with_credentials(
            Some(
                root.path()
                    .join("config.json")
                    .to_string_lossy()
                    .into_owned(),
            ),
            None,
            crate::log::test_log(),
            store.clone(),
        )
        .unwrap();
        assert!(!core.get_asana_token_status().unwrap());
        core.set_asana_token(" test-token ".into()).unwrap();
        assert!(core.get_asana_token_status().unwrap());
        assert_eq!(store.get().unwrap().as_deref(), Some("test-token"));
        assert!(core.set_asana_token(" ".into()).is_err());
        assert!(core.get_asana_token_status().unwrap());
        core.clear_asana_token().unwrap();
        core.clear_asana_token().unwrap();
        assert!(!core.get_asana_token_status().unwrap());
    }
}

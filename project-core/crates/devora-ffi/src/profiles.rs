//! The profile registry (`devora_core::profile`) across the boundary: the profile form's path validation, and registering and unregistering profiles in the global config.

use devora_core::profile::{self, RegisteredProfile};

use crate::{CoreError, DevoraCore};

#[uniffi::remote(Record)]
pub struct RegisteredProfile {
    pub name: String,
    /// The profile's root, with `~` expanded.
    pub path: String,
}

/// What a profile root path entered in the profile form would be; `expanded_path` is the path with `~` expanded.
#[derive(Debug, PartialEq, uniffi::Enum)]
pub enum ProfilePathValidation {
    /// Registering it creates the profile there: `config.json`, `repos/` and `workspaces/`.
    New { expanded_path: String },
    /// An initialized profile, which registering adds as it is, under its own name.
    ExistingProfile { expanded_path: String, name: String },
    Invalid {
        expanded_path: String,
        error: String,
    },
}

impl From<profile::ProfilePathValidation> for ProfilePathValidation {
    fn from(validation: profile::ProfilePathValidation) -> Self {
        let expanded_path = validation.expanded_path;
        match validation.kind.as_str() {
            "new" => Self::New { expanded_path },
            "existing_profile" => Self::ExistingProfile {
                expanded_path,
                name: validation.name.unwrap_or_default(),
            },
            _ => Self::Invalid {
                expanded_path,
                error: validation.error.unwrap_or_default(),
            },
        }
    }
}

#[uniffi::export]
impl DevoraCore {
    pub fn validate_profile_path(&self, path: String) -> Result<ProfilePathValidation, CoreError> {
        Ok(profile::validate_profile_path(&path)?.into())
    }

    /// Registers the profile at `path` in the global config: an existing profile as it is, under its own name, or a new one, created there with `name`.
    /// Registering a path that is already registered changes nothing.
    pub fn register_profile(
        &self,
        path: String,
        name: String,
    ) -> Result<RegisteredProfile, CoreError> {
        Ok(profile::register_profile(&self.config_path, &path, &name)?)
    }

    /// Removes the profile at `path` from the global config; its directory stays on disk.
    pub fn unregister_profile(&self, path: String) -> Result<(), CoreError> {
        Ok(profile::unregister_profile(&self.config_path, &path)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    use std::sync::Arc;

    fn core(root: &Path) -> Arc<DevoraCore> {
        let config = root.join("config.json");
        fs::write(&config, r#"{"profiles": []}"#).unwrap();
        DevoraCore::new(
            Some(config.to_string_lossy().into_owned()),
            None,
            crate::log::test_log(),
        )
        .unwrap()
    }

    fn listed(core: &DevoraCore) -> Vec<(String, String)> {
        let profiles = core.list_profiles().unwrap();
        profiles.into_iter().map(|p| (p.name, p.path)).collect()
    }

    #[test]
    fn a_new_path_is_created_as_a_profile_and_listed() {
        let tmp = tempfile::tempdir().unwrap();
        let core = core(tmp.path());
        let root = tmp.path().join("work").to_string_lossy().into_owned();

        let validation = core.validate_profile_path(root.clone()).unwrap();
        let registered = core
            .register_profile(root.clone(), " Work ".into())
            .unwrap();

        assert_eq!(
            validation,
            ProfilePathValidation::New {
                expanded_path: root.clone()
            }
        );
        assert_eq!(
            (registered.name.as_str(), registered.path.as_str()),
            ("Work", root.as_str())
        );
        assert!(tmp.path().join("work/repos").is_dir());
        assert!(tmp.path().join("work/workspaces").is_dir());
        assert_eq!(listed(&core), vec![("Work".to_string(), root)]);
    }

    #[test]
    fn an_existing_profile_keeps_its_own_name() {
        let tmp = tempfile::tempdir().unwrap();
        let core = core(tmp.path());
        let legacy = tmp.path().join("legacy");
        fs::create_dir(&legacy).unwrap();
        fs::write(legacy.join("config.json"), r#"{"name": "Legacy"}"#).unwrap();
        let root = legacy.to_string_lossy().into_owned();

        let validation = core.validate_profile_path(root.clone()).unwrap();
        let registered = core
            .register_profile(root.clone(), "Ignored".into())
            .unwrap();

        assert_eq!(
            validation,
            ProfilePathValidation::ExistingProfile {
                expanded_path: root.clone(),
                name: "Legacy".into()
            }
        );
        assert_eq!(registered.name, "Legacy");
    }

    #[test]
    fn an_invalid_path_says_why_and_cant_be_registered() {
        let tmp = tempfile::tempdir().unwrap();
        let core = core(tmp.path());

        let validation = core.validate_profile_path("relative/path".into()).unwrap();
        let registered = core.register_profile("relative/path".into(), "Work".into());

        assert_eq!(
            validation,
            ProfilePathValidation::Invalid {
                expanded_path: "relative/path".into(),
                error: "Path must be absolute or start with ~".into()
            }
        );
        assert_eq!(
            registered.unwrap_err().to_string(),
            "Path must be absolute or start with ~"
        );
    }

    #[test]
    fn unregistering_keeps_the_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let core = core(tmp.path());
        let root = tmp.path().join("work").to_string_lossy().into_owned();
        core.register_profile(root.clone(), "Work".into()).unwrap();

        core.unregister_profile(root).unwrap();

        assert_eq!(listed(&core), vec![]);
        assert!(tmp.path().join("work/config.json").is_file());
    }
}

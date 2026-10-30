//! What the app bundle ships in its resource directory (`<app>/Contents/Resources`), as written by `populate-app-resources.sh` and `mise mac-install`.

use std::path::{Path, PathBuf};

/// The app's version and install suffix, as the bundle records them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppVersion {
    /// `VERSION`, or the caller's fallback (the app's own build version) when the bundle has none.
    pub version: String,
    /// `INSTALL_SUFFIX`, written by `mise mac-install --suffix` to tell side-by-side installs apart.
    pub install_suffix: Option<String>,
}

impl AppVersion {
    pub fn read(resources_dir: Option<&Path>, fallback_version: &str) -> Self {
        let read = |name: &str| {
            resources_dir
                .and_then(|dir| std::fs::read_to_string(dir.join(name)).ok())
                .map(|value| value.trim().to_string())
        };
        Self {
            version: read("VERSION").unwrap_or_else(|| fallback_version.to_string()),
            install_suffix: read("INSTALL_SUFFIX"),
        }
    }

    pub fn window_title(&self) -> String {
        match &self.install_suffix {
            Some(suffix) => format!("Devora {} [{suffix}]", self.version),
            None => format!("Devora {}", self.version),
        }
    }
}

/// The bundled `USER_GUIDE.md`, falling back to the copy in the checkout this crate was built from, for a dev run outside a bundle.
pub fn user_guide_path(resources_dir: Option<&Path>) -> Result<PathBuf, String> {
    find_user_guide(resources_dir, &repo_root())
}

/// `<repo>/project-core/crates/devora-app` → `<repo>`.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("the crate lives three levels below the repo root")
        .to_path_buf()
}

fn find_user_guide(resources_dir: Option<&Path>, repo_root: &Path) -> Result<PathBuf, String> {
    let bundled = resources_dir.map(|dir| dir.join("USER_GUIDE.md"));
    if let Some(path) = bundled.as_ref().filter(|path| path.is_file()) {
        return Ok(path.clone());
    }
    let checkout = repo_root.join("USER_GUIDE.md");
    if checkout.is_file() {
        return Ok(checkout);
    }
    Err(format!(
        "USER_GUIDE.md not found at {}",
        bundled.unwrap_or(checkout).display()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn the_bundle_version_and_suffix_make_the_window_title() {
        let resources = tempfile::tempdir().unwrap();
        fs::write(resources.path().join("VERSION"), "1.2.3\n").unwrap();
        fs::write(resources.path().join("INSTALL_SUFFIX"), "wip\n").unwrap();

        let version = AppVersion::read(Some(resources.path()), "0.1.0");

        assert_eq!(version.window_title(), "Devora 1.2.3 [wip]");
    }

    #[test]
    fn without_a_bundle_version_the_build_version_is_used() {
        let resources = tempfile::tempdir().unwrap();

        assert_eq!(
            AppVersion::read(Some(resources.path()), "0.1.0").window_title(),
            "Devora 0.1.0"
        );
        assert_eq!(
            AppVersion::read(None, "0.1.0").window_title(),
            "Devora 0.1.0"
        );
    }

    #[test]
    fn the_user_guide_comes_from_the_bundle() {
        let resources = tempfile::tempdir().unwrap();
        let checkout = tempfile::tempdir().unwrap();
        fs::write(resources.path().join("USER_GUIDE.md"), "# Guide").unwrap();
        fs::write(checkout.path().join("USER_GUIDE.md"), "# Guide").unwrap();

        assert_eq!(
            find_user_guide(Some(resources.path()), checkout.path()).unwrap(),
            resources.path().join("USER_GUIDE.md")
        );
    }

    #[test]
    fn outside_a_bundle_the_user_guide_comes_from_the_checkout() {
        let resources = tempfile::tempdir().unwrap();

        let path = user_guide_path(Some(resources.path())).unwrap();

        assert_eq!(path, repo_root().join("USER_GUIDE.md"));
        assert!(path.is_file());
    }

    #[test]
    fn a_missing_user_guide_names_the_bundled_path() {
        let resources = tempfile::tempdir().unwrap();
        let checkout = tempfile::tempdir().unwrap();

        let error = find_user_guide(Some(resources.path()), checkout.path()).unwrap_err();

        assert_eq!(
            error,
            format!(
                "USER_GUIDE.md not found at {}",
                resources.path().join("USER_GUIDE.md").display()
            )
        );
    }
}

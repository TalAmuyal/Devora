//! Which file a Claude Code command runs: the user's `claude`, or the wrapper they set in its place.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// The file `command` runs, or `None` when there's none to run. A command is one of three things, and the whole string
/// is the program, with no arguments:
/// - a name, which is the first executable file of that name on `path` (a `PATH` value), as a shell's `command -v` finds
///   it: a directory or a file nobody may execute earlier on the path is passed over;
/// - an absolute path, used as it is;
/// - a path in the home directory (`~/…`), under `home`.
///
/// Any other path has no directory it could be relative to. A shell's aliases and functions aren't commands here.
pub fn resolve(command: &str, path: Option<&str>, home: Option<&str>) -> Option<PathBuf> {
    let file = match expand(command, home)? {
        file if file.is_absolute() => file,
        _ if command.is_empty() || command.contains('/') => return None,
        _ => {
            return path?
                .split(':')
                .filter(|directory| !directory.is_empty())
                .map(|directory| Path::new(directory).join(command))
                .find(|file| executable(file))
        }
    };
    executable(&file).then_some(file)
}

/// `command` with a leading `~/` as `home`, for a caller whose `exec` does the `PATH` lookup itself (`ccc`); `None` when
/// that needs a home directory and there's none.
pub fn expand(command: &str, home: Option<&str>) -> Option<PathBuf> {
    match command.strip_prefix("~/") {
        Some(rest) => Some(Path::new(home?).join(rest)),
        None => Some(PathBuf::from(command)),
    }
}

fn executable(file: &Path) -> bool {
    file.metadata()
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn script(file: &Path, mode: u32) {
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, "#!/bin/sh\n").unwrap();
        fs::set_permissions(file, fs::Permissions::from_mode(mode)).unwrap();
    }

    #[test]
    fn a_name_is_the_first_executable_file_on_the_path() {
        let root = tempfile::tempdir().unwrap();
        let (first, second, third) = (
            root.path().join("first"),
            root.path().join("second"),
            root.path().join("third"),
        );
        // Passed over: a directory of that name, then a file nobody may execute.
        fs::create_dir_all(first.join("claude")).unwrap();
        script(&second.join("claude"), 0o644);
        script(&third.join("claude"), 0o755);
        let path = format!(
            "{}::{}:{}:/nonexistent",
            first.display(),
            second.display(),
            third.display()
        );
        assert_eq!(
            resolve("claude", Some(&path), None),
            Some(third.join("claude"))
        );
        assert_eq!(resolve("claude-gateway", Some(&path), None), None);
        assert_eq!(resolve("claude", None, None), None);
    }

    #[test]
    fn a_path_is_absolute_or_in_the_home_directory() {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("home");
        let wrapper = home.join("bin/claude gateway");
        script(&wrapper, 0o755);
        script(&home.join("bin/notes"), 0o644);
        let home = home.to_str().unwrap();
        let path = format!("{home}/bin");

        assert_eq!(
            resolve(wrapper.to_str().unwrap(), None, None),
            Some(wrapper.clone())
        );
        assert_eq!(
            resolve("~/bin/claude gateway", None, Some(home)),
            Some(wrapper.clone())
        );
        // The whole string is the program: a space is part of its name, never an argument.
        assert_eq!(resolve("claude gateway", Some(&path), None), Some(wrapper));
        assert_eq!(resolve("~/bin/claude gateway", None, None), None);
        assert_eq!(resolve("~/bin/notes", None, Some(home)), None);
        assert_eq!(resolve("/nonexistent/claude", None, None), None);
        for relative in ["bin/claude gateway", "./claude gateway", ""] {
            assert_eq!(resolve(relative, Some(&path), Some(home)), None);
        }
    }
}

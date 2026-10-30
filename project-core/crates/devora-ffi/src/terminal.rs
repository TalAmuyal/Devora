//! What a tab's terminal runs (`devora-terminal`), resolved from the app state so Sapphire's libghostty can spawn it.

use std::collections::HashMap;
use std::path::Path;

use devora_core::state::{Pane, SessionKind};
use devora_core::terminal::{
    self, HostApp, TerminalHost, TerminalRequest, TerminalSpec, TerminalTarget,
};
use devora_core::workspace;

use crate::{CoreError, DevoraCore};

#[uniffi::remote(Record)]
pub struct TerminalSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: String,
    pub env: HashMap<String, String>,
    pub env_remove: Vec<String>,
}

/// A terminal's [`TerminalSpec`], plus the warnings met while building it (a setting that couldn't be read degrades the terminal rather than failing it).
#[derive(uniffi::Record)]
pub struct TerminalLaunchSpec {
    pub spec: TerminalSpec,
    pub warnings: Vec<String>,
}

#[uniffi::export]
impl DevoraCore {
    /// The launch spec for the terminal shown in pane `pane`, from its tab's session.
    ///
    /// `resources_dir` is the app bundle's `Resources` directory, whose tool directories go first on `PATH`.
    pub fn terminal_spec(
        &self,
        pane: u64,
        resources_dir: Option<String>,
    ) -> Result<TerminalLaunchSpec, CoreError> {
        let state = self.state.snapshot();
        let tab = state
            .window
            .tabs
            .iter()
            .find(|tab| tab.pane == Pane::Terminal { id: pane })
            .ok_or_else(|| format!("no terminal pane with id {pane}"))?;
        let target = match &tab.session.kind {
            SessionKind::Shell => TerminalTarget::Shell { cwd: None },
            SessionKind::Workspace { path } => TerminalTarget::Workspace {
                path: path.clone(),
                repos: workspace::list_repo_subdirs(Path::new(path)),
            },
            SessionKind::CreatingWorkspace | SessionKind::CreationFailed { .. } => {
                return Err(format!("tab {} has no workspace to run a terminal in", tab.id).into())
            }
        };
        let request = TerminalRequest {
            target,
            profile_path: tab.session.profile_path.clone(),
        };
        let host = TerminalHost {
            app: HostApp::Sapphire,
            resources_dir,
            // Sapphire has no IPC server yet (TalAmuyal/Devora#226).
            ipc_port: 0,
        };
        let terminal_id =
            u32::try_from(pane).map_err(|_| format!("pane id {pane} doesn't fit DEVORA_PTY_ID"))?;
        let mut warnings = Vec::new();
        let spec = terminal::terminal_spec(&request, &host, terminal_id, &mut warnings);
        Ok(TerminalLaunchSpec { spec, warnings })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use devora_core::state::{AppState, Command};
    use std::fs;
    use std::sync::Arc;

    fn core() -> Arc<DevoraCore> {
        DevoraCore::new(Some("/nonexistent/config.json".into()), None).unwrap()
    }

    /// Opens a tab and returns its pane id.
    fn open(core: &DevoraCore, kind: SessionKind, profile_path: Option<String>) -> u64 {
        core.dispatch(Command::OpenSession {
            title: "tab".into(),
            kind,
            profile_path,
        })
        .unwrap();
        let AppState { window, .. } = core.snapshot().unwrap();
        let Pane::Terminal { id } = window.tabs.last().unwrap().pane;
        id
    }

    #[test]
    fn a_workspace_tab_runs_the_profile_default_app_in_its_only_repo() {
        let tmp = tempfile::tempdir().unwrap();
        let profile = tmp.path().join("work");
        fs::create_dir_all(&profile).unwrap();
        fs::write(
            profile.join("config.json"),
            r#"{"terminal": {"default-app": "ccc"}}"#,
        )
        .unwrap();
        let ws = tmp.path().join("ws-1");
        fs::create_dir_all(ws.join("api")).unwrap();
        let ws = ws.to_string_lossy().into_owned();
        let core = core();
        let pane = open(
            &core,
            SessionKind::Workspace { path: ws.clone() },
            Some(profile.to_string_lossy().into_owned()),
        );

        let spec = core.terminal_spec(pane, None).unwrap().spec;

        assert_eq!(spec.cwd, format!("{ws}/api"));
        assert_eq!(spec.args, ["-l", "-i", "-c", "ccc"]);
        assert_eq!(spec.env["DEVORA_SAPPHIRE"], "1");
        assert_eq!(spec.env["DEVORA_PTY_ID"], pane.to_string());
    }

    #[test]
    fn a_shell_tab_runs_a_bare_login_shell() {
        let core = core();
        let pane = open(&core, SessionKind::Shell, None);

        let spec = core.terminal_spec(pane, None).unwrap().spec;

        assert_eq!(spec.args, ["-l", "-i"]);
    }

    #[test]
    fn tabs_without_a_workspace_and_unknown_panes_have_no_spec() {
        let core = core();
        let creating = open(&core, SessionKind::CreatingWorkspace, None);

        assert!(core.terminal_spec(creating, None).is_err());
        assert!(core.terminal_spec(creating + 100, None).is_err());
    }
}

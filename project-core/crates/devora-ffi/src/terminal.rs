//! What a tab's terminal runs (`devora-terminal`), resolved from the app state so Sapphire's libghostty can spawn it.

use std::collections::HashMap;
use std::path::Path;

use devora_core::state::SessionKind;
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

#[uniffi::export]
impl DevoraCore {
    /// The launch spec for the terminal shown in pane `pane`, from its tab's session.
    ///
    /// `resources_dir` is the app bundle's `Resources` directory, whose tool directories go first on `PATH`.
    /// A setting that can't be read degrades the terminal (to a bare shell) rather than failing it, so it is logged as a warning, as Ember does.
    pub fn terminal_spec(
        &self,
        pane: u64,
        resources_dir: Option<String>,
    ) -> Result<TerminalSpec, CoreError> {
        let state = self.state.snapshot();
        let tab = state
            .window
            .tabs
            .iter()
            .find(|tab| tab.pane.terminal_id() == pane)
            .ok_or_else(|| format!("no terminal pane with id {pane}"))?;
        let target = match &tab.session.kind {
            SessionKind::Shell => TerminalTarget::Shell { cwd: None },
            SessionKind::Workspace { path } => TerminalTarget::Workspace {
                path: path.clone(),
                repos: workspace::list_repo_subdirs(Path::new(path)),
            },
            SessionKind::AgentSpace { .. }
            | SessionKind::CreatingWorkspace
            | SessionKind::CreationFailed { .. } => {
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
            ipc_port: self
                .ipc
                .get()
                .ok_or_else(|| "IPC server is not ready".to_string())?
                .port(),
        };
        let terminal_id =
            u32::try_from(pane).map_err(|_| format!("pane id {pane} doesn't fit DEVORA_PTY_ID"))?;
        let mut warnings = Vec::new();
        let spec = terminal::terminal_spec(&request, &host, terminal_id, &mut warnings);
        for warning in &warnings {
            self.log.warn(warning);
        }
        Ok(spec)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use devora_core::state::{AppState, Command};
    use std::fs;
    use std::sync::Arc;

    fn core() -> Arc<DevoraCore> {
        DevoraCore::new(
            Some("/nonexistent/config.json".into()),
            None,
            crate::log::test_log(),
        )
        .unwrap()
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
        let id = window.tabs.last().unwrap().pane.terminal_id();
        id
    }

    #[test]
    fn ipc_preview_uses_the_terminal_port_and_commits_background_tab_state() {
        use std::io::{Read, Write};
        let core = core();
        let terminal = open(&core, SessionKind::Shell, None);
        open(&core, SessionKind::Shell, None);
        let spec = core.terminal_spec(terminal, None).unwrap();
        let port: u16 = spec.env["DEVORA_IPC_PORT"].parse().unwrap();
        let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let body = format!(r#"{{"ptyId":{terminal},"path":"/example.md","stack":false}}"#);
        write!(stream, "POST /preview/open HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        assert!(response.contains("200 OK"), "{response}");
        let state = core.snapshot().unwrap();
        assert_eq!(state.window.tabs[0].pane.previews()[0].path, "/example.md");
        assert_eq!(state.window.active_tab, Some(state.window.tabs[1].id));
        assert_eq!(
            core.terminal_spec(terminal, None).unwrap().env["DEVORA_PTY_ID"],
            terminal.to_string()
        );
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

        let spec = core.terminal_spec(pane, None).unwrap();

        assert_eq!(spec.cwd, format!("{ws}/api"));
        assert_eq!(spec.args, ["-l", "-i", "-c", "ccc"]);
        assert_eq!(spec.env["DEVORA_SAPPHIRE"], "1");
        assert_eq!(spec.env["DEVORA_PTY_ID"], pane.to_string());
    }

    #[test]
    fn a_workspace_opened_while_another_profile_is_active_takes_its_own_profiles_settings() {
        let tmp = tempfile::tempdir().unwrap();
        let profile = |name: &str, opus: &str| {
            let root = tmp.path().join(name);
            fs::create_dir_all(root.join("workspaces/ws-1")).unwrap();
            fs::write(root.join("workspaces/ws-1/initialized"), "").unwrap();
            fs::write(
                root.join("config.json"),
                format!(r#"{{"claude": {{"opus-model": "{opus}"}}}}"#),
            )
            .unwrap();
            root.to_string_lossy().into_owned()
        };
        let work = profile("work", "opus-for-work");
        let home = profile("home", "opus-for-home");
        let core = core();
        let listed = core.list_workspaces(work.clone()).unwrap();
        core.dispatch(Command::SetActiveProfile {
            profile_path: Some(home),
        })
        .unwrap();

        let ws = &listed[0];
        let pane = open(
            &core,
            SessionKind::Workspace {
                path: ws.path.clone(),
            },
            Some(ws.profile_path.clone()),
        );
        let spec = core.terminal_spec(pane, None).unwrap();

        assert_eq!(spec.env["ANTHROPIC_DEFAULT_OPUS_MODEL"], "opus-for-work");
        assert_eq!(spec.env["DEBI_PROFILE_PATH"], work);
    }

    #[test]
    fn a_shell_tab_runs_a_bare_login_shell() {
        let core = core();
        let pane = open(&core, SessionKind::Shell, None);

        let spec = core.terminal_spec(pane, None).unwrap();

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

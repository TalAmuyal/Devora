use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::thread::JoinHandle;
use tauri::ipc::Channel;

pub struct PtySession {
    writer: Box<dyn Write + Send>,
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    reader_handle: Option<JoinHandle<()>>,
}

pub struct PtyManager {
    sessions: HashMap<u32, PtySession>,
    next_id: u32,
}

impl PtyManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            next_id: 1,
        }
    }

    pub fn create_session(
        &mut self,
        cwd: Option<&str>,
        shell: Option<&str>,
        env: Option<&HashMap<String, String>>,
        app_command: Option<&str>,
        ipc_port: u16,
        cols: u16,
        rows: u16,
        on_output: Channel<Vec<u8>>,
        on_exit: Channel<u32>,
    ) -> Result<u32, String> {
        let id = self.next_id;
        self.next_id += 1;

        let pty_system = native_pty_system();

        let size = PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        };

        let pair = pty_system
            .openpty(size)
            .map_err(|e| format!("failed to open pty: {e}"))?;

        let shell_path = shell
            .map(String::from)
            .or_else(|| std::env::var("SHELL").ok())
            .unwrap_or_else(|| "/bin/zsh".to_string());

        let mut cmd = CommandBuilder::new(&shell_path);
        cmd.args(["-l", "-i"]);

        if let Some(app_cmd) = app_command.filter(|s| !s.is_empty()) {
            cmd.args(["-c", app_cmd]);
        }

        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        cmd.env("DEVORA_EMBER", "1");
        cmd.env("DEVORA_IPC_PORT", ipc_port.to_string());
        cmd.env("DEVORA_PTY_ID", id.to_string());

        let mut extra_paths: Vec<String> = Vec::new();

        // Compile-time: add project-crit-integration/bin if it exists
        let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent());
        if let Some(root) = repo_root {
            let cc_plugins = root.join("project-crit-integration").join("bin");
            if cc_plugins.exists() {
                extra_paths.push(cc_plugins.to_string_lossy().into_owned());
            }
        }

        // Runtime: detect bundled-apps/ relative to the executable
        // App bundle structure: Contents/MacOS/devora-ember → Contents/Resources/bundled-apps/
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(contents_dir) = exe_path.parent().and_then(|p| p.parent()) {
                let bundled_apps = contents_dir.join("Resources").join("bundled-apps");
                if bundled_apps.exists() {
                    extra_paths.push(bundled_apps.to_string_lossy().into_owned());
                }
            }
        }

        if !extra_paths.is_empty() {
            let current_path = std::env::var("PATH").unwrap_or_default();
            let new_path = format!("{}:{current_path}", extra_paths.join(":"));
            cmd.env("PATH", &new_path);
        }

        if let Some(extra_env) = env {
            for (key, value) in extra_env {
                cmd.env(key, value);
            }
        }

        let effective_cwd = cwd
            .map(String::from)
            .or_else(|| std::env::var("HOME").ok())
            .unwrap_or_else(|| "/".to_string());
        cmd.cwd(&effective_cwd);

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| format!("failed to spawn command: {e}"))?;

        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| format!("failed to clone reader: {e}"))?;

        let writer = pair
            .master
            .take_writer()
            .map_err(|e| format!("failed to take writer: {e}"))?;

        let reader_handle = std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if on_output.send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            let _ = on_exit.send(id);
        });

        let session = PtySession {
            writer,
            master: pair.master,
            child,
            reader_handle: Some(reader_handle),
        };

        self.sessions.insert(id, session);
        Ok(id)
    }

    pub fn write(&mut self, id: u32, data: &[u8]) -> Result<(), String> {
        let session = self
            .sessions
            .get_mut(&id)
            .ok_or_else(|| format!("pty session {id} not found"))?;

        session
            .writer
            .write_all(data)
            .map_err(|e| format!("failed to write to pty {id}: {e}"))?;

        session
            .writer
            .flush()
            .map_err(|e| format!("failed to flush pty {id}: {e}"))
    }

    pub fn resize(&mut self, id: u32, cols: u16, rows: u16) -> Result<(), String> {
        let session = self
            .sessions
            .get(&id)
            .ok_or_else(|| format!("pty session {id} not found"))?;

        let size = PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        };

        session
            .master
            .resize(size)
            .map_err(|e| format!("failed to resize pty {id}: {e}"))
    }

    pub fn close(&mut self, id: u32) -> Result<(), String> {
        let mut session = self
            .sessions
            .remove(&id)
            .ok_or_else(|| format!("pty session {id} not found"))?;

        let _ = session.child.kill();

        if let Some(handle) = session.reader_handle.take() {
            let _ = handle.join();
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn noop_exit_channel() -> Channel<u32> {
        Channel::new(|_| Ok(()))
    }

    fn collecting_channel() -> (Channel<Vec<u8>>, Arc<Mutex<Vec<u8>>>) {
        let collected = Arc::new(Mutex::new(Vec::new()));
        let collected_clone = collected.clone();
        let channel = Channel::new(move |body| {
            match body {
                tauri::ipc::InvokeResponseBody::Raw(bytes) => {
                    collected_clone.lock().unwrap().extend_from_slice(&bytes);
                }
                tauri::ipc::InvokeResponseBody::Json(json) => {
                    if let Ok(bytes) = serde_json::from_str::<Vec<u8>>(&json) {
                        collected_clone.lock().unwrap().extend_from_slice(&bytes);
                    }
                }
            }
            Ok(())
        });
        (channel, collected)
    }

    #[test]
    fn test_create_session() {
        let mut manager = PtyManager::new();
        let (channel, _collected) = collecting_channel();

        let id = manager
            .create_session(Some("/tmp"), None, None, None, 0, 80, 24, channel, noop_exit_channel())
            .expect("should create session");

        assert_eq!(id, 1);
        assert!(manager.sessions.contains_key(&id));

        manager.close(id).expect("should close session");
    }

    #[test]
    fn test_write_and_read() {
        let mut manager = PtyManager::new();
        let (channel, collected) = collecting_channel();

        let id = manager
            .create_session(
                Some("/tmp"),
                None,
                None,
                Some("echo EMBER_TEST_OUTPUT"),
                0,
                80,
                24,
                channel,
                noop_exit_channel(),
            )
            .expect("should create session");

        std::thread::sleep(std::time::Duration::from_millis(500));

        let output = collected.lock().unwrap();
        let output_str = String::from_utf8_lossy(&output);
        assert!(
            output_str.contains("EMBER_TEST_OUTPUT"),
            "expected output to contain 'EMBER_TEST_OUTPUT', got: {output_str}"
        );

        manager.close(id).expect("should close session");
    }

    #[test]
    fn test_resize() {
        let mut manager = PtyManager::new();
        let (channel, _collected) = collecting_channel();

        let id = manager
            .create_session(Some("/tmp"), None, None, None, 0, 80, 24, channel, noop_exit_channel())
            .expect("should create session");

        manager
            .resize(id, 120, 40)
            .expect("resize should not fail");

        manager.close(id).expect("should close session");
    }

    #[test]
    fn test_close_cleans_up() {
        let mut manager = PtyManager::new();
        let (channel, _collected) = collecting_channel();

        let id = manager
            .create_session(Some("/tmp"), None, None, None, 0, 80, 24, channel, noop_exit_channel())
            .expect("should create session");

        manager.close(id).expect("should close session");
        assert!(!manager.sessions.contains_key(&id));

        let result = manager.close(id);
        assert!(result.is_err());
    }

    #[test]
    fn test_write_to_nonexistent_session() {
        let mut manager = PtyManager::new();
        let result = manager.write(999, b"hello");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn test_resize_nonexistent_session() {
        let mut manager = PtyManager::new();
        let result = manager.resize(999, 80, 24);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn test_session_ids_increment() {
        let mut manager = PtyManager::new();
        let (channel1, _) = collecting_channel();
        let (channel2, _) = collecting_channel();

        let id1 = manager
            .create_session(Some("/tmp"), None, None, Some("true"), 0, 80, 24, channel1, noop_exit_channel())
            .expect("should create session 1");
        let id2 = manager
            .create_session(Some("/tmp"), None, None, Some("true"), 0, 80, 24, channel2, noop_exit_channel())
            .expect("should create session 2");

        assert_eq!(id1, 1);
        assert_eq!(id2, 2);

        manager.close(id1).expect("should close session 1");
        manager.close(id2).expect("should close session 2");
    }
}

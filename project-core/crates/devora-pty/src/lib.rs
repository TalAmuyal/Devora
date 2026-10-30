//! Terminal sessions: a login shell (or an app command run by one) on a pseudo-terminal, with output delivered through callbacks.

use devora_terminal::TerminalSpec;
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::thread::JoinHandle;

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

impl Default for PtyManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PtyManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            next_id: 1,
        }
    }

    /// Spawns a terminal session and returns its id.
    ///
    /// `spec_for` builds the session's [`TerminalSpec`] for the id it is given (the spec exposes that id to the terminal, see `devora_terminal::terminal_spec`).
    pub fn create_session(
        &mut self,
        spec_for: impl FnOnce(u32) -> TerminalSpec,
        cols: u16,
        rows: u16,
        mut on_output: impl FnMut(Vec<u8>) -> Result<(), String> + Send + 'static,
        on_exit: impl FnOnce(u32) + Send + 'static,
    ) -> Result<u32, String> {
        let id = self.next_id;
        self.next_id += 1;
        let spec = spec_for(id);

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

        let mut cmd = CommandBuilder::new(&spec.program);
        cmd.args(&spec.args);
        for name in &spec.env_remove {
            cmd.env_remove(name);
        }
        for (key, value) in &spec.env {
            cmd.env(key, value);
        }
        cmd.cwd(&spec.cwd);

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
                        if on_output(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            on_exit(id);
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

    fn noop_exit_channel() -> impl FnOnce(u32) + Send + 'static {
        |_| {}
    }

    type Collected = Arc<Mutex<Vec<u8>>>;

    /// A login shell in `/tmp`, running `app` when given; `$DEVORA_PTY_ID` carries the session's id.
    fn shell(app: Option<&str>) -> impl FnOnce(u32) -> TerminalSpec {
        let app = app.map(String::from);
        move |id| {
            let mut args = vec!["-l".to_string(), "-i".to_string()];
            args.extend(app.into_iter().flat_map(|app| ["-c".to_string(), app]));
            TerminalSpec {
                program: std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string()),
                args,
                cwd: "/tmp".to_string(),
                env: HashMap::from([("DEVORA_PTY_ID".to_string(), id.to_string())]),
                env_remove: vec![],
            }
        }
    }

    fn wait_for_output(collected: &Collected, needle: &str) -> String {
        // A login shell's startup time varies by machine (profile scripts), so wait for the output rather than for a fixed time.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            let output = String::from_utf8_lossy(&collected.lock().unwrap()).into_owned();
            if output.contains(needle) || std::time::Instant::now() >= deadline {
                return output;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }

    fn collecting_channel() -> (
        impl FnMut(Vec<u8>) -> Result<(), String> + Send + 'static,
        Collected,
    ) {
        let collected = Arc::new(Mutex::new(Vec::new()));
        let collected_clone = collected.clone();
        let channel = move |bytes: Vec<u8>| {
            collected_clone.lock().unwrap().extend_from_slice(&bytes);
            Ok(())
        };
        (channel, collected)
    }

    #[test]
    fn test_create_session() {
        let mut manager = PtyManager::new();
        let (channel, _collected) = collecting_channel();

        let id = manager
            .create_session(shell(None), 80, 24, channel, noop_exit_channel())
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
                shell(Some("echo EMBER_TEST_OUTPUT")),
                80,
                24,
                channel,
                noop_exit_channel(),
            )
            .expect("should create session");

        let output_str = wait_for_output(&collected, "EMBER_TEST_OUTPUT");
        assert!(
            output_str.contains("EMBER_TEST_OUTPUT"),
            "expected output to contain 'EMBER_TEST_OUTPUT', got: {output_str}"
        );

        manager.close(id).expect("should close session");
    }

    #[test]
    fn test_spec_gets_the_session_id_and_its_env_reaches_the_shell() {
        let mut manager = PtyManager::new();
        let (channel, collected) = collecting_channel();

        let id = manager
            .create_session(
                shell(Some("echo pty-id=$DEVORA_PTY_ID")),
                80,
                24,
                channel,
                noop_exit_channel(),
            )
            .expect("should create session");

        let output = wait_for_output(&collected, &format!("pty-id={id}"));
        assert!(output.contains(&format!("pty-id={id}")), "got: {output}");

        manager.close(id).expect("should close session");
    }

    #[test]
    fn test_resize() {
        let mut manager = PtyManager::new();
        let (channel, _collected) = collecting_channel();

        let id = manager
            .create_session(shell(None), 80, 24, channel, noop_exit_channel())
            .expect("should create session");

        manager.resize(id, 120, 40).expect("resize should not fail");

        manager.close(id).expect("should close session");
    }

    #[test]
    fn test_close_cleans_up() {
        let mut manager = PtyManager::new();
        let (channel, _collected) = collecting_channel();

        let id = manager
            .create_session(shell(None), 80, 24, channel, noop_exit_channel())
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
            .create_session(shell(Some("true")), 80, 24, channel1, noop_exit_channel())
            .expect("should create session 1");
        let id2 = manager
            .create_session(shell(Some("true")), 80, 24, channel2, noop_exit_channel())
            .expect("should create session 2");

        assert_eq!(id1, 1);
        assert_eq!(id2, 2);

        manager.close(id1).expect("should close session 1");
        manager.close(id2).expect("should close session 2");
    }
}

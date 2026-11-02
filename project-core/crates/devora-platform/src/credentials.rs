//! Reads/writes the Asana API token in the macOS keychain, byte-compatible with how Debi reads it (the Go `credentials` package via `go-keyring`).
//! For now, Devora is macOS-only, so we shell out to `/usr/bin/security` exactly as `go-keyring` does - a token stored here is therefore readable by `debi` and vice versa, and the keychain item's ACL trusts `/usr/bin/security` for both, so neither side triggers an access prompt.
//!
//! The service name is `devora-<provider>` and the account is the resolved login (`$USER`, else the system username) - the same convention as `project-debi/internal/credentials`.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, OnceLock};

const SECURITY_BIN: &str = "/usr/bin/security";
const ASANA_SERVICE: &str = "devora-asana";

/// The shared user's credential, independent of which profile's settings card is open.
/// UI hosts can inject a store; tests use memory so they never inspect or mutate the login keychain.
pub trait TokenStore: Send + Sync {
    fn get(&self) -> Result<Option<String>, String>;
    fn set(&self, token: &str) -> Result<(), String>;
    fn clear(&self) -> Result<(), String>;
}

#[derive(Default)]
pub struct MemoryTokenStore(Mutex<Option<String>>);

impl TokenStore for MemoryTokenStore {
    fn get(&self) -> Result<Option<String>, String> {
        Ok(self.0.lock().map_err(|e| e.to_string())?.clone())
    }

    fn set(&self, token: &str) -> Result<(), String> {
        let token = token.trim();
        if token.is_empty() {
            return Err("the token must not be empty".into());
        }
        *self.0.lock().map_err(|e| e.to_string())? = Some(token.into());
        Ok(())
    }

    fn clear(&self) -> Result<(), String> {
        *self.0.lock().map_err(|e| e.to_string())? = None;
        Ok(())
    }
}

struct KeychainTokenStore;

impl TokenStore for KeychainTokenStore {
    fn get(&self) -> Result<Option<String>, String> {
        read_keychain_token()
    }
    fn set(&self, token: &str) -> Result<(), String> {
        write_keychain_token(token)
    }
    fn clear(&self) -> Result<(), String> {
        remove_keychain_token()
    }
}

/// One store for the process, so both UI scopes see the same token.
/// Both Ember and Sapphire acceptance harnesses already launch with `DEVORA_TEST_MODE=1`.
pub fn token_store() -> Arc<dyn TokenStore> {
    static STORE: OnceLock<Arc<dyn TokenStore>> = OnceLock::new();
    Arc::clone(STORE.get_or_init(|| {
        if cfg!(test) || std::env::var("DEVORA_TEST_MODE").as_deref() == Ok("1") {
            Arc::new(MemoryTokenStore::default())
        } else {
            Arc::new(KeychainTokenStore)
        }
    }))
}

/// `go-keyring` base64-encodes secrets behind this prefix so multi-line / non-ASCII values survive the keychain round-trip; we write and read the same format for full interop.
const BASE64_PREFIX: &str = "go-keyring-base64:";
/// A legacy `go-keyring` hex-encoding prefix, decoded on read for completeness.
const HEX_PREFIX: &str = "go-keyring-encoded:";

/// Resolves the keychain account the way Debi's `credentials` package does: `$USER`, else the system login name (via `id -un`, which resolves the same `getpwuid` entry as Go's `user.Current`).
fn account() -> Result<String, String> {
    if let Ok(user) = std::env::var("USER") {
        if !user.is_empty() {
            return Ok(user);
        }
    }
    let out = Command::new("/usr/bin/id")
        .arg("-un")
        .output()
        .map_err(|e| format!("failed to resolve current user: {e}"))?;
    if !out.status.success() {
        return Err("failed to resolve current user via `id -un`".to_string());
    }
    let name = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if name.is_empty() {
        return Err("could not determine current user".to_string());
    }
    Ok(name)
}

/// Quotes a token for the `security -i` command line, mirroring `go-keyring`'s shell escaping: wrap in single quotes, escaping any embedded single quote as `'\''`.
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// Decodes a value read from the keychain, mirroring `go-keyring`'s `Get`: strip and decode the base64 / hex prefix if present, otherwise return the value verbatim.
fn decode_secret(raw: &str) -> Result<String, String> {
    if let Some(rest) = raw.strip_prefix(BASE64_PREFIX) {
        let bytes = BASE64
            .decode(rest)
            .map_err(|e| format!("failed to decode keychain token: {e}"))?;
        return String::from_utf8(bytes)
            .map_err(|e| format!("keychain token is not valid UTF-8: {e}"));
    }
    if let Some(rest) = raw.strip_prefix(HEX_PREFIX) {
        let bytes = decode_hex(rest)?;
        return String::from_utf8(bytes)
            .map_err(|e| format!("keychain token is not valid UTF-8: {e}"));
    }
    Ok(raw.to_string())
}

fn decode_hex(s: &str) -> Result<Vec<u8>, String> {
    if !s.len().is_multiple_of(2) {
        return Err("invalid hex-encoded keychain token".to_string());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&s[i..i + 2], 16)
                .map_err(|e| format!("invalid hex-encoded keychain token: {e}"))
        })
        .collect()
}

/// Returns the stored Asana token, or `None` when no token is stored.
pub fn get_asana_token() -> Result<Option<String>, String> {
    token_store().get()
}

fn read_keychain_token() -> Result<Option<String>, String> {
    let account = account()?;
    let out = Command::new(SECURITY_BIN)
        .args([
            "find-generic-password",
            "-wa",
            &account,
            "-s",
            ASANA_SERVICE,
        ])
        .output()
        .map_err(|e| format!("failed to run security: {e}"))?;

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        if stderr.contains("could not be found") {
            return Ok(None);
        }
        return Err(format!("keychain read failed: {}", stderr.trim()));
    }

    let raw = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let token = decode_secret(&raw)?;
    Ok((!token.is_empty()).then_some(token))
}

/// Whether an Asana token is currently stored.
pub fn asana_token_present() -> Result<bool, String> {
    Ok(get_asana_token()?.is_some())
}

/// Stores (or replaces) the Asana token in the keychain.
/// Mirrors `go-keyring`'s `Set`: feed the `add-generic-password` command on stdin (so the secret never appears in the process list) and store it base64-encoded behind the well-known prefix.
pub fn set_asana_token(token: &str) -> Result<(), String> {
    token_store().set(token)
}

fn write_keychain_token(token: &str) -> Result<(), String> {
    let token = token.trim();
    if token.is_empty() {
        return Err("the token must not be empty".to_string());
    }

    let account = account()?;
    let encoded = format!("{BASE64_PREFIX}{}", BASE64.encode(token.as_bytes()));
    let command = format!(
        "add-generic-password -U -s {} -a {} -w {}\n",
        shell_quote(ASANA_SERVICE),
        shell_quote(&account),
        shell_quote(&encoded),
    );

    let mut child = Command::new(SECURITY_BIN)
        .arg("-i")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to run security: {e}"))?;
    {
        let mut stdin = child.stdin.take().ok_or("failed to open security stdin")?;
        stdin
            .write_all(command.as_bytes())
            .map_err(|e| format!("failed to send command to security: {e}"))?;
    } // drop stdin so `security -i` stops reading and runs the command

    let out = child
        .wait_with_output()
        .map_err(|e| format!("security failed: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "failed to store token in keychain: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(())
}

/// Removes the stored Asana token. Idempotent: succeeds even when no token is stored.
pub fn clear_asana_token() -> Result<(), String> {
    token_store().clear()
}

fn remove_keychain_token() -> Result<(), String> {
    let account = account()?;
    let out = Command::new(SECURITY_BIN)
        .args([
            "delete-generic-password",
            "-a",
            &account,
            "-s",
            ASANA_SERVICE,
        ])
        .output()
        .map_err(|e| format!("failed to run security: {e}"))?;

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        if stderr.contains("could not be found") {
            return Ok(());
        }
        return Err(format!("keychain delete failed: {}", stderr.trim()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_memory_store_trims_replaces_and_clears_without_system_commands() {
        let store = MemoryTokenStore::default();
        assert_eq!(store.get().unwrap(), None);
        store.set("  first-token  ").unwrap();
        assert_eq!(store.get().unwrap().as_deref(), Some("first-token"));
        assert!(store.set(" \n ").is_err());
        assert_eq!(store.get().unwrap().as_deref(), Some("first-token"));
        store.set("replacement").unwrap();
        assert_eq!(store.get().unwrap().as_deref(), Some("replacement"));
        store.clear().unwrap();
        store.clear().unwrap();
        assert_eq!(store.get().unwrap(), None);
    }

    #[test]
    fn test_decode_secret_base64_prefix() {
        let encoded = format!("{BASE64_PREFIX}{}", BASE64.encode(b"asana-tok/123:abc"));
        assert_eq!(decode_secret(&encoded).unwrap(), "asana-tok/123:abc");
    }

    #[test]
    fn test_decode_secret_hex_prefix() {
        // "hi" = 0x68 0x69
        assert_eq!(decode_secret("go-keyring-encoded:6869").unwrap(), "hi");
    }

    #[test]
    fn test_decode_secret_raw_passthrough() {
        // A token written raw (e.g. via `security add-generic-password -w`) has no prefix.
        assert_eq!(decode_secret("plain-token").unwrap(), "plain-token");
    }

    #[test]
    fn test_decode_secret_rejects_bad_base64() {
        assert!(decode_secret("go-keyring-base64:not valid!!").is_err());
    }

    #[test]
    fn test_shell_quote_escapes_single_quotes() {
        assert_eq!(shell_quote("ab'c"), r"'ab'\''c'");
        assert_eq!(shell_quote("plain"), "'plain'");
    }

    #[test]
    fn test_account_prefers_user_env() {
        // account() returns $USER when set; we only read it, never mutate the global.
        if let Ok(user) = std::env::var("USER") {
            if !user.is_empty() {
                assert_eq!(account().unwrap(), user);
            }
        }
    }
}

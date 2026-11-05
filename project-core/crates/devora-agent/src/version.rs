//! The agent runtime's minimum CLI version (ADR-009).
//!
//! Process startup and a future Rust `debi health` share this check. Health
//! integration is deferred until its current Go implementation is ported.

/// Earliest CLI whose stream-json/control contract was verified by Spike 1.
pub const MINIMUM_CLI_VERSION: &str = "2.1.312";

/// Check a `claude --version` response, including its optional product suffix.
/// Unrecognized or prerelease versions are rejected, rather than assumed safe.
pub fn check_cli_version(output: &str) -> Result<(), String> {
    fn components(text: &str) -> Option<[u64; 3]> {
        if text
            .bytes()
            .any(|byte| !byte.is_ascii_digit() && byte != b'.')
        {
            return None;
        }
        let mut parts = text.split('.');
        let version = [
            parts.next()?.parse().ok()?,
            parts.next()?.parse().ok()?,
            parts.next()?.parse().ok()?,
        ];
        parts.next().is_none().then_some(version)
    }
    let version = output.split_whitespace().next().unwrap_or_default();
    let parsed = components(version)
        .ok_or_else(|| format!("unrecognized Claude Code version: {output:?}"))?;
    if parsed < components(MINIMUM_CLI_VERSION).expect("valid minimum CLI version") {
        return Err(format!(
            "Claude Code {version} is unsupported; version {MINIMUM_CLI_VERSION} or newer is required"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_version_contract_has_one_minimum() {
        for version in [
            MINIMUM_CLI_VERSION,
            "2.1.316 (Claude Code)",
            "2.2.0",
            "3.0.0",
        ] {
            assert!(check_cli_version(version).is_ok(), "{version}");
        }
        for version in ["2.1.311", "2.0.999", "1.99.999"] {
            assert!(check_cli_version(version)
                .unwrap_err()
                .contains(MINIMUM_CLI_VERSION));
        }
        for version in [
            "",
            "unknown",
            "2.1",
            "2.1.312.1",
            "2.1.312-beta",
            "2.1.x",
            "+2.1.312",
        ] {
            assert!(check_cli_version(version).is_err(), "{version}");
        }
    }
}

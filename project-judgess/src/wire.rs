//! The `PermissionRequest` hook output protocol.
//! Always exit 0; only allow and deny carry output, and a missing decision falls through to Claude Code's normal permission flow.

use serde_json::json;

use crate::model::Verdict;

/// What to print on stdout, if anything.
///
/// `Defer` is silent for now: a `PreToolUse` hook returning `ask` is the only way to force a prompt, so a declared "the user must decide" currently behaves like an abstention.
pub fn render(verdict: &Verdict) -> Option<String> {
    let decision = match verdict {
        Verdict::Approve => json!({ "behavior": "allow" }),
        Verdict::Deny { rule } => {
            let message = rule.message().map_or_else(
                || format!("Denied by Judgess rule '{}'.", rule.as_str()),
                str::to_string,
            );
            json!({ "behavior": "deny", "message": message })
        }
        Verdict::Defer { .. } | Verdict::Abstain(_) => return None,
    };
    Some(json!({ "hookSpecificOutput": { "hookEventName": "PermissionRequest", "decision": decision } }).to_string())
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::model::{Abstain, RuleId};

    fn parsed(verdict: &Verdict) -> Value {
        serde_json::from_str(&render(verdict).unwrap()).unwrap()
    }

    #[test]
    fn approve_is_a_bare_allow() {
        assert_eq!(
            parsed(&Verdict::Approve),
            json!({"hookSpecificOutput": {"hookEventName": "PermissionRequest", "decision": {"behavior": "allow"}}})
        );
    }

    #[test]
    fn deny_carries_the_rule_message() {
        assert_eq!(
            parsed(&Verdict::Deny { rule: RuleId::Eval }),
            json!({"hookSpecificOutput": {"hookEventName": "PermissionRequest",
                "decision": {"behavior": "deny", "message": "Command 'eval' is not allowed."}}})
        );
    }

    #[test]
    fn defer_and_abstain_print_nothing() {
        let unknown = Verdict::Abstain(Abstain::Unknown {
            reason: "gap".into(),
        });
        let known = Verdict::Abstain(Abstain::Known {
            rule: RuleId::ToolEdit,
        });
        let defer = Verdict::Defer {
            rule: RuleId::ToolEdit,
        };
        assert!(
            [unknown, known, defer]
                .iter()
                .all(|verdict| render(verdict).is_none())
        );
    }
}

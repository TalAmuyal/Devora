use judgess::{Request, Verdict};
use serde_json::{json, Value};
use std::panic::{catch_unwind, AssertUnwindSafe};

/// A caught panic is an abstention, preserving the CLI's normal permission flow.
pub(super) fn judge(input: &Value) -> (Verdict, Value) {
    evaluate(input, judgess::decide)
}

fn evaluate(input: &Value, decide: impl FnOnce(&Request) -> Verdict) -> (Verdict, Value) {
    let verdict = Request::from_raw(input)
        .map(|request| catch_unwind(AssertUnwindSafe(|| decide(&request))))
        .transpose()
        .map(|verdict| verdict.unwrap_or_else(|| Verdict::unknown("invalid hook input")))
        .unwrap_or_else(|_| Verdict::unknown("Judgess panicked"));
    let output = match &verdict {
        Verdict::Approve => json!({"hookSpecificOutput":{
            "hookEventName":"PreToolUse", "permissionDecision":"allow"
        }}),
        Verdict::Deny { rule } => json!({"hookSpecificOutput":{
            "hookEventName":"PreToolUse", "permissionDecision":"deny",
            "permissionDecisionReason": rule.message().map(str::to_owned)
                .unwrap_or_else(|| format!("Denied by Judgess rule '{}'.",rule.as_str()))
        }}),
        Verdict::Defer { rule } => json!({"hookSpecificOutput":{
            "hookEventName":"PreToolUse", "permissionDecision":"ask",
            "permissionDecisionReason":format!("Judgess defers this to the user ({})",rule.as_str())
        }}),
        Verdict::Abstain(_) => json!({}),
    };
    (verdict, output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn judgess_maps_real_decisions_and_panics_to_the_pretooluse_contract() {
        for (command, expected) in [
            ("git status", Some("allow")),
            ("eval foo", Some("deny")),
            ("rm file", None),
            ("unrecognized-command", None),
        ] {
            let (_, output) =
                judge(&json!({"tool_name":"Bash","tool_input":{"command":command},"cwd":"/tmp"}));
            assert_eq!(
                output
                    .pointer("/hookSpecificOutput/permissionDecision")
                    .and_then(Value::as_str),
                expected,
                "{command}"
            );
            if expected == Some("deny") {
                assert_eq!(
                    output["hookSpecificOutput"]["permissionDecisionReason"],
                    "Command 'eval' is not allowed."
                );
            }
        }
        let (_, deferred) = evaluate(
            &json!({"tool_name":"Bash","tool_input":{"command":"example"}}),
            |_| Verdict::Defer {
                rule: judgess::RuleId::IrreversibleRm,
            },
        );
        assert_eq!(deferred["hookSpecificOutput"]["permissionDecision"], "ask");
        let (_, output) = evaluate(
            &json!({"tool_name":"Bash","tool_input":{"command":"git status"}}),
            |_| panic!("injected Judgess failure"),
        );
        assert_eq!(output, json!({}));
        assert_eq!(judge(&json!({})).1, json!({}));
    }
}

//! The regression cases file: redacted requests with the verdict they must keep getting.

use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model::{Abstain, Request, RuleId, Verdict};

/// What redaction rewrites a home directory to, and so the home directory every case is decided under.
pub const REDACTED_HOME: &str = "/Users/user";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Case {
    pub input: CaseInput,
    pub expected: Expected,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CaseInput {
    pub tool_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    pub tool_input: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExpectedVerdict {
    Approve,
    Deny,
    Defer,
    /// Always a deliberate abstention; a case can never expect a gap.
    Abstain,
    /// Hand-written only: anything but an approval, gaps included.
    /// For commands that must never be approved, whatever else they turn into once the rules grow.
    #[serde(rename = "not-approve")]
    NotApprove,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Expected {
    pub verdict: ExpectedVerdict,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule: Option<String>,
}

impl Expected {
    /// `None` for a gap, which is not something a case can pin down (only `not-approve` can hold one).
    pub fn of(verdict: &Verdict) -> Option<Expected> {
        let (kind, rule) = match verdict {
            Verdict::Approve => (ExpectedVerdict::Approve, None),
            Verdict::Deny { rule } => (ExpectedVerdict::Deny, Some(*rule)),
            Verdict::Defer { rule } => (ExpectedVerdict::Defer, Some(*rule)),
            Verdict::Abstain(Abstain::Known { rule }) => (ExpectedVerdict::Abstain, Some(*rule)),
            Verdict::Abstain(Abstain::Unknown { .. }) => return None,
        };
        Some(Expected {
            verdict: kind,
            rule: rule.map(|rule| rule.as_str().to_string()),
        })
    }

    /// Checks the shape the file format requires: `rule` for everything but approve and not-approve, and only known rule ids.
    pub fn validate(&self) -> Result<(), String> {
        match (self.verdict, &self.rule) {
            (ExpectedVerdict::Approve | ExpectedVerdict::NotApprove, None) => Ok(()),
            (ExpectedVerdict::Approve | ExpectedVerdict::NotApprove, Some(_)) => {
                Err("approve and not-approve take no rule".into())
            }
            (_, None) => Err("rule is required".into()),
            (_, Some(rule)) => RuleId::parse(rule)
                .map(|_| ())
                .ok_or_else(|| format!("unknown rule '{rule}'")),
        }
    }
}

/// The part of a tool input that a verdict can depend on: a Bash command is its command line, nothing else.
pub fn decision_input(tool_name: &str, tool_input: &Value) -> Value {
    match (tool_name, tool_input.get("command")) {
        ("Bash", Some(command)) => serde_json::json!({ "command": command }),
        _ => tool_input.clone(),
    }
}

impl CaseInput {
    pub fn request(&self) -> Request {
        Request {
            tool_name: self.tool_name.clone(),
            tool_input: self.tool_input.clone(),
            cwd: self.cwd.clone(),
            home: Some(REDACTED_HOME.to_string()),
        }
    }
}

impl Case {
    pub fn check(&self) -> Result<(), String> {
        self.expected.validate()?;
        let verdict = crate::decide(&self.input.request());
        if self.expected.verdict == ExpectedVerdict::NotApprove {
            return match verdict {
                Verdict::Approve => Err("expected anything but approve, got Approve".into()),
                _ => Ok(()),
            };
        }
        match Expected::of(&verdict) {
            Some(actual) if actual == self.expected => Ok(()),
            Some(actual) => Err(format!("expected {:?}, got {:?}", self.expected, actual)),
            None => Err(format!("expected {:?}, got {:?}", self.expected, verdict)),
        }
    }
}

pub fn load(path: &Path) -> Result<Vec<Case>, String> {
    let text =
        std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    serde_json::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))
}

pub fn save(path: &Path, cases: &[Case]) -> std::io::Result<()> {
    let mut text = serde_json::to_string_pretty(cases).map_err(std::io::Error::other)?;
    text.push('\n');
    std::fs::write(path, text)
}

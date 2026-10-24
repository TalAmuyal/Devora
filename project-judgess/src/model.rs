use serde_json::Value;

#[derive(Debug, Clone)]
pub struct Request {
    pub tool_name: String,
    pub tool_input: Value,
    pub cwd: Option<String>,
    pub home: Option<String>,
}

impl Request {
    /// The request a raw hook payload (as logged in `audit.jsonl` / `unknown.jsonl`) describes.
    pub fn from_raw(raw: &Value, home: Option<&str>) -> Option<Request> {
        Some(Request {
            tool_name: raw.get("tool_name")?.as_str()?.to_string(),
            tool_input: raw.get("tool_input").cloned().unwrap_or(Value::Null),
            cwd: raw.get("cwd").and_then(Value::as_str).map(str::to_string),
            home: home.map(str::to_string),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Approve,
    Deny {
        rule: RuleId,
    },
    /// The user must decide; declared intent, as opposed to a gap.
    Defer {
        rule: RuleId,
    },
    Abstain(Abstain),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Abstain {
    /// Deliberately left to the normal permission flow.
    Known { rule: RuleId },
    /// A gap in the rules; queued for classification.
    Unknown { reason: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerdictKind {
    Approve,
    Deny,
    Defer,
    AbstainKnown,
    AbstainUnknown,
}

impl VerdictKind {
    pub fn as_str(self) -> &'static str {
        match self {
            VerdictKind::Approve => "approve",
            VerdictKind::Deny => "deny",
            VerdictKind::Defer => "defer",
            VerdictKind::AbstainKnown => "abstain-known",
            VerdictKind::AbstainUnknown => "abstain-unknown",
        }
    }
}

impl Verdict {
    pub fn kind(&self) -> VerdictKind {
        match self {
            Verdict::Approve => VerdictKind::Approve,
            Verdict::Deny { .. } => VerdictKind::Deny,
            Verdict::Defer { .. } => VerdictKind::Defer,
            Verdict::Abstain(Abstain::Known { .. }) => VerdictKind::AbstainKnown,
            Verdict::Abstain(Abstain::Unknown { .. }) => VerdictKind::AbstainUnknown,
        }
    }

    pub fn known(rule: RuleId) -> Verdict {
        Verdict::Abstain(Abstain::Known { rule })
    }

    pub fn unknown(reason: impl Into<String>) -> Verdict {
        Verdict::Abstain(Abstain::Unknown {
            reason: reason.into(),
        })
    }

    pub fn rule(&self) -> Option<RuleId> {
        match self {
            Verdict::Approve | Verdict::Abstain(Abstain::Unknown { .. }) => None,
            Verdict::Deny { rule }
            | Verdict::Defer { rule }
            | Verdict::Abstain(Abstain::Known { rule }) => Some(*rule),
        }
    }
}

macro_rules! rules {
    ($($variant:ident => $id:literal),+ $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum RuleId { $($variant),+ }

        impl RuleId {
            pub const ALL: &'static [RuleId] = &[$(RuleId::$variant),+];

            pub fn as_str(self) -> &'static str {
                match self { $(RuleId::$variant => $id),+ }
            }
        }
    };
}

rules! {
    Eval => "eval",
    UseMise => "use-mise",
    GitAddViaPrSkill => "git-add-via-pr-skill",
    WebfetchFileUseRead => "webfetch-file-use-read",
    IrreversibleRm => "irreversible-rm",
    IrreversibleGitCheckout => "irreversible-git-checkout",
    IrreversibleGitReset => "irreversible-git-reset",
    IrreversibleFindDelete => "irreversible-find-delete",
    IrreversibleSedInPlace => "irreversible-sed-in-place",
    Chmod => "chmod",
    PrivilegeEscalation => "privilege-escalation",
    LocalScript => "local-script",
    ShellScript => "shell-script",
    ToolEdit => "tool-edit",
    ToolWrite => "tool-write",
    ToolAskUserQuestion => "tool-ask-user-question",
    WebfetchHttp => "webfetch-http",
}

const MSG_USE_MISE: &str =
    "Please use a mise task or `mise exec -- ...` instead (create a generic task if needed).";

impl RuleId {
    pub fn parse(id: &str) -> Option<RuleId> {
        RuleId::ALL.iter().copied().find(|rule| rule.as_str() == id)
    }

    /// The redirect shown to the agent; only deny rules have one.
    pub fn message(self) -> Option<&'static str> {
        match self {
            RuleId::Eval => Some("Command 'eval' is not allowed."),
            RuleId::UseMise => Some(MSG_USE_MISE),
            RuleId::GitAddViaPrSkill => Some(
                "The PR skill should be used instead, but only after a PR creation was requested. \
                 If a PR already exists, use `gaac` instead.",
            ),
            RuleId::WebfetchFileUseRead => Some(
                "`WebFetch` access to local files is not allowed, use the `Read` tool instead.",
            ),
            _ => None,
        }
    }
}

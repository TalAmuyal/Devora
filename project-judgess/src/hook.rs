//! `judgess hook`: stdin JSON in, wire output out, audit trail on the side. Nothing here may fail the agent.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::Instant;

use serde_json::{Value, json};

use crate::model::{Abstain, Request, Verdict};
use crate::store::{self, Store};
use crate::{decide, wire};

/// The stdout to print. Persistence problems never change it.
pub fn run(raw_input: &str, store: &Store) -> Option<String> {
    let started = Instant::now();
    let input: Value = match serde_json::from_str(raw_input) {
        Ok(input) => input,
        Err(error) => {
            audit_error(
                store,
                &Value::Null,
                &format!("invalid JSON: {error}"),
                started,
            );
            return None;
        }
    };
    let Some(request) = Request::from_raw(&input) else {
        audit_error(store, &input, "request without a tool name", started);
        return None;
    };

    match catch_unwind(AssertUnwindSafe(|| decide(&request))) {
        Ok(verdict) => {
            persist(store, &input, &verdict, started);
            wire::render(&verdict)
        }
        Err(_) => {
            audit_error(store, &input, "decide panicked", started);
            let _ = store.append(store::UNKNOWN, &input);
            None
        }
    }
}

fn persist(store: &Store, input: &Value, verdict: &Verdict, started: Instant) {
    let mut entry = audit_entry(input, started);
    entry["verdict"] = verdict.kind().as_str().into();
    match verdict {
        Verdict::Abstain(Abstain::Unknown { reason }) => {
            entry["reason"] = reason.as_str().into();
            let _ = store.append(store::UNKNOWN, input);
        }
        other => {
            if let Some(rule) = other.rule() {
                entry["rule"] = rule.as_str().into();
            }
        }
    }
    audit(store, &entry);
}

fn audit_error(store: &Store, input: &Value, message: &str, started: Instant) {
    let mut entry = audit_entry(input, started);
    entry["verdict"] = "error".into();
    entry["reason"] = message.into();
    audit(store, &entry);
}

/// `Write` requests carry whole file contents, which the audit trail has no business keeping.
fn audit(store: &Store, entry: &Value) {
    if entry["tool_name"] != "Write" {
        let _ = store.append(store::AUDIT, entry);
    }
}

fn audit_entry(input: &Value, started: Instant) -> Value {
    json!({
        "ts": store::timestamp(),
        "session_id": input.get("session_id"),
        "agent_id": input.get("agent_id"),
        "tool_name": input.get("tool_name"),
        "tool_input": input.get("tool_input"),
        "cwd": input.get("cwd"),
        "duration_us": started.elapsed().as_micros() as u64,
    })
}

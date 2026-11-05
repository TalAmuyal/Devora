use super::Nullable;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// Both directions of the CLI's control channel. Correlation belongs to the
/// envelope's request_id, independently of a tool_use_id inside the request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "subtype", rename_all = "snake_case")]
pub enum ControlRequest {
    Initialize {
        hooks: Nullable<BTreeMap<String, Vec<HookMatcher>>>,
        #[serde(flatten)]
        extra: Map<String, Value>,
    },
    Interrupt {
        #[serde(flatten)]
        extra: Map<String, Value>,
    },
    SetPermissionMode {
        mode: String,
        #[serde(flatten)]
        extra: Map<String, Value>,
    },
    SetModel {
        model: Nullable<String>,
        #[serde(flatten)]
        extra: Map<String, Value>,
    },
    CanUseTool(PermissionRequest),
    HookCallback {
        callback_id: String,
        input: Value,
        tool_use_id: Nullable<String>,
        #[serde(flatten)]
        extra: Map<String, Value>,
    },
    McpMessage {
        server_name: String,
        message: Value,
        #[serde(flatten)]
        extra: Map<String, Value>,
    },
}

/// A CLI permission question. Optional fields remain in `extra` so absent and
/// explicitly null values survive a roundtrip unchanged.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PermissionRequest {
    pub tool_name: String,
    pub input: Value,
    pub tool_use_id: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl PermissionRequest {
    /// The CLI's reason for asking, including a PreToolUse hook's reason.
    pub fn decision_reason(&self) -> Option<&str> {
        self.extra.get("decision_reason")?.as_str()
    }

    /// CLI-provided updates for an "always allow" answer, preserved as wire JSON.
    pub fn permission_suggestions(&self) -> Option<&[Value]> {
        self.extra
            .get("permission_suggestions")?
            .as_array()
            .map(Vec::as_slice)
    }
}

/// Success or error, in either direction. Success payloads depend on the request
/// being answered and are decoded separately after correlating its request_id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "subtype", rename_all = "snake_case")]
pub enum ControlResponse {
    Success {
        request_id: String,
        response: Value,
        #[serde(flatten)]
        extra: Map<String, Value>,
    },
    Error {
        request_id: String,
        error: String,
        #[serde(flatten)]
        extra: Map<String, Value>,
    },
}

/// Hook registration inside an initialize request. CLI spelling is camelCase.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HookMatcher {
    pub matcher: String,
    #[serde(rename = "hookCallbackIds")]
    pub hook_callback_ids: Vec<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A successful initialize response, decoded after request correlation. Additional
/// capability metadata (effort levels, styles and permission mode) stays in extra.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InitializeResponse {
    pub commands: Vec<Value>,
    pub models: Vec<Value>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// An answer to can_use_tool. Hook callbacks use their own hookSpecificOutput
/// JSON rather than this permission response shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "behavior", rename_all = "snake_case")]
pub enum PermissionResponse {
    Allow {
        #[serde(rename = "updatedInput")]
        updated_input: Value,
        #[serde(flatten)]
        extra: Map<String, Value>,
    },
    Deny {
        message: String,
        #[serde(flatten)]
        extra: Map<String, Value>,
    },
}

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

/// Stable, serializable application metadata shared across Caelum interfaces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppInfo {
    pub name: String,
    pub version: String,
}

/// A project path approved and normalized by the Rust core.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub name: String,
    pub path: String,
}

/// A provider-specific request for explicit user approval.
///
/// `provider_details` is intentionally opaque to Caelum so adapters can retain
/// provider-specific permission semantics instead of coercing them into a
/// generic permission mode.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionRequest {
    pub provider: String,
    pub request_id: String,
    pub summary: String,
    pub description: String,
    pub provider_details: serde_json::Value,
}

/// The selected fields shown to the user for an approval request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalPrompt {
    pub provider: String,
    pub request_id: String,
    pub summary: String,
    pub description: String,
}

/// Decisions are one-shot: there is deliberately no persistent "allow all".
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ApprovalDecision {
    AllowOnce,
    #[default]
    Deny,
}

/// A Codex thread as returned by the local app-server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexThreadInfo {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub preview: String,
    #[serde(default)]
    pub updated_at: Option<u64>,
}

/// The project and thread selected in the desktop Codex workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexSessionInfo {
    pub project: ProjectInfo,
    pub thread: CodexThreadInfo,
}

/// Recent Codex-owned threads scoped to one project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexSessionList {
    pub project: ProjectInfo,
    pub threads: Vec<CodexThreadInfo>,
}

/// A notification or server-initiated request from the Codex app-server.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexEvent {
    pub project_path: String,
    pub method: String,
    pub params: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::CodexEvent;
    use serde_json::json;

    #[test]
    fn codex_events_use_the_frontend_camel_case_contract() {
        let event = CodexEvent {
            project_path: "/workspace/project".into(),
            method: "item/agentMessage/delta".into(),
            params: json!({ "delta": "hello" }),
            request_id: None,
        };

        assert_eq!(
            serde_json::to_value(event).unwrap(),
            json!({
                "projectPath": "/workspace/project",
                "method": "item/agentMessage/delta",
                "params": { "delta": "hello" }
            })
        );
    }
}

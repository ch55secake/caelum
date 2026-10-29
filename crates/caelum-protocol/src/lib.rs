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

/// Project and session metadata restored from Caelum's local workspace database.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistedProject {
    pub project: ProjectInfo,
    pub provider: String,
    pub codex_session_id: Option<String>,
    pub opencode_session_id: Option<String>,
    pub codex_session_ids: Vec<String>,
    pub opencode_session_ids: Vec<String>,
}

/// App-owned workspace metadata. Provider conversation history is not included.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSnapshot {
    pub projects: Vec<PersistedProject>,
    pub active_project_path: Option<String>,
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
    pub project_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
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
    pub project_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
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

/// A session owned by the OpenCode HTTP API.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenCodeSession {
    pub id: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub updated_at: Option<u64>,
}

/// The selected project and OpenCode session shown in the desktop workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenCodeSessionInfo {
    pub project: ProjectInfo,
    pub session: OpenCodeSession,
}

/// Recent OpenCode sessions scoped to one project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenCodeSessionList {
    pub project: ProjectInfo,
    pub sessions: Vec<OpenCodeSession>,
}

/// A flattened entry from OpenCode's persisted session history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenCodeConversationEntry {
    pub id: String,
    pub kind: String,
    pub text: String,
}

/// An event received from OpenCode's `/api/event` server-sent event stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenCodeEvent {
    pub project_path: String,
    pub event_type: String,
    pub data: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::{
        ApprovalPrompt, CodexEvent, OpenCodeEvent, PersistedProject, ProjectInfo, WorkspaceSnapshot,
    };
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

    #[test]
    fn opencode_events_use_the_frontend_camel_case_contract() {
        let event = OpenCodeEvent {
            project_path: "/workspace/project".into(),
            event_type: "session.text.delta".into(),
            data: json!({ "sessionID": "ses_1", "delta": "hello" }),
            location: None,
        };

        assert_eq!(
            serde_json::to_value(event).unwrap(),
            json!({
                "projectPath": "/workspace/project",
                "eventType": "session.text.delta",
                "data": { "sessionID": "ses_1", "delta": "hello" }
            })
        );
    }

    #[test]
    fn approval_prompts_include_project_and_session_context() {
        let prompt = ApprovalPrompt {
            provider: "opencode".into(),
            project_path: "/workspace/project".into(),
            session_id: Some("ses_1".into()),
            request_id: "ses_1:per_1".into(),
            summary: "Approve command".into(),
            description: "Run a command".into(),
        };

        assert_eq!(
            serde_json::to_value(prompt).unwrap(),
            json!({
                "provider": "opencode",
                "projectPath": "/workspace/project",
                "sessionId": "ses_1",
                "requestId": "ses_1:per_1",
                "summary": "Approve command",
                "description": "Run a command"
            })
        );
    }

    #[test]
    fn workspace_snapshot_uses_the_frontend_restore_contract() {
        let snapshot = WorkspaceSnapshot {
            projects: vec![PersistedProject {
                project: ProjectInfo {
                    name: "project".into(),
                    path: "/workspace/project".into(),
                },
                provider: "opencode".into(),
                codex_session_id: Some("thread-1".into()),
                opencode_session_id: None,
                codex_session_ids: vec!["thread-1".into()],
                opencode_session_ids: vec![],
            }],
            active_project_path: Some("/workspace/project".into()),
        };

        assert_eq!(
            serde_json::to_value(snapshot).unwrap(),
            json!({
                "projects": [{
                    "project": { "name": "project", "path": "/workspace/project" },
                    "provider": "opencode",
                    "codexSessionId": "thread-1",
                    "opencodeSessionId": null,
                    "codexSessionIds": ["thread-1"],
                    "opencodeSessionIds": []
                }],
                "activeProjectPath": "/workspace/project"
            })
        );
    }
}

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

/// A changed path reported by Git's porcelain status format.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitFileStatus {
    pub path: String,
    pub index_status: String,
    pub worktree_status: String,
}

/// Read-only Git status scoped to the selected project directory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitRepositoryStatus {
    pub project_path: String,
    pub repository_root: Option<String>,
    pub branch: Option<String>,
    pub changes: Vec<GitFileStatus>,
}

/// Separate staged, unstaged, and untracked diffs for one changed path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitFileDiff {
    pub path: String,
    pub staged: String,
    pub unstaged: String,
    pub untracked: String,
}

/// Bounded before/working-tree text for Monaco's side-by-side diff editor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitFileVersions {
    pub path: String,
    pub original: String,
    pub modified: String,
    pub truncated: bool,
}

/// Kind of an entry in the selected project's filesystem tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProjectEntryKind {
    File,
    Directory,
    Symlink,
}

/// A single, non-recursive entry in a project directory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFileEntry {
    pub name: String,
    pub path: String,
    pub kind: ProjectEntryKind,
    pub size_bytes: Option<u64>,
}

/// One paginated directory listing; child directories are fetched only when expanded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDirectoryListing {
    pub path: String,
    pub entries: Vec<ProjectFileEntry>,
    pub total: usize,
    pub next_offset: Option<usize>,
}

/// A bounded UTF-8 preview of a file inside the selected project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFileContent {
    pub path: String,
    pub content: String,
    pub size_bytes: u64,
    pub truncated: bool,
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
    #[serde(default)]
    pub is_working: bool,
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
    #[serde(default)]
    pub is_working: bool,
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

/// One chronological page of provider-owned history, retaining native tool details.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionHistoryPage {
    pub data: Vec<serde_json::Value>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderChoice {
    pub id: String,
    pub name: String,
}

/// Choices reported by the installed provider, rather than hard-coded UI options.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderChoices {
    pub models: Vec<ProviderChoice>,
    pub agents: Vec<ProviderChoice>,
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
        ApprovalPrompt, CodexEvent, GitFileDiff, GitFileStatus, GitFileVersions,
        GitRepositoryStatus, OpenCodeEvent, PersistedProject, ProjectDirectoryListing,
        ProjectEntryKind, ProjectFileContent, ProjectFileEntry, ProjectInfo, WorkspaceSnapshot,
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

    #[test]
    fn git_status_and_diff_use_the_frontend_camel_case_contract() {
        let status = GitRepositoryStatus {
            project_path: "/workspace/project".into(),
            repository_root: Some("/workspace".into()),
            branch: Some("main".into()),
            changes: vec![GitFileStatus {
                path: "src/main.rs".into(),
                index_status: "M".into(),
                worktree_status: " ".into(),
            }],
        };
        let diff = GitFileDiff {
            path: "src/main.rs".into(),
            staged: "staged".into(),
            unstaged: "unstaged".into(),
            untracked: String::new(),
        };

        assert_eq!(
            serde_json::to_value(status).unwrap(),
            json!({
                "projectPath": "/workspace/project",
                "repositoryRoot": "/workspace",
                "branch": "main",
                "changes": [{
                    "path": "src/main.rs",
                    "indexStatus": "M",
                    "worktreeStatus": " "
                }]
            })
        );
        assert_eq!(
            serde_json::to_value(diff).unwrap(),
            json!({
                "path": "src/main.rs",
                "staged": "staged",
                "unstaged": "unstaged",
                "untracked": ""
            })
        );
    }

    #[test]
    fn file_browser_and_version_responses_use_the_frontend_contract() {
        let listing = ProjectDirectoryListing {
            path: "src".into(),
            entries: vec![ProjectFileEntry {
                name: "main.rs".into(),
                path: "src/main.rs".into(),
                kind: ProjectEntryKind::File,
                size_bytes: Some(42),
            }],
            total: 201,
            next_offset: Some(200),
        };
        let content = ProjectFileContent {
            path: "src/main.rs".into(),
            content: "fn main() {}".into(),
            size_bytes: 42,
            truncated: false,
        };
        let versions = GitFileVersions {
            path: "src/main.rs".into(),
            original: "before".into(),
            modified: "after".into(),
            truncated: false,
        };

        assert_eq!(
            serde_json::to_value(listing).unwrap(),
            json!({
                "path": "src",
                "entries": [{
                    "name": "main.rs",
                    "path": "src/main.rs",
                    "kind": "file",
                    "sizeBytes": 42
                }],
                "total": 201,
                "nextOffset": 200
            })
        );
        assert_eq!(
            serde_json::to_value(content).unwrap(),
            json!({
                "path": "src/main.rs",
                "content": "fn main() {}",
                "sizeBytes": 42,
                "truncated": false
            })
        );
        assert_eq!(
            serde_json::to_value(versions).unwrap(),
            json!({
                "path": "src/main.rs",
                "original": "before",
                "modified": "after",
                "truncated": false
            })
        );
    }
}

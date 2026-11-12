use super::events::publish_event;
use super::{valid_session_id, EventSubscribers, OpenCodeServer, APPROVAL_TIMEOUT, PROVIDER_NAME};
use crate::approvals::ApprovalBroker;
use caelum_protocol::{ApprovalDecision, OpenCodeEvent, PermissionRequest};
use reqwest::Method;
use serde_json::{json, Value};
use std::{
    collections::HashSet,
    path::Path,
    sync::{Arc, Mutex},
    thread,
};

pub(super) fn request_permission(
    server: &OpenCodeServer,
    approvals: &ApprovalBroker,
    project_path: &Path,
    subscribers: &EventSubscribers,
    data: &Value,
) {
    let Some(session_id) = data.get("sessionID").and_then(Value::as_str) else {
        return;
    };
    let Some(permission_id) = data.get("id").and_then(Value::as_str) else {
        return;
    };
    if !valid_session_id(session_id) || !valid_permission_id(permission_id) {
        return;
    }
    let action = data
        .get("action")
        .and_then(Value::as_str)
        .unwrap_or("tool action");
    let resources = data
        .get("resources")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    let message = data
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let description = [message, resources.as_str()]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    let request_id = format!("{session_id}:{permission_id}");
    let request = PermissionRequest {
        provider: PROVIDER_NAME.to_owned(),
        project_path: project_path.to_string_lossy().into_owned(),
        session_id: Some(session_id.to_owned()),
        request_id,
        summary: format!("OpenCode requests permission for {action}"),
        description: if description.is_empty() {
            "OpenCode requested permission to continue.".to_owned()
        } else {
            description
        },
        provider_details: json!({
            "sessionId": session_id,
            "permissionId": permission_id,
            "projectPath": project_path.to_string_lossy(),
            "request": data,
        }),
    };
    let Ok(waiter) = approvals.request(request) else {
        return;
    };

    server
        .pending_permission_ids
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(format!("{session_id}:{permission_id}"));
    let server = server.clone_for_permission();
    let subscribers = Arc::clone(subscribers);
    let project_path = project_path.to_path_buf();
    let session_id = session_id.to_owned();
    let permission_id = permission_id.to_owned();
    let request_id = format!("{session_id}:{permission_id}");
    thread::spawn(move || {
        let decision = waiter.wait(APPROVAL_TIMEOUT);
        let decision = permission_reply(decision);
        let path = format!("/api/session/{session_id}/permission/{permission_id}/reply");
        let result = server.request_json(
            Method::POST,
            &path,
            &[],
            Some(json!({ "decision": decision })),
        );
        server
            .pending_permission_ids
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&request_id);
        if let Err(error) = result {
            publish_event(
                &subscribers,
                OpenCodeEvent {
                    project_path: project_path.to_string_lossy().into_owned(),
                    event_type: "permission.reply.failed".to_owned(),
                    id: None,
                    created: None,
                    data: json!({ "sessionID": session_id, "message": error.to_string() }),
                    location: Some(json!({ "directory": project_path.to_string_lossy() })),
                },
            );
        }
    });
}

pub(super) fn permission_reply(decision: ApprovalDecision) -> &'static str {
    match decision {
        ApprovalDecision::AllowOnce => "once",
        ApprovalDecision::Deny => "reject",
    }
}

pub(super) fn deny_pending_approvals(
    approvals: &ApprovalBroker,
    project_path: Option<&str>,
    request_ids: Option<&HashSet<String>>,
) {
    for request in approvals.pending().into_iter().filter(|request| {
        if request.provider != PROVIDER_NAME {
            return false;
        }
        project_path.is_none_or(|path| request.project_path == path)
            && request_ids.is_none_or(|request_ids| request_ids.contains(&request.request_id))
    }) {
        let _ = approvals.resolve(
            PROVIDER_NAME,
            &request.project_path,
            &request.request_id,
            ApprovalDecision::Deny,
        );
    }
}

fn valid_permission_id(value: &str) -> bool {
    value.starts_with("per")
        && value.len() > 3
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
}

impl OpenCodeServer {
    fn clone_for_permission(&self) -> Self {
        Self {
            process_id: self.process_id,
            base_url: self.base_url.clone(),
            client: self.client.clone(),
            event_client: self.event_client.clone(),
            password: self.password.clone(),
            alive: Arc::clone(&self.alive),
            session_ids: Mutex::new(
                self.session_ids
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .clone(),
            ),
            pending_permission_ids: Arc::clone(&self.pending_permission_ids),
        }
    }
}

use super::permissions::{deny_pending_approvals, request_permission};
use super::questions::question_request;
use super::transport::{api_url, parse_event, read_sse_frames};
use super::{
    directory_matches, EventSubscribers, OpenCodeError, OpenCodeServer, PROVIDER_NAME,
    SERVER_START_TIMEOUT,
};
use crate::{approvals::ApprovalBroker, questions::QuestionBroker};
use caelum_protocol::OpenCodeEvent;
use serde_json::{json, Value};
use std::{
    io::BufReader,
    path::{Path, PathBuf},
    sync::{atomic::Ordering, mpsc, Arc},
    thread,
};

pub(super) fn spawn_event_listener(
    server: Arc<OpenCodeServer>,
    project_path: PathBuf,
    approvals: ApprovalBroker,
    questions: QuestionBroker,
    subscribers: EventSubscribers,
) -> Result<(), OpenCodeError> {
    let (ready, receiver) = mpsc::channel();
    thread::spawn(move || {
        let url = match api_url(&server.base_url, "/api/event", &[]) {
            Ok(url) => url,
            Err(_) => return,
        };
        let request = server.authenticate(server.event_client.get(url));
        let result = request.send();
        let Ok(response) = result else {
            publish_disconnected(&server, &project_path, &approvals, &questions, &subscribers);
            return;
        };
        if !response.status().is_success() {
            publish_disconnected(&server, &project_path, &approvals, &questions, &subscribers);
            return;
        }

        let mut reader = BufReader::new(response);
        let mut ready = Some(ready);
        let _ = read_sse_frames(
            &mut reader,
            || server.alive.load(Ordering::Acquire),
            |payload| {
                let Ok(event) = parse_event(&payload, &project_path) else {
                    return;
                };
                // Wait for the stream's first frame before admitting prompts or
                // fetching snapshots: SSE subscriptions have no replay.
                if let Some(ready) = ready.take() {
                    let _ = ready.send(());
                }
                if !server.alive.load(Ordering::Acquire) {
                    return;
                }
                // A resolution can arrive during the first snapshot, before
                // its session is known locally. Invalidate that snapshot even
                // when a location-less event cannot otherwise be routed yet.
                if matches!(event.event_type.as_str(), "form.replied" | "form.cancelled")
                    && event
                        .location
                        .as_ref()
                        .and_then(|location| location["directory"].as_str())
                        .is_none_or(|directory| directory_matches(directory, &project_path))
                {
                    if let (Some(session), Some(id)) =
                        (event.data["sessionID"].as_str(), event.data["id"].as_str())
                    {
                        questions.dismiss(
                            PROVIDER_NAME,
                            &project_path.to_string_lossy(),
                            session,
                            id,
                        );
                    }
                }
                if !event_belongs_to_project(
                    event.location.as_ref(),
                    event.data.get("form").unwrap_or(&event.data),
                    &project_path,
                    &server,
                ) {
                    return;
                }

                if event.event_type == "permission.asked" {
                    request_permission(
                        &server,
                        &approvals,
                        &project_path,
                        &subscribers,
                        &event.data,
                    );
                }
                match event.event_type.as_str() {
                    "form.created" => {
                        if let Ok((prompt, reply)) =
                            question_request(&server, &project_path, &event.data["form"])
                        {
                            let _ = questions.insert(prompt, reply);
                        }
                    }
                    "session.deleted" | "session.archived" | "session.execution.interrupted" => {
                        if let Some(session) = event.data["sessionID"].as_str() {
                            questions.clear(
                                PROVIDER_NAME,
                                &project_path.to_string_lossy(),
                                Some(session),
                            );
                        }
                    }
                    _ => {}
                }
                publish_event(&subscribers, event);
            },
        );
        publish_disconnected(&server, &project_path, &approvals, &questions, &subscribers);
    });
    receiver
        .recv_timeout(SERVER_START_TIMEOUT)
        .map_err(|error| OpenCodeError::ServerNotReady(format!("event subscription: {error}")))
}

fn event_belongs_to_project(
    location: Option<&Value>,
    data: &Value,
    project_path: &Path,
    server: &OpenCodeServer,
) -> bool {
    if let Some(directory) = location
        .and_then(|location| location.get("directory"))
        .and_then(Value::as_str)
    {
        return directory_matches(directory, project_path);
    }
    data.get("sessionID")
        .and_then(Value::as_str)
        .is_some_and(|session_id| server.owns_session(session_id))
}

fn publish_disconnected(
    server: &OpenCodeServer,
    project_path: &Path,
    approvals: &ApprovalBroker,
    questions: &QuestionBroker,
    subscribers: &EventSubscribers,
) {
    if server.alive.load(Ordering::Acquire) {
        questions.clear(PROVIDER_NAME, &project_path.to_string_lossy(), None);
    }
    if server.alive.swap(false, Ordering::AcqRel) {
        let pending_ids = server
            .pending_permission_ids
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        let project_path = project_path.to_string_lossy().into_owned();
        deny_pending_approvals(approvals, Some(&project_path), Some(&pending_ids));
        publish_event(
            subscribers,
            OpenCodeEvent {
                project_path: project_path.clone(),
                event_type: "server.disconnected".to_owned(),
                id: None,
                created: None,
                data: Value::Null,
                location: Some(json!({ "directory": project_path })),
            },
        );
    }
}

pub(super) fn publish_event(subscribers: &EventSubscribers, event: OpenCodeEvent) {
    subscribers
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .retain(|subscriber| subscriber.send(event.clone()).is_ok());
}

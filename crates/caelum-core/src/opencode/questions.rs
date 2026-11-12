use super::{valid_session_id, OpenCodeError, OpenCodeServer, PROVIDER_NAME};
use crate::questions::{QuestionBroker, Reply};
use caelum_protocol::QuestionPrompt;
use reqwest::Method;
use serde_json::{json, Value};
use std::{
    path::Path,
    sync::{atomic::Ordering, Arc},
};

pub(super) fn question_request(
    server: &Arc<OpenCodeServer>,
    project: &Path,
    form: &Value,
) -> Result<(QuestionPrompt, Reply), OpenCodeError> {
    let session = form["sessionID"]
        .as_str()
        .filter(|id| valid_session_id(id))
        .ok_or_else(|| OpenCodeError::Protocol("form has no valid session ID".into()))?;
    let id = form["id"]
        .as_str()
        .filter(|id| {
            id.starts_with("frm_")
                && id.len() > 4
                && id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        })
        .ok_or_else(|| OpenCodeError::Protocol("form has no valid ID".into()))?;
    let fields = form["fields"]
        .as_array()
        .ok_or_else(|| OpenCodeError::Protocol("form has no fields".into()))?;
    let prompt = QuestionPrompt {
        provider: PROVIDER_NAME.into(),
        project_path: project.to_string_lossy().into_owned(),
        session_id: session.into(),
        request_id: id.into(),
        title: form["title"].as_str().unwrap_or("Questions").into(),
        fields: fields.clone(),
    };
    let path = format!("/api/session/{session}/form/{id}");
    server.remember_session(session);
    let server = Arc::clone(server);
    let reply: Reply = Arc::new(move |answer| {
        if !server.alive.load(Ordering::Acquire) {
            return Err("OpenCode disconnected. Reconnect before answering.".into());
        }
        let result = match answer {
            Some(answer) => server.request_json(
                Method::POST,
                &format!("{path}/reply"),
                &[],
                Some(json!({ "answer": answer })),
            ),
            None => server.request_json(Method::DELETE, &path, &[], None),
        };
        result.map(|_| ()).map_err(|error| error.to_string())
    });
    Ok((prompt, reply))
}

pub(super) fn sync_questions(
    server: &Arc<OpenCodeServer>,
    questions: &QuestionBroker,
    project: &Path,
) -> Result<(), OpenCodeError> {
    let project_path = project.to_string_lossy();
    for _ in 0..2 {
        let revision = questions.revision();
        let response = server.request_json(
            Method::GET,
            "/api/form",
            &[("location[directory]", project_path.to_string())],
            None,
        )?;
        let forms = response["data"]
            .as_array()
            .ok_or_else(|| OpenCodeError::Protocol("form list has no data array".into()))?;
        let requests = forms
            .iter()
            // Global MCP forms do not belong to a conversation.
            .filter(|form| form["sessionID"].as_str().is_some_and(valid_session_id))
            .map(|form| question_request(server, project, form))
            .collect::<Result<Vec<_>, _>>()?;
        if !server.alive.load(Ordering::Acquire) {
            return Ok(());
        }
        if questions
            .sync(PROVIDER_NAME, &project_path, revision, requests)
            .map_err(OpenCodeError::Protocol)?
        {
            break;
        }
    }
    Ok(())
}

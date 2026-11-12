use super::transport::write_message;
use super::{project_key, request_id_string, APPROVAL_TIMEOUT, PROVIDER_NAME};
use crate::{approvals::ApprovalBroker, questions::QuestionBroker};
use caelum_protocol::{ApprovalDecision, PermissionRequest, QuestionPrompt};
use serde_json::{json, Value};
use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
};

pub(super) fn request_question(
    id: &Value,
    params: &Value,
    writer: &Arc<Mutex<std::process::ChildStdin>>,
    alive: &Arc<AtomicBool>,
    questions: &QuestionBroker,
    project: &Path,
) -> Result<(), String> {
    request_id_string(id).map_err(|_| "Invalid question request ID")?;
    let session_id = params["threadId"]
        .as_str()
        .ok_or("Question has no thread ID")?;
    let values = params["questions"]
        .as_array()
        .ok_or("Question request has no questions")?;
    let fields: Vec<Value> = values.iter().map(|question| {
        let key = question["id"].as_str().ok_or("Question has no ID")?;
        let text = question["question"].as_str().ok_or("Question has no text")?;
        let options: Vec<Value> = question["options"].as_array().into_iter().flatten().map(|option| {
            let label = option["label"].as_str().ok_or("Question option has no label")?;
            Ok(json!({ "value": label, "label": label, "description": option["description"].as_str().unwrap_or("") }))
        }).collect::<Result<_, String>>()?;
        Ok(json!({
            "key": key, "type": "string", "title": question["header"].as_str().unwrap_or("Question"),
            "description": text, "required": true,
            "custom": options.is_empty() || question["isOther"].as_bool().unwrap_or(false),
            "secret": question["isSecret"].as_bool().unwrap_or(false), "options": options,
        }))
    }).collect::<Result<_, String>>()?;
    let prompt = QuestionPrompt {
        provider: PROVIDER_NAME.into(),
        project_path: project.to_string_lossy().into_owned(),
        session_id: session_id.into(),
        request_id: id.to_string(),
        title: "Questions".into(),
        fields: fields.clone(),
    };
    let id = id.clone();
    let writer = Arc::clone(writer);
    let alive = Arc::clone(alive);
    questions.insert(
        prompt,
        Arc::new(move |answer| {
            if !alive.load(Ordering::Acquire) {
                return Err("Codex disconnected before the answer could be sent".into());
            }
            let mut answers = serde_json::Map::new();
            if let Some(answer) = answer {
                if answer
                    .keys()
                    .any(|key| !fields.iter().any(|field| field["key"] == *key))
                {
                    return Err("Answer contains an unknown question".into());
                }
                for field in &fields {
                    let key = field["key"].as_str().expect("validated question key");
                    let text = answer
                        .get(key)
                        .and_then(Value::as_str)
                        .filter(|text| !text.trim().is_empty())
                        .ok_or_else(|| {
                            format!(
                                "Answer {} before submitting",
                                field["title"].as_str().unwrap_or(key)
                            )
                        })?;
                    if field["custom"] != true
                        && !field["options"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .any(|option| option["value"] == text)
                    {
                        return Err("Choose one of the available answers".into());
                    }
                    answers.insert(key.into(), json!({ "answers": [text] }));
                }
            }
            write_message(
                &writer,
                &json!({ "id": id, "result": { "answers": answers } }),
            )
        }),
    )
}

pub(super) fn handle_server_request(
    method: &str,
    request_id: Value,
    params: Value,
    writer: &Arc<Mutex<std::process::ChildStdin>>,
    approvals: &ApprovalBroker,
    project_path: &Path,
) {
    if !matches!(
        method,
        "item/commandExecution/requestApproval" | "item/fileChange/requestApproval"
    ) {
        let _ = write_message(
            writer,
            &json!({
                "id": request_id,
                "error": { "code": -32601, "message": "Unsupported Codex app-server request" }
            }),
        );
        return;
    }

    let Ok(request_key) = request_id_string(&request_id) else {
        let _ = write_message(
            writer,
            &json!({
                "id": request_id,
                "error": { "code": -32600, "message": "Codex approval request ID must be a string or number" }
            }),
        );
        return;
    };
    let opaque_id = format!("{}:{request_key}", project_key(project_path));
    let command = method == "item/commandExecution/requestApproval";
    let reason = params.get("reason").and_then(Value::as_str).unwrap_or("");
    let command_text = command_description(params.get("command"));
    let cwd = params.get("cwd").and_then(Value::as_str).unwrap_or("");
    let summary = if command {
        "Codex requests approval to run a command"
    } else {
        "Codex requests approval to change files"
    };
    let mut details = Vec::new();
    if !reason.trim().is_empty() {
        details.push(reason.trim().to_owned());
    }
    if command && !command_text.is_empty() {
        details.push(format!("Command: {command_text}"));
    }
    if !cwd.trim().is_empty() {
        details.push(format!("Working directory: {cwd}"));
    }
    if let Some(grant_root) = params.get("grantRoot").and_then(Value::as_str) {
        details.push(format!("Requested file access root: {grant_root}"));
    }
    if let Some(host) = params
        .get("networkApprovalContext")
        .and_then(|context| context.get("host"))
        .and_then(Value::as_str)
    {
        details.push(format!("Requested network destination: {host}"));
    }
    if details.is_empty() {
        details.push("Codex needs your permission to continue this operation.".into());
    }

    let request = PermissionRequest {
        provider: PROVIDER_NAME.to_owned(),
        project_path: project_path.to_string_lossy().into_owned(),
        session_id: params
            .get("threadId")
            .and_then(Value::as_str)
            .or_else(|| {
                params
                    .get("thread")
                    .and_then(|thread| thread.get("id"))
                    .and_then(Value::as_str)
            })
            .map(str::to_owned),
        request_id: opaque_id,
        summary: summary.to_owned(),
        description: details.join("\n"),
        provider_details: json!({ "method": method, "params": params }),
    };
    let waiter = match approvals.request(request.clone()) {
        Ok(waiter) => waiter,
        Err(_) => {
            let _ = respond_to_approval(writer, request_id, "decline");
            return;
        }
    };
    let writer = Arc::clone(writer);
    let available = request
        .provider_details
        .get("params")
        .and_then(|value| value.get("availableDecisions"))
        .and_then(Value::as_array)
        .map(|options| {
            options
                .iter()
                .filter_map(Value::as_str)
                .any(|option| option == "accept")
        })
        .unwrap_or(true);
    thread::spawn(move || {
        let decision = waiter.wait(APPROVAL_TIMEOUT);
        let response_decision = if decision == ApprovalDecision::AllowOnce && available {
            "accept"
        } else {
            "decline"
        };
        let _ = respond_to_approval(&writer, request_id, response_decision);
    });
}

fn respond_to_approval(
    writer: &Arc<Mutex<std::process::ChildStdin>>,
    request_id: Value,
    decision: &str,
) -> Result<(), String> {
    write_message(
        writer,
        &json!({ "id": request_id, "result": { "decision": decision } }),
    )
}

fn command_description(command: Option<&Value>) -> String {
    match command {
        Some(Value::Array(parts)) => parts
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join(" "),
        Some(Value::String(command)) => command.clone(),
        _ => String::new(),
    }
}

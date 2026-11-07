use std::{
    io::{self, BufRead, BufReader},
    path::Path,
    process::ChildStdout,
    sync::mpsc::{self, Receiver},
    thread,
};

use caelum_protocol::OpenCodeEvent;
use reqwest::{StatusCode, Url};
use serde_json::Value;

use super::OpenCodeError;

pub(super) fn read_sse_frames<R, F>(
    reader: &mut R,
    mut active: impl FnMut() -> bool,
    mut on_data: F,
) -> io::Result<()>
where
    R: BufRead,
    F: FnMut(String),
{
    let mut data = Vec::new();
    while active() {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            if !data.is_empty() {
                on_data(data.join("\n"));
            }
            return Ok(());
        }
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            if !data.is_empty() {
                on_data(data.join("\n"));
                data.clear();
            }
            continue;
        }
        if line.starts_with(':') {
            continue;
        }
        let (field, value) = line.split_once(':').unwrap_or((line, ""));
        if field == "data" {
            data.push(value.strip_prefix(' ').unwrap_or(value).to_owned());
        }
    }
    Ok(())
}

pub(super) fn capture_server_password(stdout: ChildStdout) -> Receiver<Result<String, String>> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut sent_password = false;
        for line in BufReader::new(stdout).lines() {
            match line {
                Ok(line) => {
                    if !sent_password {
                        if let Some(password) = line.strip_prefix("server password ") {
                            sent_password = true;
                            if sender.send(Ok(password.to_owned())).is_err() {
                                return;
                            }
                        }
                    }
                }
                Err(error) => {
                    if !sent_password {
                        let _ = sender.send(Err(error.to_string()));
                    }
                    return;
                }
            }
        }
        if !sent_password {
            let _ = sender.send(Err(
                "OpenCode exited without printing its generated server password".to_owned(),
            ));
        }
    });
    receiver
}

pub(super) fn api_url(
    base_url: &str,
    path: &str,
    query: &[(&str, String)],
) -> Result<Url, OpenCodeError> {
    let mut url = Url::parse(base_url)
        .map_err(|error| OpenCodeError::Protocol(format!("invalid server URL: {error}")))?;
    url.set_path(path);
    if !query.is_empty() {
        let mut pairs = url.query_pairs_mut();
        for (name, value) in query {
            pairs.append_pair(name, value);
        }
    }
    Ok(url)
}

pub(super) fn decode_json_response(
    status: StatusCode,
    body: &[u8],
) -> Result<Value, OpenCodeError> {
    if status == StatusCode::UNAUTHORIZED {
        return Err(OpenCodeError::AuthenticationRequired);
    }
    if !status.is_success() {
        let message = serde_json::from_slice::<Value>(body)
            .ok()
            .and_then(|value| {
                value
                    .get("message")
                    .or_else(|| value.get("error"))
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            })
            .unwrap_or_else(|| String::from_utf8_lossy(body).trim().to_owned())
            .chars()
            .take(600)
            .collect();
        return Err(OpenCodeError::HttpStatus {
            status: status.as_u16(),
            message,
        });
    }
    if body.is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_slice(body)
        .map_err(|error| OpenCodeError::Protocol(format!("invalid JSON response: {error}")))
}

pub(super) fn parse_event(
    payload: &str,
    project_path: &Path,
) -> Result<OpenCodeEvent, OpenCodeError> {
    let value = serde_json::from_str::<Value>(payload)
        .map_err(|error| OpenCodeError::Protocol(format!("invalid SSE event JSON: {error}")))?;
    let event_type = value
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| OpenCodeError::Protocol("SSE event has no type".to_owned()))?;
    Ok(OpenCodeEvent {
        project_path: project_path.to_string_lossy().into_owned(),
        event_type: event_type.to_owned(),
        id: value.get("id").and_then(Value::as_str).map(str::to_owned),
        created: value.get("created").and_then(Value::as_u64),
        data: value.get("data").cloned().unwrap_or(Value::Null),
        location: value.get("location").cloned(),
    })
}

mod content;
use super::tools::{Call, MAX_CURSOR};
use crate::remote::{Line, Run, Speaker, ToolSource};
use boat_sdk::agents::models::SandboxEvent;
use serde_json::{Map, Value};

// Unlike a conversation filter, an event/call ID is an opaque equality key.
pub(super) fn event_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 256 && !id.chars().any(|c| c.is_control() || c.is_whitespace())
}

pub(super) fn response<'a>(
    e: &'a SandboxEvent,
    chat: &str,
    prompt: &str,
) -> Option<&'a Map<String, Value>> {
    let task = e.task_id.as_deref();
    let conversation = e.conversation_id.as_deref();
    if e.kind != "response"
        || task.is_some_and(|id| id != prompt)
        || conversation.is_some_and(|id| id != chat)
        || (conversation.is_none() && task.is_none())
    {
        return None;
    }
    e.data
        .as_ref()
        .filter(|d| d.get("is_reverted").and_then(Value::as_bool) != Some(true))
}

/// Boat sends snapshots: the tool result is paired with its `use`, not another
/// chat message. Keep call identity and outcome without copying raw inputs or output.
pub(super) fn lines(
    events: Vec<SandboxEvent>,
    run: &Run,
    root: Option<&str>,
    cursor: Option<&str>,
) -> Vec<Line> {
    let mut lines = Vec::new();
    for (idx, e) in events.iter().enumerate() {
        let Some(data) = response(e, &run.conversation, &run.prompt) else {
            continue;
        };
        let event = e.id.as_deref().filter(|id| event_id(id));
        content::append(&mut lines, data, event, run);
        let Some(tools) = data.get("tools").and_then(Value::as_array) else {
            continue;
        };
        let previous = idx
            .checked_sub(1)
            .and_then(|i| super::harness::position(&events[i]));
        let before = previous.as_deref().or(cursor);
        for (i, tool) in tools.iter().take(200).enumerate() {
            let Some(call) = Call::read(tool) else {
                continue;
            };
            let key = call
                .id
                .map(|id| format!("{}:tool:{id}", run.id))
                .or_else(|| event.map(|id| format!("{}:event-tool:{id}:{i}", run.id)));
            let name = call
                .name
                .chars()
                .filter(|c| !c.is_control())
                .take(80)
                .collect::<String>();
            let source = event
                .filter(|_| before.is_none_or(|s| s.len() <= MAX_CURSOR))
                .and_then(|event| {
                    Some(Box::new(ToolSource {
                        conversation: run.conversation.clone(),
                        prompt: run.prompt.clone(),
                        event: event.into(),
                        before: before.map(str::to_owned),
                        call: call.identity(i)?,
                    }))
                });
            lines.push(Line {
                key,
                tool_detail: call.input.and_then(|v| detail(&name, v, root)),
                tool_source: source,
                ..Line::new(Speaker::Tool(call.state), name)
            });
        }
    }
    lines
}

fn detail(name: &str, input: &serde_json::Value, root: Option<&str>) -> Option<String> {
    match name.to_ascii_lowercase().as_str() {
        "bash" | "shell" | "exec_command" | "commandexecution" => {
            let word = input
                .get("command")
                .or_else(|| input.get("cmd"))?
                .as_str()?
                .split_whitespace()
                .next()?;
            if !word
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"/._-+".contains(&b))
            {
                return None;
            }
            let program = word.rsplit('/').next()?;
            (!program.is_empty() && program.len() <= 40).then(|| program.to_owned())
        }
        "read" | "read_file" | "edit" | "write" | "grep" | "glob" => {
            let path = ["filePath", "file_path", "path"]
                .iter()
                .find_map(|p| input.get(p)?.as_str())?;
            let path = if path.starts_with('/') {
                path.strip_prefix(root?)?.strip_prefix('/')?
            } else {
                path.trim_start_matches("./")
            };
            (!path.is_empty()
                && !path.starts_with(['~', '/'])
                && path.len() <= 160
                && !path.chars().any(char::is_control)
                && !path.split('/').any(|p| p == ".."))
            .then(|| path.to_owned())
        }
        _ => None,
    }
}

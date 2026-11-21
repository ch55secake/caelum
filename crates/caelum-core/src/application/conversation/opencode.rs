use serde_json::Value;

use super::{
    agent_name, display, model_id, part_id, set_metadata, shells, string, subagents, text_content,
    Conversation, Entry, EntryKind, Provider,
};
use crate::application::presentation;

#[derive(Debug)]
pub(super) struct Step {
    message_id: String,
    started_at: u64,
    model: Option<String>,
}

impl Conversation {
    pub(super) fn opencode(&mut self, method: &str, data: &Value) {
        if !self.failure.event(method, data) {
            return;
        }
        if method == "session.execution.failed" {
            self.activity += 1;
        }
        if method == "session.execution.started" {
            self.provider_started(data);
        }
        if method.starts_with("session.compaction.") {
            let at = data["_created"].as_u64().unwrap_or(0);
            let latest = self
                .entries
                .iter()
                .rposition(|e| e.kind == EntryKind::Compaction);
            let owner = data["inputID"].as_str().map(|id| format!("opencode-{id}"));
            let target = if let Some(id) = &owner {
                self.entries
                    .iter()
                    .position(|e| e.kind == EntryKind::Compaction && &e.id == id)
            } else {
                latest
            };
            if owner.is_none()
                && target.is_some_and(|i| at > 0 && at < self.entries[i].started_at.unwrap_or(0))
            {
                return;
            }
            if method == "session.compaction.started" {
                let id = data["inputID"].as_str().map(str::to_owned).or_else(|| {
                    data["_event_id"]
                        .as_str()
                        .map(|id| id.replacen("evt_", "msg_", 1))
                });
                if let Some(id) = id {
                    let id = format!("opencode-{id}");
                    if !self.entries.iter().any(|e| e.id == id && e.complete) {
                        let mut entry = Entry::new(id, EntryKind::Compaction, String::new(), false);
                        entry.started_at = Some(at);
                        self.put(entry);
                        self.working = true;
                    }
                }
            } else if let Some(i) = target.filter(|i| !self.entries[*i].complete) {
                if method.ends_with("delta") {
                    let id = self.entries[i].id.clone();
                    self.delta(
                        id,
                        EntryKind::Compaction,
                        data["text"]
                            .as_str()
                            .or(data["delta"].as_str())
                            .unwrap_or_default(),
                        false,
                    );
                } else if method.ends_with("ended") || method.ends_with("failed") {
                    let mut entry = self.entries[i].clone();
                    entry.complete = true;
                    entry.status = if method.ends_with("failed") {
                        "failed"
                    } else {
                        "completed"
                    }
                    .into();
                    if let Some(text) = data["text"]
                        .as_str()
                        .or(data["summary"].as_str())
                        .filter(|text| !text.is_empty())
                    {
                        entry.text = text.into();
                    }
                    entry.error = display(data["error"].get("message").unwrap_or(&data["error"]));
                    entry.metadata = serde_json::json!({"ended": at});
                    self.put(entry);
                }
            }
            if method.ends_with("ended") && at >= self.usage_at {
                self.used_tokens = None;
                self.usage_at = at;
            }
            return;
        }
        if matches!(method, "session.shell.started" | "session.shell.ended") {
            if let Some(entry) = shells::entry(&data["shell"], &data["output"]) {
                if !self
                    .entries
                    .iter()
                    .any(|old| old.id == entry.id && old.complete && !entry.complete)
                {
                    self.put(entry);
                }
            }
            return;
        }
        if method == "session.synthetic" {
            if let Some(entry) = shells::synthetic(data).or_else(|| subagents::synthetic(data)) {
                self.put(entry);
            } else if let Some(id) = data["_event_id"].as_str() {
                self.put(Entry::new(
                    format!("opencode-{}", id.replacen("evt_", "msg_", 1)),
                    EntryKind::Status,
                    display(&data["text"]),
                    true,
                ));
            }
        }
        if method == "session.step.started" {
            self.step = data["assistantMessageID"].as_str().map(|id| Step {
                message_id: id.into(),
                started_at: data["started"]
                    .as_u64()
                    .or(data["_created"].as_u64())
                    .unwrap_or(0),
                model: model_id(data),
            });
        }
        if method == "session.step.ended" {
            let step = if self.step.as_ref().is_some_and(|step| {
                Some(step.message_id.as_str()) == data["assistantMessageID"].as_str()
            }) {
                self.step.take()
            } else {
                None
            };
            let at = step
                .as_ref()
                .map(|step| step.started_at)
                .or(data["_created"].as_u64())
                .unwrap_or(0);
            if self.usage(&data["tokens"], at) {
                self.usage_model = model_id(data).or_else(|| step.and_then(|step| step.model));
            }
        }
        if let Some(rest) = method.strip_prefix("session.") {
            if let Some((kind, phase)) = rest.split_once('.') {
                if matches!(kind, "text" | "reasoning") {
                    let Some(message) = data["assistantMessageID"]
                        .as_str()
                        .or(data["messageID"].as_str())
                    else {
                        return;
                    };
                    let id = part_id(message, kind, data["ordinal"].as_u64().unwrap_or(0));
                    let kind = if kind == "text" {
                        EntryKind::Assistant
                    } else {
                        EntryKind::Reasoning
                    };
                    match phase {
                        "delta" => self.delta(
                            id,
                            kind,
                            data["delta"]
                                .as_str()
                                .or(data["text"].as_str())
                                .unwrap_or(""),
                            false,
                        ),
                        "ended" => {
                            let text =
                                data["text"].as_str().map(str::to_owned).unwrap_or_else(|| {
                                    self.entries
                                        .iter()
                                        .find(|entry| entry.id == id)
                                        .map(|entry| entry.text.clone())
                                        .unwrap_or_default()
                                });
                            let mut entry = Entry::new(id, kind, text, true);
                            entry.metadata = serde_json::json!({"ended":data["_created"].as_u64().unwrap_or_else(presentation::now)});
                            if kind == EntryKind::Assistant {
                                if let Some(agent) = agent_name(&data["agent"]) {
                                    set_metadata(&mut entry, "agent", agent);
                                }
                            }
                            self.put(entry);
                        }
                        "started" if !self.entries.iter().any(|entry| entry.id == id) => {
                            let mut entry = Entry::new(id, kind, String::new(), false);
                            entry.started_at =
                                Some(data["_created"].as_u64().unwrap_or_else(presentation::now));
                            if kind == EntryKind::Assistant {
                                if let Some(agent) = agent_name(&data["agent"]) {
                                    set_metadata(&mut entry, "agent", agent);
                                }
                            }
                            self.put(entry);
                            self.working = true;
                        }
                        _ => {}
                    }
                }
            }
        }
        if method.starts_with("session.tool.") {
            if let Some(id) = data["id"].as_str().or(data["partID"].as_str()) {
                let id = format!("opencode-tool-{id}");
                let previous = self.entries.iter().find(|entry| entry.id == id);
                let complete = method.ends_with("success") || method.ends_with("failed");
                if previous.is_some_and(|entry| entry.complete) && !complete {
                    return;
                }
                let mut entry = previous
                    .cloned()
                    .unwrap_or_else(|| Entry::new(id, EntryKind::Tool, String::new(), false));
                entry.complete = complete;
                entry.status = if method.ends_with("failed") {
                    "failed"
                } else if complete {
                    "completed"
                } else {
                    "running"
                }
                .into();
                if let Some(name) = data["name"].as_str().or(data["tool"].as_str()) {
                    entry.name = name.into();
                }
                if method == "session.tool.input.delta" {
                    entry.input.push_str(string(&data["delta"]));
                } else if !data["input"].is_null() {
                    entry.input = display(&data["input"]);
                } else if method == "session.tool.input.ended" {
                    entry.input = display(&data["text"]);
                }
                if !data["content"].is_null() {
                    entry.output = text_content(&data["content"]);
                    entry.media = presentation::media(&data["content"]);
                } else if !data["output"].is_null() {
                    entry.output = display(&data["output"]);
                } else if let Some(output) = data["metadata"]["output"].as_str() {
                    entry.output = output.into();
                }
                if !data["error"].is_null() {
                    entry.error = display(&data["error"]);
                }
                if !data["metadata"].is_null() {
                    entry.metadata = data["metadata"].clone();
                }
                entry.started_at = entry
                    .started_at
                    .or(data["started"].as_u64())
                    .or(data["_created"].as_u64())
                    .or_else(|| Some(presentation::now()));
                if let Some(ended) = data["_created"].as_u64().filter(|_| complete) {
                    if !entry.metadata.is_object() {
                        entry.metadata = serde_json::json!({});
                    }
                    entry.metadata["ended"] = ended.into();
                }
                subagents::tool(&mut entry);
                if previous.is_some_and(|old| old.complete) && !entry.complete {
                    return;
                }
                if !complete {
                    self.working = true;
                }

                if let Some(index) = self.entries.iter().position(|e| e.id == entry.id) {
                    self.entries[index] = entry;
                } else {
                    self.entries.push(entry);
                }
                subagents::reconcile(&mut self.entries);
            }
        }
        if data["message"]["type"].is_string() {
            self.history(
                Provider::OpenCode,
                std::slice::from_ref(&data["message"]),
                false,
            );
        }
        match method {
            "session.execution.started" | "session.step.started" | "session.retry.scheduled" => {
                self.working = true
            }
            "session.status" => match string(&data["status"]["type"]) {
                "busy" | "retry" => self.working = true,
                "idle" => {
                    self.settle_at(data["_created"].as_u64().unwrap_or_else(presentation::now))
                }
                _ => {}
            },
            "session.execution.succeeded"
            | "session.execution.failed"
            | "session.execution.interrupted"
            | "session.idle" => {
                self.settle_at(data["_created"].as_u64().unwrap_or_else(presentation::now));
            }
            _ => {}
        }
    }

    pub(super) fn usage(&mut self, tokens: &Value, at: u64) -> bool {
        if at < self.usage_at {
            return false;
        }
        let counts = [
            tokens["input"].as_u64(),
            tokens["output"].as_u64(),
            tokens["cache"]["read"].as_u64(),
            tokens["cache"]["write"].as_u64(),
        ];
        let total = counts
            .into_iter()
            .try_fold(0u64, |sum, count| sum.checked_add(count?));
        if let Some(total) = total {
            self.used_tokens = Some(total);
            self.usage_at = at;
            true
        } else {
            false
        }
    }
}

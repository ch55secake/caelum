use std::collections::BTreeMap;

use serde_json::Value;

use super::Provider;

mod failure;
mod opencode;
mod reconciliation;
pub(super) mod shells;
mod subagents;
mod timing;
#[cfg(test)]
mod timing_tests;
mod turn;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    User,
    Assistant,
    Reasoning,
    Compaction,
    Tool,
    Status,
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub id: String,
    pub kind: EntryKind,
    pub text: String,
    pub name: String,
    pub input: String,
    pub output: String,
    pub complete: bool,
    pub optimistic: bool,
    pub status: String,
    pub started_at: Option<u64>,
    pub background: bool,
    pub metadata: Value,
    pub error: String,
    pub media: Vec<super::presentation::Media>,
    history: String,
    stream: String,
    history_output: String,
    stream_output: String,
    reasoning: BTreeMap<(bool, u64), String>,
}

impl Entry {
    fn new(id: String, kind: EntryKind, text: String, complete: bool) -> Self {
        let media = super::presentation::markdown_media(&text);
        Self {
            media,
            id,
            kind,
            text,
            complete,
            name: String::new(),
            input: String::new(),
            output: String::new(),
            optimistic: false,
            status: if complete { "completed" } else { "running" }.into(),
            started_at: None,
            background: false,
            metadata: Value::Null,
            error: String::new(),
            history: String::new(),
            stream: String::new(),
            history_output: String::new(),
            stream_output: String::new(),
            reasoning: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Default)]
pub struct Conversation {
    pub activity: u64,
    pub entries: Vec<Entry>,
    pub working: bool,
    pub retrying: bool,
    pub used_tokens: Option<u64>,
    pub context_window: Option<u64>,
    pub usage_model: Option<String>,
    usage_at: u64,
    step: Option<opencode::Step>,
    turn: Option<turn::Turn>,
    output_history: timing::History,
    pub failure: failure::State,
}

impl Conversation {
    pub fn optimistic(&mut self, id: String, prompt: String) {
        let mut entry = Entry::new(id, EntryKind::User, prompt, true);
        entry.started_at = Some(super::presentation::now());
        entry.optimistic = true;
        self.entries.push(entry);
        self.working = true;
    }

    pub fn remove_optimistic(&mut self) {
        self.entries.retain(|entry| !entry.optimistic);
    }

    pub fn history(&mut self, provider: Provider, data: &[Value], older: bool) {
        if provider == Provider::OpenCode && (!older || self.usage_at == 0) {
            for message in data.iter().rev() {
                let at = message["time"]["created"].as_u64().unwrap_or(0);
                if at < self.usage_at {
                    continue;
                }
                if message["type"] == "compaction" && message["status"] == "completed" {
                    self.used_tokens = None;
                    self.usage_at = at;
                    break;
                }
                if message["type"] == "assistant" && self.usage(&message["tokens"], at) {
                    self.usage_model = model_id(message);
                    break;
                }
            }
        }
        let entries = match provider {
            Provider::Codex => data
                .iter()
                .flat_map(|turn| {
                    let items = array(&turn["items"]);
                    let final_assistant = items
                        .iter()
                        .rposition(|item| item["type"] == "agentMessage");
                    let turn_started = turn["createdAt"].as_u64().map(timestamp_ms);
                    let turn_ended = turn["completedAt"]
                        .as_u64()
                        .or(turn["endedAt"].as_u64())
                        .map(timestamp_ms);
                    items
                        .iter()
                        .enumerate()
                        .filter_map(move |(position, item)| {
                            codex_item(
                                item,
                                turn["status"].as_str().or(turn["status"]["type"].as_str())
                                    != Some("inProgress"),
                            )
                            .map(|mut entry| {
                                entry.started_at = entry.started_at.or(turn_started);
                                if entry.kind == EntryKind::Assistant {
                                    if let Some(agent) = agent_name(&item["agent"])
                                        .or_else(|| agent_name(&item["metadata"]["agent"]))
                                        .or_else(|| agent_name(&turn["agent"]))
                                    {
                                        set_metadata(&mut entry, "agent", agent);
                                    }
                                    if Some(position) == final_assistant {
                                        if let (Some(start), Some(end)) = (turn_started, turn_ended)
                                        {
                                            set_metadata(&mut entry, "ended", Value::from(end));
                                            set_metadata(
                                                &mut entry,
                                                "outputDurationMs",
                                                Value::from(end.saturating_sub(start)),
                                            );
                                        }
                                    }
                                }
                                entry
                            })
                        })
                })
                .collect(),
            Provider::OpenCode => data
                .iter()
                .flat_map(|message| {
                    let at = message["time"]["created"].as_u64();
                    opencode_message(message).into_iter().map(move |mut entry| {
                        if !entry.background
                            || (entry.name == "subagent" && entry.metadata["source"] != "subagent")
                        {
                            entry.started_at = entry.started_at.or(at);
                        }
                        entry
                    })
                })
                .collect(),
        };
        reconciliation::merge(self, entries, older);
        if provider == Provider::OpenCode {
            let timings = self.output_history.update(data, older);
            for entry in &mut self.entries {
                if let Some(&(start, end)) = timings.get(&entry.id) {
                    set_output_duration(entry, start, end);
                }
            }
        }
    }

    fn put(&mut self, entry: Entry) {
        if entry.kind == EntryKind::Assistant {
            self.note_turn_output(&entry.id);
        }
        if entry.kind != EntryKind::User
            && !self.entries.iter().any(|old| {
                old.id == entry.id
                    && old.text == entry.text
                    && old.input == entry.input
                    && old.output == entry.output
                    && old.complete == entry.complete
                    && old.status == entry.status
            })
        {
            self.activity += 1;
        }
        reconciliation::merge(self, vec![entry], false);
    }

    fn delta(&mut self, id: String, kind: EntryKind, delta: &str, output: bool) {
        if delta.is_empty() {
            return;
        }
        let index = self
            .entries
            .iter()
            .position(|entry| entry.id == id)
            .unwrap_or_else(|| {
                self.entries
                    .push(Entry::new(id, kind, String::new(), false));
                self.entries.len() - 1
            });
        if kind == EntryKind::Assistant {
            let id = self.entries[index].id.clone();
            self.note_turn_output(&id);
        }
        let entry = &mut self.entries[index];
        if entry.complete {
            return;
        }
        self.activity += 1;
        if output {
            entry.stream_output.push_str(delta);
            entry.output = reconciliation::reconcile(&entry.history_output, &entry.stream_output);
        } else {
            entry.stream.push_str(delta);
            entry.text = reconciliation::reconcile(&entry.history, &entry.stream);
        }
        self.working = true;
    }

    pub fn settle(&mut self) {
        self.settle_at(super::presentation::now());
    }

    fn settle_at(&mut self, ended_at: u64) {
        self.finish_turn(ended_at);
        self.step = None;
        self.working = false;
        self.retrying = false;
        for entry in &mut self.entries {
            if !entry.background {
                entry.complete = true;
                if entry.status == "running"
                    || (entry.kind == EntryKind::Compaction
                        && matches!(entry.status.as_str(), "pending" | "in_progress"))
                {
                    entry.status = if entry.kind == EntryKind::Compaction {
                        "interrupted"
                    } else {
                        "completed"
                    }
                    .into();
                }
            }
        }
    }

    pub fn apply(&mut self, provider: Provider, method: &str, data: &Value) {
        if method == "session.retry.scheduled"
            || (method == "session.status" && data["status"]["type"] == "retry")
        {
            self.retrying = true;
        } else if method.ends_with("delta")
            || method.ends_with("Delta")
            || method.ends_with(".started")
            || method == "item/started"
            || method == "turn/started"
            || (method == "session.status" && data["status"]["type"] == "busy")
        {
            self.retrying = false;
        }
        match provider {
            Provider::Codex => self.codex(method, data),
            Provider::OpenCode => self.opencode(method, data),
        }
    }

    fn codex(&mut self, method: &str, data: &Value) {
        match method {
            "thread/tokenUsage/updated" => {
                if let Some(used) = data["tokenUsage"]["last"]["totalTokens"].as_u64() {
                    self.used_tokens = Some(used);
                    self.context_window = data["tokenUsage"]["modelContextWindow"]
                        .as_u64()
                        .filter(|n| *n > 0);
                }
            }
            "item/started" | "item/completed" => {
                if let Some(mut entry) = codex_item(&data["item"], method == "item/completed") {
                    let at = data["_created"]
                        .as_u64()
                        .unwrap_or_else(super::presentation::now);
                    if !entry.complete {
                        entry.started_at = Some(at);
                    } else {
                        if !entry.metadata.is_object() {
                            entry.metadata = serde_json::json!({});
                        }
                        entry.metadata["ended"] = at.into();
                    }
                    let already_complete =
                        self.entries.iter().any(|e| e.id == entry.id && e.complete);
                    if !already_complete || entry.complete {
                        if !entry.complete {
                            self.working = true;
                        }
                        self.put(entry);
                    }
                }
            }
            "item/agentMessage/delta" | "item/commandExecution/outputDelta" => {
                if let Some(id) = data["itemId"].as_str() {
                    let output = method.contains("outputDelta");
                    self.delta(
                        format!("codex-{id}"),
                        if output {
                            EntryKind::Tool
                        } else {
                            EntryKind::Assistant
                        },
                        string(&data["delta"]),
                        output,
                    );
                }
            }
            "item/reasoning/summaryTextDelta" | "item/reasoning/textDelta" => {
                if let Some(id) = data["itemId"].as_str() {
                    let id = format!("codex-{id}");
                    if !self.entries.iter().any(|entry| entry.id == id) {
                        self.entries.push(Entry::new(
                            id.clone(),
                            EntryKind::Reasoning,
                            String::new(),
                            false,
                        ));
                    }
                    let entry = self
                        .entries
                        .iter_mut()
                        .find(|entry| entry.id == id)
                        .expect("inserted entry");
                    if entry.complete {
                        return;
                    }
                    let summary = method.contains("summaryText");
                    let index = data[if summary {
                        "summaryIndex"
                    } else {
                        "contentIndex"
                    }]
                    .as_u64()
                    .unwrap_or(0);
                    entry
                        .reasoning
                        .entry((summary, index))
                        .or_default()
                        .push_str(string(&data["delta"]));
                    let has_summary = entry.reasoning.keys().any(|(summary, _)| *summary);
                    entry.stream = entry
                        .reasoning
                        .iter()
                        .filter(|((summary, _), _)| *summary == has_summary)
                        .map(|(_, text)| text.as_str())
                        .collect::<Vec<_>>()
                        .join("\n\n");
                    entry.text = reconciliation::reconcile(&entry.history, &entry.stream);
                    self.working = true;
                }
            }
            "turn/started" => {
                self.provider_started(data);
                self.working = true;
            }
            "thread/status/changed" => self.working = data["status"]["type"] == "active",
            "turn/completed" => {
                self.settle_at(
                    data["_created"]
                        .as_u64()
                        .unwrap_or_else(super::presentation::now),
                );
                if let Some(error) = data["turn"]["error"]["message"].as_str() {
                    self.put(Entry::new(
                        format!("codex-error-{}", string(&data["turn"]["id"])),
                        EntryKind::Status,
                        error.into(),
                        true,
                    ));
                }
            }
            _ => {}
        }
    }
}

fn set_metadata(entry: &mut Entry, key: &str, value: impl Into<Value>) {
    if !entry.metadata.is_object() {
        entry.metadata = serde_json::json!({});
    }
    entry.metadata[key] = value.into();
}

fn set_output_duration(entry: &mut Entry, start: u64, end: u64) {
    let start = entry.metadata["outputStartedAt"]
        .as_u64()
        .map_or(start, |at| at.min(start));
    let end = entry.metadata["outputEndedAt"]
        .as_u64()
        .map_or(end, |at| at.max(end));
    set_metadata(entry, "outputStartedAt", start);
    set_metadata(entry, "outputEndedAt", end);
    set_metadata(entry, "outputDurationMs", end.saturating_sub(start));
}

fn agent_name(value: &Value) -> Option<String> {
    value
        .as_str()
        .or(value["id"].as_str())
        .filter(|agent| !agent.trim().is_empty())
        .map(str::to_owned)
}

fn timestamp_ms(timestamp: u64) -> u64 {
    if timestamp < 10_000_000_000 {
        timestamp.saturating_mul(1000)
    } else {
        timestamp
    }
}

pub(super) fn provider_started_at(data: &Value) -> u64 {
    data["_created"]
        .as_u64()
        .or(data["started"].as_u64())
        .or(data["startedAt"].as_u64())
        .or_else(|| data["turn"]["createdAt"].as_u64().map(timestamp_ms))
        .or_else(|| data["turn"]["startedAt"].as_u64().map(timestamp_ms))
        .unwrap_or_else(super::presentation::now)
}

fn codex_item(item: &Value, complete: bool) -> Option<Entry> {
    let id = format!("codex-{}", item["id"].as_str()?);
    let kind = match string(&item["type"]) {
        "userMessage" => EntryKind::User,
        "agentMessage" => EntryKind::Assistant,
        "reasoning" => EntryKind::Reasoning,
        "contextCompaction" => EntryKind::Compaction,
        _ => EntryKind::Tool,
    };
    let text = match kind {
        EntryKind::User => text_content(&item["content"]),
        EntryKind::Reasoning => {
            let summary = text_content(&item["summary"]);
            if summary.is_empty() {
                text_content(&item["content"])
            } else {
                summary
            }
        }
        EntryKind::Compaction => text_content(&item["summary"]),
        _ => display(&item["text"]),
    };
    let mut entry = Entry::new(id, kind, text, complete || kind == EntryKind::User);
    if kind == EntryKind::Assistant {
        if let Some(agent) =
            agent_name(&item["agent"]).or_else(|| agent_name(&item["metadata"]["agent"]))
        {
            set_metadata(&mut entry, "agent", agent);
        }
    }
    entry
        .media
        .extend(super::presentation::media(&item["content"]));
    entry.status = item["status"]
        .as_str()
        .unwrap_or(if complete { "completed" } else { "running" })
        .into();
    if item["exitCode"].as_i64().is_some_and(|code| code != 0) {
        entry.status = format!("failed (exit {})", item["exitCode"]);
    }
    if kind == EntryKind::Compaction {
        entry.error = display(item["error"].get("message").unwrap_or(&item["error"]));
    }
    if kind == EntryKind::Tool {
        entry.metadata = serde_json::json!({"cwd":item["cwd"],"exitCode":item["exitCode"],"durationMs":item["durationMs"],"signal":item["signal"]});
        entry.error = display(&item["error"]);
        entry.name = item["tool"]
            .as_str()
            .unwrap_or(string(&item["type"]))
            .into();
        entry.input = ["command", "arguments", "changes", "query"]
            .iter()
            .find_map(|key| item.get(key))
            .map(display)
            .unwrap_or_default();
        entry.output = ["aggregatedOutput", "result", "content", "error"]
            .iter()
            .find_map(|key| item.get(key).filter(|v| !v.is_null()))
            .map(display)
            .unwrap_or_default();
    }
    Some(entry)
}

fn opencode_message(message: &Value) -> Vec<Entry> {
    let Some(id) = message["id"].as_str() else {
        return Vec::new();
    };
    match string(&message["type"]) {
        "user" => {
            let mut entry = Entry::new(
                format!("opencode-{id}"),
                EntryKind::User,
                display(&message["text"]),
                true,
            );
            entry
                .media
                .extend(super::presentation::media(&message["files"]));
            vec![entry]
        }
        "assistant" => {
            let mut text_ordinal = 0;
            let mut reasoning_ordinal = 0;
            array(&message["content"])
                .iter()
                .filter_map(|part| {
                    let kind = string(&part["type"]);
                    if matches!(kind, "text" | "reasoning") {
                        let entry_kind = if kind == "text" {
                            EntryKind::Assistant
                        } else {
                            EntryKind::Reasoning
                        };
                        let ordinal = if kind == "text" {
                            &mut text_ordinal
                        } else {
                            &mut reasoning_ordinal
                        };
                        let mut entry = Entry::new(
                            part_id(id, kind, *ordinal),
                            entry_kind,
                            display(&part["text"]),
                            !part["time"]["completed"].is_null()
                                || !message["time"]["completed"].is_null()
                                || !message["finish"].is_null(),
                        );
                        entry.started_at = message["time"]["created"].as_u64();
                        if entry_kind == EntryKind::Assistant {
                            if let Some(agent) = agent_name(&message["agent"]) {
                                set_metadata(&mut entry, "agent", agent);
                            }
                        }
                        if entry_kind == EntryKind::Assistant {
                            if let Some(end) = message["time"]["completed"].as_u64() {
                                set_metadata(&mut entry, "ended", Value::from(end));
                            }
                        }
                        *ordinal += 1;
                        Some(entry)
                    } else if kind == "tool" {
                        let state = &part["state"];
                        let mut entry = Entry::new(
                            format!("opencode-tool-{}", part["id"].as_str()?),
                            EntryKind::Tool,
                            String::new(),
                            matches!(
                                string(&state["status"]),
                                "completed" | "success" | "error" | "failed"
                            ),
                        );
                        entry.name = display(&part["name"]);
                        entry.status = if !state["error"].is_null() {
                            "failed"
                        } else {
                            state["status"].as_str().unwrap_or(if entry.complete {
                                "completed"
                            } else {
                                "running"
                            })
                        }
                        .into();
                        entry.error = display(&state["error"]);
                        entry.metadata = state["metadata"].clone();
                        entry.started_at = state["time"]["start"]
                            .as_u64()
                            .or(part["time"]["created"].as_u64());
                        if let Some(ended) = state["time"]["end"]
                            .as_u64()
                            .or(part["time"]["completed"].as_u64())
                        {
                            if !entry.metadata.is_object() {
                                entry.metadata = serde_json::json!({});
                            }
                            entry.metadata["ended"] = ended.into();
                        }
                        entry.input = display(&state["input"]);
                        entry.media = super::presentation::media(&state["content"]);
                        entry.output = if state["error"].is_null() {
                            text_content(&state["content"])
                        } else {
                            display(&state["error"])
                        };
                        subagents::tool(&mut entry);
                        Some(entry)
                    } else {
                        None
                    }
                })
                .collect()
        }
        "shell" => shells::history(message).into_iter().collect(),
        "synthetic" => vec![shells::synthetic(message)
            .or_else(|| subagents::synthetic(message))
            .unwrap_or_else(|| {
                Entry::new(
                    format!("opencode-{id}"),
                    EntryKind::Status,
                    display(&message["text"]),
                    true,
                )
            })],
        "compaction" => {
            let mut entry = Entry::new(
                format!("opencode-{id}"),
                EntryKind::Compaction,
                text_content(&message["summary"]),
                !matches!(
                    message["status"].as_str(),
                    Some("running" | "pending" | "in_progress")
                ),
            );
            entry.status = message["status"].as_str().unwrap_or("completed").into();
            entry.error = display(message["error"].get("message").unwrap_or(&message["error"]));
            if let Some(ended) = message["time"]["completed"].as_u64() {
                entry.metadata = serde_json::json!({"ended": ended});
            }
            vec![entry]
        }
        _ => Vec::new(),
    }
}

fn part_id(message: &str, kind: &str, ordinal: u64) -> String {
    if kind == "text" && ordinal == 0 {
        format!("opencode-{message}")
    } else {
        format!("opencode-{message}-{kind}-{ordinal}")
    }
}

fn model_id(value: &Value) -> Option<String> {
    let model = value["model"]["id"]
        .as_str()
        .or(value["modelID"].as_str())?;
    let provider = value["model"]["providerID"]
        .as_str()
        .or(value["providerID"].as_str());
    Some(if model.contains('/') {
        model.into()
    } else if let Some(provider) = provider {
        format!("{provider}/{model}")
    } else {
        model.into()
    })
}

fn string(value: &Value) -> &str {
    value.as_str().unwrap_or("")
}
fn array(value: &Value) -> &[Value] {
    value.as_array().map(Vec::as_slice).unwrap_or_default()
}
fn display(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(value) => value.clone(),
        value => serde_json::to_string_pretty(value).unwrap_or_default(),
    }
}
fn text_content(value: &Value) -> String {
    match value.as_array() {
        Some(parts) => parts
            .iter()
            .filter(|part| {
                !matches!(
                    part["type"].as_str(),
                    Some("file" | "image" | "localImage" | "input_image")
                )
            })
            .map(|part| {
                part.get("text")
                    .map(display)
                    .unwrap_or_else(|| display(part))
            })
            .collect::<Vec<_>>()
            .join("\n"),
        None => display(value),
    }
}

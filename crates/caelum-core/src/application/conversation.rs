use std::collections::{BTreeMap, HashSet};

use serde_json::Value;

use super::Provider;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    User,
    Assistant,
    Reasoning,
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
    history: String,
    stream: String,
    history_output: String,
    stream_output: String,
    reasoning: BTreeMap<(bool, u64), String>,
}

impl Entry {
    fn new(id: String, kind: EntryKind, text: String, complete: bool) -> Self {
        Self {
            id,
            kind,
            text,
            complete,
            name: String::new(),
            input: String::new(),
            output: String::new(),
            optimistic: false,
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
    pub entries: Vec<Entry>,
    pub working: bool,
}

impl Conversation {
    pub fn optimistic(&mut self, id: String, prompt: String) {
        let mut entry = Entry::new(id, EntryKind::User, prompt, true);
        entry.optimistic = true;
        self.entries.push(entry);
        self.working = true;
    }

    pub fn remove_optimistic(&mut self) {
        self.entries.retain(|entry| !entry.optimistic);
    }

    pub fn history(&mut self, provider: Provider, data: &[Value], older: bool) {
        let entries = match provider {
            Provider::Codex => data
                .iter()
                .flat_map(|turn| {
                    array(&turn["items"]).iter().filter_map(|item| {
                        codex_item(
                            item,
                            turn["status"].as_str().or(turn["status"]["type"].as_str())
                                != Some("inProgress"),
                        )
                    })
                })
                .collect(),
            Provider::OpenCode => data.iter().flat_map(opencode_message).collect(),
        };
        self.merge(entries, older);
    }

    fn merge(&mut self, history: Vec<Entry>, older: bool) {
        let mut live = std::mem::take(&mut self.entries);
        let ids: HashSet<_> = history.iter().map(|entry| entry.id.as_str()).collect();
        // A repeated prompt may occur in old history. Only its newest occurrence
        // in a refresh can adopt this client's pending optimistic submission.
        let optimistic = if older {
            None
        } else {
            live.iter()
                .position(|entry| entry.optimistic)
                .and_then(|index| {
                    history
                        .iter()
                        .rposition(|entry| {
                            entry.kind == EntryKind::User && entry.text == live[index].text
                        })
                        .map(|history_index| (index, history_index))
                })
        };
        let insertion = live
            .iter()
            .position(|entry| ids.contains(entry.id.as_str()))
            .or_else(|| optimistic.map(|(index, _)| index))
            .unwrap_or(if older { 0 } else { live.len() });
        let mut before = Vec::new();
        let mut after = Vec::new();
        let mut existing = BTreeMap::new();
        for (index, entry) in live.drain(..).enumerate() {
            if ids.contains(entry.id.as_str()) {
                existing.insert(entry.id.clone(), entry);
            } else if optimistic.map_or(true, |(adopted, _)| adopted != index) {
                if index < insertion {
                    before.push(entry);
                } else {
                    after.push(entry);
                }
            }
        }
        for snapshot in history {
            let entry = match existing.remove(&snapshot.id) {
                Some(current) => merge_entry(snapshot, current),
                None => snapshot_entry(snapshot),
            };
            before.push(entry);
        }
        before.extend(after);
        self.entries = before;
    }

    fn put(&mut self, entry: Entry) {
        self.merge(vec![entry], false);
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
        let entry = &mut self.entries[index];
        if entry.complete {
            return;
        }
        if output {
            entry.stream_output.push_str(delta);
            entry.output = reconcile(&entry.history_output, &entry.stream_output);
        } else {
            entry.stream.push_str(delta);
            entry.text = reconcile(&entry.history, &entry.stream);
        }
        self.working = true;
    }

    pub fn settle(&mut self) {
        self.working = false;
        for entry in &mut self.entries {
            entry.complete = true;
        }
    }

    pub fn apply(&mut self, provider: Provider, method: &str, data: &Value) {
        match provider {
            Provider::Codex => self.codex(method, data),
            Provider::OpenCode => self.opencode(method, data),
        }
    }

    fn codex(&mut self, method: &str, data: &Value) {
        match method {
            "item/started" | "item/completed" => {
                if let Some(entry) = codex_item(&data["item"], method == "item/completed") {
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
                    entry.text = reconcile(&entry.history, &entry.stream);
                    self.working = true;
                }
            }
            "turn/started" => self.working = true,
            "thread/status/changed" => self.working = data["status"]["type"] == "active",
            "turn/completed" => {
                self.settle();
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

    fn opencode(&mut self, method: &str, data: &Value) {
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
                            self.put(Entry::new(id, kind, text, true));
                        }
                        "started" if !self.entries.iter().any(|entry| entry.id == id) => {
                            self.put(Entry::new(id, kind, String::new(), false));
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
                } else if !data["output"].is_null() {
                    entry.output = display(&data["output"]);
                } else if let Some(output) = data["metadata"]["output"].as_str() {
                    entry.output = output.into();
                }
                if !data["error"].is_null() {
                    entry.output = display(&data["error"]);
                }
                if !complete {
                    self.working = true;
                }
                // This is a new live tool snapshot, not a historical prefix.
                if let Some(index) = self.entries.iter().position(|e| e.id == entry.id) {
                    self.entries[index] = entry;
                } else {
                    self.entries.push(entry);
                }
            }
        }
        if data["message"]["type"].is_string() {
            self.merge(opencode_message(&data["message"]), false);
        }
        match method {
            "session.execution.started" | "session.step.started" | "session.retry.scheduled" => {
                self.working = true
            }
            "session.status" => match string(&data["status"]["type"]) {
                "busy" | "retry" => self.working = true,
                "idle" => self.settle(),
                _ => {}
            },
            "session.execution.succeeded"
            | "session.execution.failed"
            | "session.execution.interrupted"
            | "session.idle" => {
                self.settle();
                if method.ends_with("failed") {
                    self.put(Entry::new(
                        format!("opencode-error-{}", self.entries.len()),
                        EntryKind::Status,
                        display(&data["error"]),
                        true,
                    ));
                }
            }
            _ => {}
        }
    }
}

fn snapshot_entry(mut entry: Entry) -> Entry {
    entry.history = entry.text.clone();
    entry.history_output = entry.output.clone();
    entry
}

fn merge_entry(mut snapshot: Entry, mut current: Entry) -> Entry {
    if snapshot.complete {
        if snapshot.kind == EntryKind::Tool && snapshot.output.is_empty() {
            snapshot.output = current.output;
        }
        return snapshot_entry(snapshot);
    }
    if current.complete {
        return current;
    }
    current.history = snapshot.text;
    current.text = reconcile(&current.history, &current.stream);
    // OpenCode publishes whole output snapshots, while Codex also publishes
    // output deltas. Retain either kind when a slower history page arrives.
    let live_output = if current.stream_output.is_empty() {
        &current.output
    } else {
        &current.stream_output
    };
    current.history_output = snapshot.output;
    current.output = reconcile(&current.history_output, live_output);
    if current.name.is_empty() {
        current.name = snapshot.name;
    }
    if current.input.is_empty() {
        current.input = snapshot.input;
    }
    current
}

fn codex_item(item: &Value, complete: bool) -> Option<Entry> {
    let id = format!("codex-{}", item["id"].as_str()?);
    let kind = match string(&item["type"]) {
        "userMessage" => EntryKind::User,
        "agentMessage" => EntryKind::Assistant,
        "reasoning" => EntryKind::Reasoning,
        "contextCompaction" => EntryKind::Status,
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
        EntryKind::Status => "Compacting context…".into(),
        _ => display(&item["text"]),
    };
    let mut entry = Entry::new(id, kind, text, complete || kind == EntryKind::User);
    if kind == EntryKind::Tool {
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
        "user" => vec![Entry::new(
            format!("opencode-{id}"),
            EntryKind::User,
            display(&message["text"]),
            true,
        )],
        "assistant" => {
            let mut text_ordinal = 0;
            let mut reasoning_ordinal = 0;
            array(&message["content"])
                .iter()
                .filter_map(|part| {
                    let kind = string(&part["type"]);
                    if matches!(kind, "text" | "reasoning") {
                        let ordinal = if kind == "text" {
                            &mut text_ordinal
                        } else {
                            &mut reasoning_ordinal
                        };
                        let entry = Entry::new(
                            part_id(id, kind, *ordinal),
                            if kind == "text" {
                                EntryKind::Assistant
                            } else {
                                EntryKind::Reasoning
                            },
                            display(&part["text"]),
                            !part["time"]["completed"].is_null()
                                || !message["time"]["completed"].is_null()
                                || !message["finish"].is_null(),
                        );
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
                        entry.input = display(&state["input"]);
                        entry.output = if state["error"].is_null() {
                            text_content(&state["content"])
                        } else {
                            display(&state["error"])
                        };
                        Some(entry)
                    } else {
                        None
                    }
                })
                .collect()
        }
        "system" | "synthetic" => vec![Entry::new(
            format!("opencode-{id}"),
            EntryKind::Status,
            display(&message["text"]),
            true,
        )],
        "compaction" => vec![Entry::new(
            format!("opencode-{id}"),
            EntryKind::Status,
            format!("Context compaction\n{}", display(&message["summary"])),
            message["status"] != "running",
        )],
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

/// Linear-time snapshot/live overlap, including a stream that lags the snapshot.
fn reconcile(snapshot: &str, stream: &str) -> String {
    if stream.is_empty() || snapshot.contains(stream) {
        return snapshot.into();
    }
    if snapshot.is_empty() || stream.contains(snapshot) {
        return stream.into();
    }
    let pattern: Vec<_> = stream.chars().collect();
    let mut failure = vec![0; pattern.len()];
    let mut matched = 0;
    for index in 1..pattern.len() {
        while matched > 0 && pattern[index] != pattern[matched] {
            matched = failure[matched - 1];
        }
        if pattern[index] == pattern[matched] {
            matched += 1;
        }
        failure[index] = matched;
    }
    matched = 0;
    for character in snapshot.chars() {
        while matched > 0 && (matched == pattern.len() || character != pattern[matched]) {
            matched = failure[matched - 1];
        }
        if character == pattern[matched] {
            matched += 1;
        }
    }
    let mut result = snapshot.to_owned();
    result.extend(pattern[matched..].iter());
    result
}

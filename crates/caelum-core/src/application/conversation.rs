use std::collections::{BTreeMap, HashSet};

use serde_json::Value;

use super::Provider;

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
    step_started: Option<(String, u64)>,
    step_model: Option<String>,
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
                    array(&turn["items"]).iter().filter_map(|item| {
                        codex_item(
                            item,
                            turn["status"].as_str().or(turn["status"]["type"].as_str())
                                != Some("inProgress"),
                        )
                        .map(|mut entry| {
                            entry.started_at = entry.started_at.or(turn["createdAt"]
                                .as_u64()
                                .map(|n| if n < 10_000_000_000 { n * 1000 } else { n }));
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
                        entry.started_at = entry.started_at.or(at);
                        entry
                    })
                })
                .collect(),
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
            } else if optimistic.is_none_or(|(adopted, _)| adopted != index) {
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
        self.activity += 1;
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
        self.retrying = false;
        for entry in &mut self.entries {
            if !entry.background {
                entry.complete = true;
                if entry.status == "running" {
                    entry.status = "completed".into();
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
        if method.starts_with("session.compaction.") {
            let at = data["_created"].as_u64().unwrap_or(0);
            let latest = self
                .entries
                .iter()
                .rposition(|e| e.kind == EntryKind::Compaction);
            if latest.is_some_and(|i| at > 0 && at < self.entries[i].started_at.unwrap_or(0)) {
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
            } else if let Some(i) = latest.filter(|i| !self.entries[*i].complete) {
                if method.ends_with("delta") {
                    let id = self.entries[i].id.clone();
                    self.delta(id, EntryKind::Compaction, string(&data["text"]), false);
                } else if method.ends_with("ended") || method.ends_with("failed") {
                    let entry = &mut self.entries[i];
                    entry.complete = true;
                    entry.status = if method.ends_with("failed") {
                        "failed"
                    } else {
                        "completed"
                    }
                    .into();
                    if let Some(text) = data["text"].as_str() {
                        entry.text = text.into();
                    }
                    if !data["error"].is_null() {
                        entry.output = display(&data["error"]);
                    }
                }
            }
            if method.ends_with("ended") && at >= self.usage_at {
                self.used_tokens = None;
                self.usage_at = at;
            }
            return;
        }
        if matches!(method, "session.shell.started" | "session.shell.ended") {
            if let Some(entry) = shell_entry(&data["shell"], &data["output"]) {
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
            if let Some(id) = data["_event_id"].as_str() {
                self.put(Entry::new(
                    format!("opencode-{}", id.replacen("evt_", "msg_", 1)),
                    EntryKind::Status,
                    display(&data["text"]),
                    true,
                ));
            }
        }
        if method == "session.step.started" {
            self.step_model = model_id(data);
            self.step_started = data["assistantMessageID"].as_str().map(|id| {
                (
                    id.into(),
                    data["started"]
                        .as_u64()
                        .or(data["_created"].as_u64())
                        .unwrap_or(0),
                )
            });
        }
        if method == "session.step.ended" {
            let at = self
                .step_started
                .as_ref()
                .filter(|(id, _)| Some(id.as_str()) == data["assistantMessageID"].as_str())
                .map(|(_, at)| *at)
                .or(data["_created"].as_u64())
                .unwrap_or(0);
            if self.usage(&data["tokens"], at) {
                self.usage_model = model_id(data).or_else(|| self.step_model.clone());
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
                            entry.metadata = serde_json::json!({"ended":data["_created"].as_u64().unwrap_or_else(super::presentation::now)});
                            self.put(entry);
                        }
                        "started" if !self.entries.iter().any(|entry| entry.id == id) => {
                            let mut entry = Entry::new(id, kind, String::new(), false);
                            entry.started_at = Some(
                                data["_created"]
                                    .as_u64()
                                    .unwrap_or_else(super::presentation::now),
                            );
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
                    entry.media = super::presentation::media(&data["content"]);
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
                    .or_else(|| Some(super::presentation::now()));
                if let Some(ended) = data["_created"].as_u64().filter(|_| complete) {
                    if !entry.metadata.is_object() {
                        entry.metadata = serde_json::json!({});
                    }
                    entry.metadata["ended"] = ended.into();
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

    fn usage(&mut self, tokens: &Value, at: u64) -> bool {
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

fn snapshot_entry(mut entry: Entry) -> Entry {
    entry.history = entry.text.clone();
    entry.history_output = entry.output.clone();
    entry
}

fn merge_entry(mut snapshot: Entry, mut current: Entry) -> Entry {
    snapshot.started_at = snapshot.started_at.or(current.started_at);
    if snapshot.metadata.is_null() {
        snapshot.metadata = current.metadata.clone();
    }
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
        EntryKind::Compaction => String::new(),
        _ => display(&item["text"]),
    };
    let mut entry = Entry::new(id, kind, text, complete || kind == EntryKind::User);
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
        "user" => { let mut entry = Entry::new(
            format!("opencode-{id}"),
            EntryKind::User,
            display(&message["text"]),
            true,
        ); entry.media.extend(super::presentation::media(&message["files"])); vec![entry] },
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
                        entry.status = if !state["error"].is_null() { "failed" } else { state["status"].as_str().unwrap_or(if entry.complete { "completed" } else { "running" }) }.into();
                        entry.error = display(&state["error"]);
                        entry.metadata = state["metadata"].clone();
                        entry.started_at = state["time"]["start"].as_u64().or(part["time"]["created"].as_u64());
                        if let Some(ended) = state["time"]["end"].as_u64().or(part["time"]["completed"].as_u64()) {
                            if !entry.metadata.is_object() { entry.metadata = serde_json::json!({}); }
                            entry.metadata["ended"] = ended.into();
                        }
                        entry.input = display(&state["input"]);
                        entry.media = super::presentation::media(&state["content"]);
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
        "shell" => shell_entry(&serde_json::json!({"id": message["shellID"].as_str().or(message["id"].as_str()), "command":message["command"], "status":message["status"], "exit":message["exit"], "metadata":message["metadata"]}), &message["output"]).into_iter().collect(),
        "synthetic" => vec![Entry::new(
            format!("opencode-{id}"),
            EntryKind::Status,
            display(&message["text"]),
            true,
        )],
        "compaction" => vec![Entry::new(
            format!("opencode-{id}"),
            EntryKind::Compaction,
            display(&message["summary"]),
            message["status"] != "running",
        )],
        _ => Vec::new(),
    }
}

fn shell_entry(shell: &Value, output: &Value) -> Option<Entry> {
    let id = shell["id"].as_str()?;
    let complete = shell["status"] != "running";
    let mut entry = Entry::new(
        format!("opencode-shell-{id}"),
        EntryKind::Tool,
        String::new(),
        complete,
    );
    entry.name = "Background shell".into();
    entry.background = true;
    entry.started_at = shell["time"]["started"]
        .as_u64()
        .or(shell["started"].as_u64())
        .or(shell["startedAt"].as_u64());
    entry.metadata = serde_json::json!({"exit":shell["exit"],"signal":shell["signal"],"ended":shell["time"]["completed"].as_u64().or(shell["ended"].as_u64()),"cwd":shell["cwd"]});
    entry.input = display(&shell["command"]);
    entry.output = if output.is_object() {
        display(&output["output"])
    } else {
        display(output)
    };
    entry.status = if shell["exit"].as_i64().is_some_and(|code| code != 0) {
        format!("failed (exit {})", shell["exit"])
    } else {
        shell["status"].as_str().unwrap_or("completed").into()
    };
    Some(entry)
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

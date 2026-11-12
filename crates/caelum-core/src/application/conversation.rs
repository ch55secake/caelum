use std::collections::BTreeMap;

use serde_json::Value;

use super::Provider;

mod reconciliation;
mod timing;
#[cfg(test)]
mod timing_tests;

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
    turn_started_at: Option<u64>,
    turn_agent: Option<String>,
    turn_agent_captured: bool,
    turn_assistant_ids: Vec<String>,
    turn_provider_started: bool,
    output_history: timing::History,
}

impl Conversation {
    pub fn optimistic(&mut self, id: String, prompt: String) {
        let mut entry = Entry::new(id, EntryKind::User, prompt, true);
        entry.started_at = Some(super::presentation::now());
        entry.optimistic = true;
        self.entries.push(entry);
        self.working = true;
    }

    pub fn begin_turn(&mut self, agent: Option<String>, started_at: u64) {
        if !self.working || self.turn_started_at.is_none() {
            self.turn_started_at = Some(started_at);
            self.turn_assistant_ids.clear();
            self.turn_agent = agent;
            self.turn_agent_captured = true;
            self.turn_provider_started = false;
        } else if !self.turn_agent_captured {
            self.turn_agent = agent;
            self.turn_agent_captured = true;
        }
    }

    pub fn provider_started_with_agent(&mut self, agent: Option<String>, started_at: u64) {
        if self.turn_provider_started {
            return;
        }
        if !self.working || self.turn_started_at.is_none() {
            self.turn_started_at = Some(started_at);
            self.turn_assistant_ids.clear();
            self.turn_agent = agent.clone();
            self.turn_agent_captured = true;
        } else if !self.turn_agent_captured {
            self.turn_agent = agent.clone();
            self.turn_agent_captured = true;
        }
        // Admission and session startup belong to the original prompt too. A
        // later provider start must not discard time already spent waiting.
        self.turn_started_at = self.turn_started_at.map(|at| at.min(started_at));
        self.turn_provider_started = true;
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
                        entry.started_at = entry.started_at.or(at);
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
        let started_at = self.turn_started_at.take();
        let assistant_id = self.turn_assistant_ids.last().cloned();
        let turn_agent = self.turn_agent.clone();
        if let (Some(started_at), Some(id)) = (started_at, assistant_id) {
            if let Some(entry) = self.entries.iter_mut().find(|entry| entry.id == id) {
                if !entry.metadata.is_object() {
                    entry.metadata = serde_json::json!({});
                }
                if entry.metadata["agent"].as_str().is_none() {
                    if let Some(agent) = turn_agent.as_deref() {
                        entry.metadata["agent"] = agent.into();
                    }
                }
                entry.metadata["ended"] = ended_at.into();
                set_output_duration(entry, started_at, ended_at);
            }
        }
        self.turn_started_at = None;
        self.turn_agent = None;
        self.turn_agent_captured = false;
        self.turn_assistant_ids.clear();
        self.turn_provider_started = false;
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

    fn note_turn_output(&mut self, id: &str) {
        if self.turn_started_at.is_some()
            && !self
                .turn_assistant_ids
                .iter()
                .any(|existing| existing == id)
        {
            self.turn_assistant_ids.push(id.into());
        }
    }

    fn provider_started(&mut self, data: &Value) {
        self.provider_started_with_agent(self.turn_agent.clone(), provider_started_at(data));
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

    fn opencode(&mut self, method: &str, data: &Value) {
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
                            if kind == EntryKind::Assistant {
                                if let Some(agent) = agent_name(&data["agent"]) {
                                    set_metadata(&mut entry, "agent", agent);
                                }
                            }
                            self.put(entry);
                        }
                        "started" if !self.entries.iter().any(|entry| entry.id == id) => {
                            let mut entry = Entry::new(id, kind, String::new(), false);
                            entry.started_at = Some(
                                data["_created"]
                                    .as_u64()
                                    .unwrap_or_else(super::presentation::now),
                            );
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
                "idle" => self.settle_at(
                    data["_created"]
                        .as_u64()
                        .unwrap_or_else(super::presentation::now),
                ),
                _ => {}
            },
            "session.execution.succeeded"
            | "session.execution.failed"
            | "session.execution.interrupted"
            | "session.idle" => {
                self.settle_at(
                    data["_created"]
                        .as_u64()
                        .unwrap_or_else(super::presentation::now),
                );
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

fn set_metadata(entry: &mut Entry, key: &str, value: impl Into<Value>) {
    if !entry.metadata.is_object() {
        entry.metadata = serde_json::json!({});
    }
    entry.metadata[key] = value.into();
}

fn set_output_duration(entry: &mut Entry, start: u64, end: u64) {
    // A history refresh may know an earlier prompt or a later execution end,
    // but must not shorten a complete duration captured from live events.
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
        "compaction" => {
            let mut entry = Entry::new(
                format!("opencode-{id}"),
                EntryKind::Compaction,
                text_content(&message["summary"]),
                !matches!(message["status"].as_str(), Some("running" | "pending" | "in_progress")),
            );
            entry.status = message["status"].as_str().unwrap_or("completed").into();
            entry.error = display(message["error"].get("message").unwrap_or(&message["error"]));
            if let Some(ended) = message["time"]["completed"].as_u64() {
                entry.metadata = serde_json::json!({"ended": ended});
            }
            vec![entry]
        },
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

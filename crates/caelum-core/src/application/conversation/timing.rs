//! OpenCode assistant messages are individual steps, not complete prompt turns.
//! Keep the timing boundaries across pages so loading an earlier prompt can fill
//! in the full duration without inventing a shorter, output-only fallback.
use std::collections::{BTreeMap, HashSet};

use serde_json::Value;

#[derive(Debug)]
enum Kind {
    User,
    Assistant(Vec<String>),
    Idle,
}

#[derive(Debug)]
struct Message {
    id: String,
    kind: Kind,
    created: Option<u64>,
    completed: Option<u64>,
}

impl Message {
    fn read(value: &Value) -> Option<Self> {
        let id = value["id"].as_str()?;
        let kind = match value["type"].as_str()? {
            "user" => Kind::User,
            "assistant" => Kind::Assistant(
                super::array(&value["content"])
                    .iter()
                    .filter(|part| part["type"] == "text")
                    .enumerate()
                    .map(|(ordinal, _)| super::part_id(id, "text", ordinal as u64))
                    .collect(),
            ),
            "idle" => Kind::Idle,
            _ => return None,
        };
        Some(Self {
            id: id.into(),
            kind,
            created: value["time"]["created"].as_u64(),
            completed: value["time"]["completed"].as_u64(),
        })
    }
}

#[derive(Debug, Default)]
pub(super) struct History {
    messages: Vec<Message>,
}

impl History {
    pub fn update(&mut self, data: &[Value], older: bool) -> BTreeMap<String, (u64, u64)> {
        let incoming: Vec<_> = data.iter().filter_map(Message::read).collect();
        let ids: HashSet<_> = incoming.iter().map(|message| message.id.as_str()).collect();
        // Match the transcript's ordered page merge, including overlapping and
        // delayed refreshes. IDs and timestamps alone need not sort by turn.
        let insertion = self
            .messages
            .iter()
            .position(|message| ids.contains(message.id.as_str()))
            .unwrap_or(if older { 0 } else { self.messages.len() });
        let mut before = Vec::new();
        let mut after = Vec::new();
        let mut existing = BTreeMap::new();
        for (index, message) in std::mem::take(&mut self.messages).into_iter().enumerate() {
            if ids.contains(message.id.as_str()) {
                existing.insert(message.id.clone(), message);
            } else if index < insertion {
                before.push(message);
            } else {
                after.push(message);
            }
        }
        for mut message in incoming {
            if let Some(previous) = existing.remove(&message.id) {
                message.created = message.created.or(previous.created);
                message.completed = message.completed.max(previous.completed);
                if let (Kind::Assistant(outputs), Kind::Assistant(previous)) =
                    (&mut message.kind, previous.kind)
                {
                    if outputs.len() < previous.len() {
                        *outputs = previous;
                    }
                }
            }
            before.push(message);
        }
        before.extend(after);
        self.messages = before;

        let mut timings = BTreeMap::new();
        let mut start = None;
        let mut last_output = None;
        let mut in_turn = false;
        for message in &self.messages {
            match &message.kind {
                Kind::User => {
                    // Steering input is still part of the original execution.
                    if !in_turn {
                        start = message.created;
                        in_turn = true;
                    }
                }
                Kind::Assistant(outputs) => {
                    // A page can begin midway through a turn. Later steering
                    // must not stand in for its still-unloaded original prompt.
                    in_turn = true;
                    for id in outputs {
                        last_output = Some(id);
                        if let (Some(start), Some(end)) = (start, message.completed) {
                            timings.insert(id.clone(), (start, end));
                        }
                    }
                }
                Kind::Idle => {
                    if let (Some(start), Some(end), Some(id)) =
                        (start, message.created, last_output)
                    {
                        timings.insert(id.clone(), (start, end));
                    }
                    start = None;
                    last_output = None;
                    in_turn = false;
                }
            }
        }
        timings
    }
}

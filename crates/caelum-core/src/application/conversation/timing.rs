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

#[derive(Default)]
struct TurnTiming<'a> {
    started_at: Option<u64>,
    last_output: Option<&'a String>,
}

impl History {
    pub fn update(&mut self, data: &[Value], older: bool) -> BTreeMap<String, (u64, u64)> {
        let incoming: Vec<_> = data.iter().filter_map(Message::read).collect();
        let ids: HashSet<_> = incoming.iter().map(|message| message.id.as_str()).collect();

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
        let mut turn = None;
        for message in &self.messages {
            match &message.kind {
                Kind::User => {
                    turn.get_or_insert(TurnTiming {
                        started_at: message.created,
                        last_output: None,
                    });
                }
                Kind::Assistant(outputs) => {
                    let turn = turn.get_or_insert_with(TurnTiming::default);
                    for id in outputs {
                        turn.last_output = Some(id);
                        if let (Some(start), Some(end)) = (turn.started_at, message.completed) {
                            timings.insert(id.clone(), (start, end));
                        }
                    }
                }
                Kind::Idle => {
                    if let Some(turn) = turn.take() {
                        if let (Some(start), Some(end), Some(id)) =
                            (turn.started_at, message.created, turn.last_output)
                        {
                            timings.insert(id.clone(), (start, end));
                        }
                    }
                }
            }
        }
        timings
    }
}

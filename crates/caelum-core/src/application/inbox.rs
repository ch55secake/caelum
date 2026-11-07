//! Provider-owned queued prompts. Identity survives admission retries and late snapshots.
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InboxPrompt {
    pub id: String,
    pub text: String,
    #[serde(default)]
    pub pending: bool,
    #[serde(default)]
    pub removing: bool,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Default)]
pub struct Inbox {
    pub items: Vec<InboxPrompt>,
    pub revision: u64,
    settled: std::collections::VecDeque<String>,
}

impl Inbox {
    pub fn settle(&mut self, id: &str) -> Option<InboxPrompt> {
        if !self.settled.iter().any(|v| v == id) {
            self.settled.push_back(id.into());
            if self.settled.len() > 256 {
                self.settled.pop_front();
            }
        }
        self.items.iter().position(|v| v.id == id).map(|i| {
            self.revision += 1;
            self.items.remove(i)
        })
    }

    pub fn snapshot(&mut self, revision: u64, values: &[Value]) {
        if revision != self.revision {
            return;
        }
        let mut items: Vec<_> = values
            .iter()
            .filter_map(parse)
            .filter(|v| !self.settled.contains(&v.id))
            .collect();
        for item in &self.items {
            if item.removing {
                if let Some(remote) = items.iter_mut().find(|v| v.id == item.id) {
                    remote.removing = true;
                    remote.pending = item.pending;
                    remote.error.clone_from(&item.error);
                }
            }
            if (item.pending || item.error.is_some()) && !items.iter().any(|v| v.id == item.id) {
                items.push(item.clone());
            }
        }
        self.items = items;
    }

    pub fn event(&mut self, method: &str, data: &Value) -> Option<InboxPrompt> {
        let id = data["inboxID"].as_str()?;
        match method {
            "session.inbox.enqueued" => {
                self.revision += 1;
                if !self.settled.iter().any(|v| v == id) {
                    let mut item = data["item"].clone();
                    item["id"] = id.into();
                    if let Some(item) = parse(&item) {
                        if let Some(previous) = self.items.iter_mut().find(|v| v.id == id) {
                            if previous.removing {
                                previous.text = item.text;
                            } else {
                                *previous = item;
                            }
                        } else {
                            self.items.push(item);
                        }
                    }
                }
            }
            "session.inbox.delivered" => return self.settle(id),
            "session.inbox.cancelled" => {
                self.settle(id);
            }
            _ => {}
        }
        None
    }
}

fn parse(value: &Value) -> Option<InboxPrompt> {
    if value["type"] != "user" {
        return None;
    }
    let text = value["payload"]["text"].as_str()?;
    let attachments = super::presentation::media(&value["payload"]["files"])
        .into_iter()
        .filter_map(|m| {
            if !m.source.starts_with("data:image/") {
                return None;
            }
            let mut a = super::Attachment::new(m.name, String::new(), None, false);
            a.content = m.source;
            a.kind = super::AttachmentKind::Image;
            Some(a)
        })
        .collect::<Vec<_>>();
    Some(InboxPrompt {
        id: value["id"].as_str()?.into(),
        text: super::composer::with_context(text, &attachments),
        pending: false,
        removing: false,
        error: None,
    })
}

//! Session-owned answer drafts and the question mode of the composer dock.
mod view;

pub use view::view;

use std::{cell::Cell, collections::HashMap, rc::Rc};

use crate::{
    Workspace,
    live::{Live, live, live_mut},
    panels::Screen,
};
use caelum_core::application::{Event, forms};
use caelum_protocol::QuestionPrompt;
use rune::{Key as InputKey, KeyEvent, ScrollPos, TextEdit, Tx};
use serde_json::{Map, Value};

type Key = (String, String, String, String);
fn key(p: &QuestionPrompt) -> Key {
    (
        p.provider.clone(),
        p.project_path.clone(),
        p.session_id.clone(),
        p.request_id.clone(),
    )
}

fn matches_key(p: &QuestionPrompt, key: &Key) -> bool {
    p.provider == key.0 && p.project_path == key.1 && p.session_id == key.2 && p.request_id == key.3
}

#[derive(Default)]
pub struct Forms {
    drafts: HashMap<Key, Draft>,
    order: Vec<Key>,
    shown: Option<(usize, Key)>,
}

#[derive(Default)]
enum Status {
    #[default]
    Editing,
    Invalid,
    Submitting {
        request: u64,
        dismiss: bool,
    },
    Failed(String),
}

struct Draft {
    values: Map<String, Value>,
    editors: HashMap<String, TextEdit>,
    status: Status,
    position: Rc<Cell<ScrollPos>>,
}

impl Draft {
    fn submitting(&self) -> bool {
        matches!(self.status, Status::Submitting { .. })
    }
}

impl Forms {
    pub fn sync(&mut self, prompts: &[QuestionPrompt]) {
        self.drafts
            .retain(|k, _| prompts.iter().any(|p| matches_key(p, k)));
        self.order.retain(|k| self.drafts.contains_key(k));
        for prompt in prompts {
            let owner = key(prompt);
            if self.drafts.contains_key(&owner) {
                continue;
            }
            self.order.push(owner.clone());
            let mut values = forms::defaults(prompt);
            let editors = prompt
                .fields
                .iter()
                .filter_map(|field| {
                    let name = field["key"].as_str()?;
                    let value = values.get(name).cloned();
                    let text = if field["options"].as_array().is_some_and(|v| !v.is_empty()) {
                        if field["custom"] == true {
                            let listed = |v: &Value| {
                                field["options"]
                                    .as_array()
                                    .unwrap()
                                    .iter()
                                    .any(|o| o["value"] == *v)
                            };
                            if let Some(Value::Array(items)) = &value {
                                values.insert(
                                    name.into(),
                                    Value::Array(
                                        items.iter().filter(|v| listed(v)).cloned().collect(),
                                    ),
                                );
                                items
                                    .iter()
                                    .filter(|v| !listed(v))
                                    .filter_map(Value::as_str)
                                    .collect::<Vec<_>>()
                                    .join("\n")
                            } else {
                                value
                                    .as_ref()
                                    .filter(|v| !listed(v))
                                    .and_then(Value::as_str)
                                    .unwrap_or_default()
                                    .into()
                            }
                        } else {
                            String::new()
                        }
                    } else {
                        value
                            .as_ref()
                            .and_then(Value::as_str)
                            .map(str::to_owned)
                            .or_else(|| {
                                value
                                    .as_ref()
                                    .filter(|v| v.is_number())
                                    .map(Value::to_string)
                            })
                            .unwrap_or_default()
                    };
                    Some((name.into(), TextEdit::new(text)))
                })
                .collect();
            self.drafts.insert(
                owner,
                Draft {
                    values,
                    editors,
                    status: Status::Editing,
                    position: Default::default(),
                },
            );
        }
    }

    pub fn apply(&mut self, event: &Event) {
        if let Event::Completed(id, Err(error)) = event {
            for draft in self.drafts.values_mut() {
                if matches!(draft.status, Status::Submitting { request, .. } if request == *id) {
                    draft.status = Status::Failed(error.clone());
                }
            }
        }
    }
}

fn belongs(prompt: &QuestionPrompt, session: &caelum_core::application::Session) -> bool {
    prompt.provider == session.scope.provider.as_str()
        && prompt.project_path == session.scope.project
        && Some(&prompt.session_id) == session.remote_id.as_ref()
}

pub fn active(l: &Live, index: usize) -> Option<&QuestionPrompt> {
    let session = l.app.sessions.get(index)?;
    // Keep arrival order even when a provider's next snapshot sorts requests differently.
    l.forms
        .order
        .iter()
        .find_map(|owner| {
            l.app
                .questions
                .iter()
                .find(|p| matches_key(p, owner) && belongs(p, session))
        })
        .or_else(|| l.app.questions.iter().find(|p| belongs(p, session)))
}

fn scroll_key(index: usize, prompt: &QuestionPrompt) -> String {
    format!("question-fields-{index}-{}", prompt.request_id)
}

pub fn sync(l: &mut Live, tx: &mut Tx) -> bool {
    let next = l
        .app
        .selected
        .filter(|_| l.panels.screen != Screen::Review && l.panels.overlay.is_none())
        .and_then(|index| active(l, index).map(|p| (index, key(p))));
    if l.forms.shown == next {
        return false;
    }
    let previous = std::mem::replace(&mut l.forms.shown, next.clone());
    for draft in l.forms.drafts.values_mut() {
        for editor in draft.editors.values_mut() {
            editor.focused = false;
        }
    }
    if let Some((index, owner)) = next {
        l.drafts[index].focused = false;
        l.commands.side_editor.focused = false;
        l.models = Default::default();
        if let Some((pending, ..)) = l.completion.pending.take() {
            l.app.cancel_mentions(pending);
        }
        if let Some(prompt) = active(l, index) {
            tx.scroll_to_y(
                scroll_key(index, prompt),
                l.forms.drafts[&owner].position.get().offset[1],
            );
        }
        focus(l);
    } else if previous.is_some_and(|(index, _)| Some(index) == l.app.selected)
        && l.panels.screen != Screen::Review
        && l.panels.overlay.is_none()
    {
        if l.commands.side_owner == l.app.selected && l.commands.side_owner.is_some() {
            l.commands.side_editor.focused = true;
        } else {
            l.focus();
        }
    }
    true
}

pub fn focus(l: &mut Live) -> bool {
    let Some(index) = l.app.selected else {
        return false;
    };
    let Some(prompt) = active(l, index).cloned() else {
        return false;
    };
    l.drafts[index].focused = false;
    let Some(draft) = l.forms.drafts.get_mut(&key(&prompt)) else {
        return true;
    };
    let checked = answer(&prompt, draft);
    // Text-only questions focus their first active field. Choice groups use
    // Rune's focus handoff from the replaced message composer and native arrows.
    if let Some(field) = prompt.fields.iter().find(|field| {
        field["key"]
            .as_str()
            .is_some_and(|name| checked.active.iter().any(|k| k == name))
    }) && has_input(field)
        && !has_options(field)
        && !draft.submitting()
        && let Some(editor) = field["key"]
            .as_str()
            .and_then(|name| draft.editors.get_mut(name))
    {
        editor.focused = true;
    }
    true
}

pub fn on_key(s: &mut Workspace, ev: &KeyEvent, tx: &mut Tx) -> bool {
    let l = live(s);
    if l.panels.screen == Screen::Review || l.panels.overlay.is_some() {
        return false;
    }
    let Some(prompt) = l.app.selected.and_then(|index| active(l, index)).cloned() else {
        return false;
    };
    if ev.key == InputKey::Escape || (ev.key == InputKey::Enter && ev.mods.cmd) {
        submit(s, &prompt, ev.key == InputKey::Escape, tx);
        true
    } else {
        false
    }
}

fn submit(s: &mut Workspace, prompt: &QuestionPrompt, dismiss: bool, tx: &mut Tx) {
    let l = live_mut(s);
    let owner = key(prompt);
    if !l.app.questions.iter().any(|p| matches_key(p, &owner)) {
        return;
    }
    let Some(draft) = l.forms.drafts.get_mut(&owner) else {
        return;
    };
    if draft.submitting() {
        return;
    }
    let checked = answer(prompt, draft);
    if !dismiss && !checked.errors.is_empty() {
        draft.status = Status::Invalid;
        if let Some(index) = l.app.selected {
            tx.scroll_to_y(scroll_key(index, prompt), 0.0);
        }
        return;
    }
    if let Some(request) = l
        .app
        .answer(prompt.clone(), (!dismiss).then_some(checked.values))
    {
        draft.status = Status::Submitting { request, dismiss };
    }
}

fn has_options(field: &Value) -> bool {
    field["type"] == "boolean" || field["options"].as_array().is_some_and(|v| !v.is_empty())
}

fn has_input(field: &Value) -> bool {
    field["type"] != "external" && (!has_options(field) || field["custom"] == true)
}

fn answer(prompt: &QuestionPrompt, draft: &Draft) -> forms::Answer {
    let mut values = draft.values.clone();
    for field in &prompt.fields {
        let Some(name) = field["key"].as_str() else {
            continue;
        };
        let Some(editor) = draft.editors.get(name) else {
            continue;
        };
        match field["type"].as_str().unwrap_or_default() {
            "string" | "number" | "integer"
                if !has_options(field) || (field["custom"] == true && !editor.text.is_empty()) =>
            {
                values.insert(name.into(), Value::String(editor.text.clone()));
            }
            "multiselect" => {
                let mut selected = values
                    .get(name)
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                if field["custom"] == true {
                    selected.extend(
                        editor
                            .text
                            .lines()
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                            .map(|s| Value::String(s.into())),
                    );
                }
                values.insert(name.into(), Value::Array(selected));
            }
            _ => {}
        }
    }
    forms::validate(prompt, &values)
}

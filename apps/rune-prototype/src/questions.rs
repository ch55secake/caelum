use crate::{
    Workspace,
    live::{live, live_mut},
};
use caelum_core::application::forms;
use caelum_protocol::QuestionPrompt;
use rune::{El, TextEdit, col, row, text, text_input};
use rune_kit::{Button, ButtonVariant, Kit};
use serde_json::{Map, Value};
use std::collections::HashMap;

type Key = (String, String, String, String);
fn key(p: &QuestionPrompt) -> Key {
    (
        p.provider.clone(),
        p.project_path.clone(),
        p.session_id.clone(),
        p.request_id.clone(),
    )
}

#[derive(Default)]
pub struct Forms {
    drafts: HashMap<Key, Draft>,
}
struct Draft {
    values: Map<String, Value>,
    editors: HashMap<String, TextEdit>,
    attempted: bool,
}

impl Forms {
    pub fn sync(&mut self, prompts: &[QuestionPrompt]) {
        self.drafts
            .retain(|k, _| prompts.iter().any(|p| key(p) == *k));
        for prompt in prompts {
            self.drafts.entry(key(prompt)).or_insert_with(|| {
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
                                        .any(|option| option["value"] == *v)
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
                Draft {
                    values,
                    editors,
                    attempted: false,
                }
            });
        }
    }
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
        let has_options = field["options"].as_array().is_some_and(|v| !v.is_empty());
        match field["type"].as_str().unwrap_or_default() {
            "string" | "number" | "integer"
                if !has_options || (field["custom"] == true && !editor.text.is_empty()) =>
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

pub fn view(s: &Workspace, k: &Kit, index: usize) -> El {
    let app = &live(s).app;
    let session = &app.sessions[index];
    let mut content = col().w_full().gap(16.0);
    for prompt in app.questions.iter().filter(|p| {
        p.provider == session.scope.provider.as_str()
            && p.project_path == session.scope.project
            && Some(&p.session_id) == session.remote_id.as_ref()
    }) {
        let owner = key(prompt);
        let Some(draft) = live(s).forms.drafts.get(&owner) else {
            continue;
        };
        let checked = answer(prompt, draft);
        let mut form = col()
            .w_full()
            .gap(12.0)
            .padding(16.0)
            .radius(10.0)
            .border(1.0, k.c.border_focus)
            .child(
                text(&prompt.title)
                    .font(k.ty.ui_medium)
                    .color(k.c.text_base)
                    .wrap(),
            );
        for field in &prompt.fields {
            let name = field["key"].as_str().unwrap_or_default();
            if !checked.active.iter().any(|key| key == name) {
                continue;
            }
            form = form.child(
                text(
                    field["label"]
                        .as_str()
                        .or(field["title"].as_str())
                        .unwrap_or(name),
                )
                .font(k.ty.ui_medium)
                .color(k.c.text_base)
                .wrap(),
            );
            if let Some(description) = field["description"].as_str() {
                form = form.child(
                    text(description)
                        .font(k.ty.small)
                        .color(k.c.text_muted)
                        .wrap(),
                );
            }
            let mut choices = col().w_full().gap(4.0);
            let options = if field["type"] == "boolean" {
                vec![
                    serde_json::json!({"label":"Yes", "value":true}),
                    serde_json::json!({"label":"No", "value":false}),
                ]
            } else {
                field["options"].as_array().cloned().unwrap_or_default()
            };
            for (i, option) in options.iter().enumerate() {
                let value = option["value"].clone();
                let owner = owner.clone();
                let name = name.to_owned();
                let multiple = field["type"] == "multiselect";
                let selected = draft.values.get(&name).is_some_and(|v| {
                    *v == value || v.as_array().is_some_and(|v| v.contains(&value))
                });
                let label = option["label"]
                    .as_str()
                    .or(option["value"].as_str())
                    .unwrap_or("Answer");
                let element_key = format!("question-{}-{name}-{i}", prompt.request_id);
                choices = choices.child(
                    Button::new(format!("{} {label}", if selected { "✓" } else { "○" }))
                        .variant(ButtonVariant::Outline)
                        .on_press(move |s: &mut Workspace| {
                            let draft = live_mut(s)
                                .forms
                                .drafts
                                .get_mut(&owner)
                                .expect("active question");
                            if multiple {
                                let mut values = draft
                                    .values
                                    .get(&name)
                                    .and_then(Value::as_array)
                                    .cloned()
                                    .unwrap_or_default();
                                if values.contains(&value) {
                                    values.retain(|v| v != &value);
                                } else {
                                    values.push(value.clone());
                                }
                                draft.values.insert(name.clone(), Value::Array(values));
                            } else {
                                draft.values.insert(name.clone(), value.clone());
                                draft.editors.get_mut(&name).unwrap().clear();
                            }
                        })
                        .build(k)
                        .key(element_key),
                );
            }
            form = form.child(choices);
            if field["type"] == "external" {
                if let Some(url) = field["url"]
                    .as_str()
                    .filter(|v| v.starts_with("https://") || v.starts_with("http://"))
                {
                    let url = url.to_owned();
                    form = form.child(
                        Button::new("Open link")
                            .variant(ButtonVariant::Outline)
                            .on_press_tx(move |_: &mut Workspace, tx| {
                                let _ = tx.open_url(&url);
                            })
                            .build(k),
                    );
                }
            } else if options.is_empty() && field["type"] != "boolean" || field["custom"] == true {
                let owner = owner.clone();
                let name = name.to_owned();
                form = form.child(
                    text_input(&draft.editors[&name], k.ty.ui)
                        .key(format!("question-input-{}-{name}", prompt.request_id))
                        .label(if field["custom"] == true {
                            "Custom answer"
                        } else {
                            "Answer"
                        })
                        .placeholder(
                            if field["custom"] == true {
                                "Other answer…"
                            } else {
                                "Your answer…"
                            },
                            k.c.text_muted,
                        )
                        .color(k.c.text_base)
                        .w_full()
                        .min_h(32.0)
                        .on_edit(move |s: &mut Workspace| {
                            live_mut(s)
                                .forms
                                .drafts
                                .get_mut(&owner)
                                .unwrap()
                                .editors
                                .get_mut(&name)
                                .unwrap()
                        }),
                );
            }
            if draft.attempted
                && let Some(error) = checked.errors.get(name)
            {
                form = form.child(text(error).font(k.ty.small).color(k.c.text_accent));
            }
        }
        let submit = prompt.clone();
        let dismiss = prompt.clone();
        form = form.child(
            row()
                .gap(8.0)
                .child(
                    Button::new("Submit answers")
                        .variant(ButtonVariant::Outline)
                        .on_press(move |s: &mut Workspace| {
                            let live = live_mut(s);
                            let draft = live.forms.drafts.get_mut(&key(&submit)).unwrap();
                            draft.attempted = true;
                            let checked = answer(&submit, draft);
                            if checked.errors.is_empty() {
                                live.app.answer(submit.clone(), Some(checked.values));
                            }
                        })
                        .build(k)
                        .key(format!("question-submit-{}", prompt.request_id)),
                )
                .child(
                    Button::new("Dismiss")
                        .variant(ButtonVariant::GhostMuted)
                        .on_press(move |s: &mut Workspace| {
                            live_mut(s).app.answer(dismiss.clone(), None)
                        })
                        .build(k),
                ),
        );
        content = content.child(form);
    }
    content
}

use super::{Status, answer, belongs, has_input, key, scroll_key, submit};
use crate::{
    Workspace,
    context::EditMenu,
    live::{live, live_mut},
    ui,
};
use caelum_protocol::QuestionPrompt;
use rune::{El, ScrollSpec, col, row, text, text_input};
use rune_kit::{ButtonVariant, Kit, ScrollBar};
use serde_json::Value;

pub fn view(s: &Workspace, k: &Kit, index: usize, prompt: &QuestionPrompt, padding: f32) -> El {
    let l = live(s);
    let owner = key(prompt);
    let Some(draft) = l.forms.drafts.get(&owner) else {
        return col();
    };
    let checked = answer(prompt, draft);
    let pending = draft.submitting();
    let count = l
        .app
        .questions
        .iter()
        .filter(|p| belongs(p, &l.app.sessions[index]))
        .count();
    let mut fields = col().w_full().gap(12.0).px(4.0).py(4.0).child(
        text(&prompt.title)
            .font(k.ty.ui_medium)
            .color(k.c.text_base)
            .wrap(),
    );
    if let Status::Failed(error) = &draft.status {
        fields = fields.child(
            ui::caption(error, k)
                .color(k.c.danger.fg)
                .role("alert")
                .key("question-error"),
        );
    }
    for field in &prompt.fields {
        let name = field["key"].as_str().unwrap_or_default();
        if !checked.active.iter().any(|key| key == name) {
            continue;
        }
        let label = field["label"]
            .as_str()
            .or(field["title"].as_str())
            .unwrap_or(name);
        let mut group = col()
            .w_full()
            .gap(6.0)
            .child(text(label).font(k.ty.ui_medium).color(k.c.text_base).wrap());
        if let Some(description) = field["description"].as_str() {
            group = group.child(ui::caption(description, k));
        }
        let multiple = field["type"] == "multiselect";
        let mut choices = col()
            .w_full()
            .gap(4.0)
            .focus_group()
            .role(if multiple { "group" } else { "radiogroup" })
            .label(label);
        let options = if field["type"] == "boolean" {
            vec![
                serde_json::json!({"label":"Yes","value":true}),
                serde_json::json!({"label":"No","value":false}),
            ]
        } else {
            field["options"].as_array().cloned().unwrap_or_default()
        };
        for (i, option) in options.iter().enumerate() {
            let value = option["value"].clone();
            let owner = owner.clone();
            let name = name.to_owned();
            let selected = draft
                .values
                .get(&name)
                .is_some_and(|v| *v == value || v.as_array().is_some_and(|v| v.contains(&value)));
            let label = option["label"]
                .as_str()
                .or(option["value"].as_str())
                .unwrap_or("Answer");
            // Answer text wraps rather than truncating the choice the user must make.
            let choice = k
                .interactive(
                    row()
                        .w_full()
                        .min_h(40.0)
                        .px(10.0)
                        .py(8.0)
                        .gap(10.0)
                        .center_y()
                        .child(
                            rune::icon(k.glyph.check_small, 14.0)
                                .color(k.c.text_accent)
                                .opacity(if selected { 1.0 } else { 0.0 }),
                        )
                        .child(
                            col()
                                .grow(1.0)
                                .min_w(0.0)
                                .gap(2.0)
                                .child(text(label).font(k.ty.ui).color(k.c.text_base).wrap())
                                .children(
                                    option["description"]
                                        .as_str()
                                        .map(|description| ui::caption(description, k)),
                                ),
                        ),
                )
                .role(if multiple { "checkbox" } else { "radio" })
                .label(label)
                .checked(selected)
                .disabled(pending)
                .focusable(!pending)
                .focus_selected(selected)
                .focus_ring(1.5, 0.0, k.c.border_focus)
                .on_press(move |s: &mut Workspace| {
                    let Some(draft) = live_mut(s).forms.drafts.get_mut(&owner) else {
                        return;
                    };
                    if draft.submitting() {
                        return;
                    }
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
                .key(format!(
                    "question-{}-{}-{i}",
                    prompt.request_id,
                    field["key"].as_str().unwrap_or_default()
                ))
                .radius(8.0)
                .fill(k.alpha(k.c.text_base, if selected { 0.075 } else { 0.025 }))
                .border(
                    1.0,
                    if selected {
                        k.c.border_focus
                    } else {
                        k.alpha(k.c.text_base, 0.06)
                    },
                );
            choices = choices.child(choice);
        }
        if !options.is_empty() {
            group = group.child(choices);
        }
        if field["type"] == "external" {
            if let Some(url) = field["url"]
                .as_str()
                .filter(|v| v.starts_with("https://") || v.starts_with("http://"))
            {
                let url = url.to_owned();
                group = group.child(
                    ui::Button::new("Open link")
                        .variant(ButtonVariant::Outline)
                        .on_press_tx(move |_: &mut Workspace, tx| {
                            let _ = tx.open_url(&url);
                        })
                        .build(k),
                );
            }
        } else if has_input(field) {
            let owner = owner.clone();
            let name = name.to_owned();
            let submit_prompt = prompt.clone();
            let input = text_input(&draft.editors[&name], k.ty.ui)
                .label(if field["custom"] == true {
                    "Custom answer"
                } else {
                    label
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
                .selection_color(k.selection())
                .min_h(36.0)
                .padding(8.0)
                .radius(7.0)
                .fill(k.alpha(k.c.text_base, 0.025))
                .border(
                    1.0,
                    if draft.editors[&name].focused {
                        k.c.border_focus
                    } else {
                        k.c.border_base
                    },
                )
                .disabled(pending)
                .edit_menu(
                    format!("question-input-{}-{name}", prompt.request_id),
                    move |s| {
                        live_mut(s)
                            .forms
                            .drafts
                            .get_mut(&owner)?
                            .editors
                            .get_mut(&name)
                    },
                )
                .on_input_key(crate::on_key)
                .on_submit(move |s: &mut Workspace, tx| submit(s, &submit_prompt, false, tx));
            group = group.child(if pending { input.inert() } else { input });
        }
        if matches!(draft.status, Status::Invalid)
            && let Some(error) = checked.errors.get(name)
        {
            group = group.child(ui::caption(error, k).color(k.c.danger.fg).role("alert"));
        }
        fields = fields.child(group);
    }
    let pane = rune::memo::window_layout("main-pane").map_or(700.0, |r| r[3]);
    let height = (pane - 148.0 - (pane * 0.3).max(140.0)).clamp(100.0, 420.0);
    let position = draft.position.clone();
    let restored = l.forms.shown.as_ref() == Some(&(index, owner));
    let submit_prompt = prompt.clone();
    let dismiss = prompt.clone();
    let submit_label = if pending {
        "Sending…"
    } else if matches!(draft.status, Status::Failed(_)) {
        "Retry answers"
    } else {
        "Submit answers"
    };
    ui::composer_dock(
        col()
            .key("question-composer")
            .role("form")
            .label("Answer agent question")
            .focus_return(if l.commands.side_owner == Some(index) {
                "side-question-input".into()
            } else {
                format!("live-draft-{index}")
            })
            .child(
                row()
                    .w_full()
                    .center_y()
                    .gap(8.0)
                    .px(4.0)
                    .pb(8.0)
                    .child(ui::caption("Your input needed", k).grow(1.0))
                    .children(
                        (count > 1).then(|| {
                            ui::caption(format!("{count} pending"), k).key("question-count")
                        }),
                    ),
            )
            .child(
                col()
                    .key(scroll_key(index, prompt))
                    .w_full()
                    .max_h(height)
                    .scroll(ScrollSpec {
                        stick_bottom: false,
                    })
                    .on_scroll_offset(move |at| {
                        if restored {
                            position.set(at);
                        }
                    })
                    .child(fields)
                    .scroll_bar(k),
            )
            .child(ui::divider(k).opacity(0.5).mt(8.0).mb(8.0))
            .child(
                row()
                    .key("question-actions")
                    .w_full()
                    .center_y()
                    .justify_end()
                    .gap(6.0)
                    .min_h(32.0)
                    .child(
                        ui::Button::new(
                            if matches!(draft.status, Status::Submitting { dismiss: true, .. }) {
                                "Dismissing…"
                            } else {
                                "Dismiss"
                            },
                        )
                        .variant(ButtonVariant::GhostMuted)
                        .disabled(pending)
                        .on_press_tx(move |s: &mut Workspace, tx| submit(s, &dismiss, true, tx))
                        .build(k)
                        .key(format!("question-dismiss-{}", prompt.request_id))
                        .help("Dismiss this question · Escape"),
                    )
                    .child(
                        ui::Button::new(submit_label)
                            .variant(ButtonVariant::Contrast)
                            .disabled(pending)
                            .on_press_tx(move |s: &mut Workspace, tx| {
                                submit(s, &submit_prompt, false, tx)
                            })
                            .build(k)
                            .key(format!("question-submit-{}", prompt.request_id))
                            .help("Submit answers · ⌘Enter"),
                    ),
            ),
        padding,
        k,
    )
}

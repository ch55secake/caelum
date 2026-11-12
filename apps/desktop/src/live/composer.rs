use rune::{El, TextStyle, col, row, text_input};
use rune_kit::{ButtonVariant, Kit, Placement};

use crate::context::EditMenu;
use crate::select::{Choice, Select};
use crate::ui::Button;
use crate::{SEND, STOP, Workspace, on_key, ui};

use super::{live, live_mut};

pub fn composer(s: &Workspace, k: &Kit, padding: f32, wide: bool, layers: &mut Vec<El>) -> El {
    if let Some(index) = live(s).app.selected
        && let Some(prompt) = crate::questions::active(live(s), index)
    {
        return crate::questions::view(s, k, index, prompt, padding);
    }
    if live(s).commands.side_owner == live(s).app.selected && live(s).commands.side_owner.is_some()
    {
        return crate::commands::side_view(s, k, padding);
    }
    let live = live(s);
    let Some(index) = live.app.selected else {
        return col();
    };
    let session = &live.app.sessions[index];
    let draft = &live.drafts[index];
    let queued = live.app.will_queue(index);
    let input = text_input(draft, TextStyle::new(14.0, 400.0).lh(23.0))
        .label("Message")
        .placeholder(
            "Use / for commands, @ for files, # for PRs.",
            k.c.text_muted,
        )
        .help(if queued {
            "Enter to queue · Shift+Enter for a new line"
        } else {
            "Enter to send · Shift+Enter for a new line"
        })
        .color(k.c.text_base)
        .selection_color(k.selection())
        .w_full()
        .edit_menu_changed(
            format!("live-draft-{index}"),
            move |s| live_mut(s).drafts.get_mut(index),
            crate::completion::changed,
        )
        .on_input_key(on_key)
        .on_submit(Workspace::send)
        .on_input_paste(move |s: &mut Workspace, content, _| {
            let Some(image) = content.image() else {
                return false;
            };
            live_mut(s)
                .app
                .attach_image(index, "Pasted image.png".into(), image.png().to_vec());
            true
        })
        .on_drop_files(move |s: &mut Workspace, paths, _| {
            let project = live_mut(s).app.sessions[index].scope.project.clone();
            for path in paths {
                match path.strip_prefix(&project) {
                    Ok(path) => live_mut(s)
                        .app
                        .attach_file(index, path.to_string_lossy().into_owned()),
                    Err(_) => live_mut(s).app.sessions[index].attachment_error(
                        path.to_string_lossy().into_owned(),
                        "Drop files from this session’s project".into(),
                    ),
                }
            }
        });
    let submit = Button::new(if queued { "Queue" } else { "Send" })
        .icon(if queued { &ui::QUEUE } else { &SEND })
        .variant(ButtonVariant::Contrast)
        .disabled(
            !session.can_submit()
                || (!queued && !session.can_send())
                || (draft.text.trim().is_empty() && session.attachments.is_empty()),
        )
        .on_press_tx(move |s: &mut Workspace, tx| {
            if live_mut(s).app.selected == Some(index) {
                s.send(tx);
            }
        })
        .build(k)
        .key("send")
        .label(if queued {
            "Queue message"
        } else {
            "Send message"
        })
        .help(if queued {
            "Queue message · Enter\nRuns after earlier work · Shift+Enter for a new line"
        } else {
            "Send message · Enter\nShift+Enter for a new line"
        });
    let actions = row()
        .key("composer-actions")
        .center_y()
        .justify_end()
        .gap(6.0)
        .children((session.working() || session.stopping).then(|| {
            Button::new(if session.stopping {
                "Stopping…"
            } else {
                "Stop"
            })
            .icon(&STOP)
            .variant(ButtonVariant::GhostMuted)
            .disabled(session.stopping)
            .on_press(move |s: &mut Workspace| {
                let live = live_mut(s);
                live.schedule();
                live.app.cancel(index);
            })
            .build(k)
            .px(8.0)
            .key("stop")
            .label("Stop generation")
            .help("Stop generation and pause the queue · Escape")
        }))
        .child(submit);
    let pickers = pickers(s, k, layers).min_w(0.0);
    let usage_label = wide || rune::memo::window_layout("main-pane").is_some_and(|r| r[2] > 440.0);
    let usage = crate::activity::context(s, k, index, usage_label);
    let controls = if wide {
        row()
            .center_y()
            .gap(12.0)
            .child(pickers.grow(1.0))
            .child(usage)
            .child(actions)
    } else {
        col().gap(8.0).child(pickers.w_full()).child(
            row()
                .w_full()
                .center_y()
                .gap(8.0)
                .child(usage)
                .child(rune::el().grow(1.0))
                .child(actions),
        )
    }
    .key("composer-controls")
    .w_full();
    let activity = crate::activity::session_activity(s, index);
    let has_queue = !session.queue.is_empty() || !session.inbox.items.is_empty();
    let status =
        row()
            .w_full()
            .flex_wrap()
            .center_y()
            .gap(6.0)
            .children(activity.as_ref().map(|a| {
                crate::activity::indicator(a, session.options.agent.as_deref(), k).grow(1.0)
            }))
            .children(has_queue.then(|| crate::panels::queue_summary(s, k, index)));
    ui::composer(
        crate::composer::editor(s, index, input),
        controls,
        col()
            .key(format!("composer-context-{index}"))
            .w_full()
            .max_h(180.0)
            .gap(6.0)
            .children((activity.is_some() || has_queue).then_some(status))
            .children((!session.shells.is_empty()).then(|| crate::activity::shells(s, k, index)))
            .child(crate::panels::composer_context(s, k, index)),
        padding,
        k,
    )
}

fn pickers(s: &Workspace, k: &Kit, layers: &mut Vec<El>) -> El {
    let live = live(s);
    let maximum = rune::memo::window_layout("composer-options").map_or(220.0, |r| r[2].max(80.0));
    let mut content = row()
        .key("composer-options")
        .flex_wrap()
        .gap(6.0)
        .center_y();
    let Some(index) = live.app.selected else {
        return content;
    };
    let session = &live.app.sessions[index];
    let choices = live.app.choices.get(&session.scope);
    content = content.child(
        Select::new::<&str>("model", &[])
            .current(
                choices
                    .and_then(|c| {
                        c.models
                            .iter()
                            .find(|m| Some(&m.id) == session.options.model.as_ref())
                    })
                    .map(|m| m.name.as_str())
                    .or(session.options.model.as_deref()),
            )
            .placeholder("Provider default")
            .label("Model")
            .prominent()
            .placement(Placement::TopStart)
            .open(live.models.is_open())
            .disabled(session.archived || session.archiving)
            .on_toggle(|s: &mut Workspace| crate::models::open(s, None))
            .trigger(k)
            .max_w(maximum.min(220.0))
            .min_w(0.0),
    );
    if let Some(choices) = choices {
        let model_id = session
            .options
            .model
            .as_ref()
            .or(choices.default_model.as_ref());
        if let Some(model) = choices
            .models
            .iter()
            .find(|model| Some(&model.id) == model_id)
            && !model.efforts.is_empty()
        {
            let default = model
                .default_effort
                .as_deref()
                .and_then(|id| model.efforts.iter().find(|e| e.id == id))
                .map(choice_label);
            let mut efforts = vec![
                Choice::new("", "Model default").detail(default.unwrap_or("Let the model choose")),
            ];
            efforts.extend(model.efforts.iter().map(effort_choice));
            let current = session.options.effort.as_deref().unwrap_or("");
            let display = model
                .efforts
                .iter()
                .find(|e| e.id == current)
                .map(choice_label)
                .or(default.filter(|_| current.is_empty()))
                .unwrap_or(if current.is_empty() {
                    "Default"
                } else {
                    current
                });
            content = content.child(
                Select::choices("effort", efforts)
                    .current(Some(current))
                    .display(display)
                    .label("Reasoning effort")
                    .icon(&ui::EFFORT)
                    .menu_description("Depth of reasoning for the next message")
                    .width(280.0)
                    .placement(Placement::TopStart)
                    .open(live.menu == Some(("effort", index)))
                    .disabled(session.archived || session.archiving)
                    .on_toggle(|s: &mut Workspace| toggle_menu(s, "effort"))
                    .on_close(|s: &mut Workspace| live_mut(s).menu = None)
                    .on_choose(move |s: &mut Workspace, value, _| {
                        let live = live_mut(s);
                        live.app
                            .set_effort(index, (!value.is_empty()).then(|| value.into()));
                        live.menu = None;
                        live.focus();
                    })
                    .build(k, layers)
                    .max_w(maximum),
            );
        }
        if !choices.agents.is_empty() {
            let mut agents =
                vec![Choice::new("", "Default agent").detail("Use the provider's default agent")];
            agents.extend(
                choices
                    .agents
                    .iter()
                    .map(|agent| Choice::new(&agent.id, choice_label(agent))),
            );
            content = content.child(
                Select::choices("agent", agents)
                    .current(Some(session.options.agent.as_deref().unwrap_or("")))
                    .label("Agent")
                    .icon(&ui::AGENT)
                    .menu_description("Agent for the next message")
                    .width(280.0)
                    .placement(Placement::TopStart)
                    .open(live.menu == Some(("agent", index)))
                    .disabled(session.archived || session.archiving)
                    .on_toggle(|s: &mut Workspace| toggle_menu(s, "agent"))
                    .on_close(|s: &mut Workspace| live_mut(s).menu = None)
                    .on_choose(move |s: &mut Workspace, value, _| {
                        let live = live_mut(s);
                        live.app.sessions[index].options.agent =
                            (!value.is_empty()).then(|| value.into());
                        live.app.options_changed(index);
                        live.menu = None;
                        live.focus();
                    })
                    .build(k, layers)
                    .max_w(maximum),
            );
        }
    }
    content
}

fn choice_label(choice: &caelum_protocol::ProviderChoice) -> &str {
    if choice.name != choice.id {
        return &choice.name;
    }
    match choice.name.as_str() {
        "none" => "None",
        "minimal" => "Minimal",
        "low" => "Low",
        "medium" => "Medium",
        "high" => "High",
        "xhigh" => "Extra high",
        "max" => "Maximum",
        "build" => "Build",
        "plan" => "Plan",
        _ => &choice.name,
    }
}

fn effort_choice(effort: &caelum_protocol::ProviderChoice) -> Choice {
    let choice = Choice::new(&effort.id, choice_label(effort));
    match effort_description(&effort.id) {
        Some(description) => choice.detail(description),
        None => choice,
    }
}

pub(super) fn effort_description(id: &str) -> Option<&'static str> {
    match id.to_ascii_lowercase().as_str() {
        "none" => Some("Respond without extra reasoning"),
        "minimal" | "low" => Some("Less reasoning, quicker responses"),
        "medium" => Some("Balance reasoning depth and speed"),
        "high" => Some("Think more deeply about complex tasks"),
        "xhigh" | "max" => Some("Use the maximum reasoning effort for complex tasks"),
        _ => None,
    }
}

fn toggle_menu(s: &mut Workspace, name: &'static str) {
    let live = live_mut(s);
    let next = live.app.selected.map(|index| (name, index));
    live.menu = if live.menu == next { None } else { next };
}

pub fn open_effort(s: &mut Workspace, index: usize) {
    let live = live_mut(s);
    live.menu = Some(("effort", index));
    live.drafts[index].focused = false;
}

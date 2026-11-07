//! Native surfaces for local commands; provider requests and results stay in core.
use crate::{
    Workspace,
    live::{live, live_mut},
    panels::{self, Screen},
    quiet_button, theme, ui,
};
use caelum_core::application::{EntryKind, Task, commands::LocalCommand};
use caelum_protocol::OpenCodeStatsFilter;
use rune::{El, Key, KeyEvent, ScrollSpec, TextEdit, Tx, col, row, text, text_input};
use rune_kit::{Button, ButtonVariant, Kit, Markdown, Progress, glyphs};

pub struct State {
    pub owner: Option<usize>,
    pub search: TextEdit,
    pub stats_project: Option<String>,
    pub stats_days: Option<u64>,
    pub stats_anchor: u64,
    pub side_owner: Option<usize>,
    pub side_editor: TextEdit,
    pub previous_drafts: Vec<String>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            owner: None,
            search: TextEdit::new(""),
            stats_project: None,
            stats_days: None,
            stats_anchor: 0,
            side_owner: None,
            side_editor: TextEdit::new(""),
            previous_drafts: Vec::new(),
        }
    }
}

pub fn dispatch(s: &mut Workspace, tx: &mut Tx) -> bool {
    let Some(index) = live(s).app.selected else {
        return false;
    };
    let input = live(s).drafts[index].text.clone();
    let command = match LocalCommand::parse(&input, live(s).app.sessions[index].scope.provider) {
        Ok(Some(command)) => command,
        Ok(None) => return false,
        Err(error) => {
            live_mut(s).app.sessions[index].error = Some(error);
            return true;
        }
    };
    let (command, args) = command;
    let l = live_mut(s);
    if command == LocalCommand::SideQuestion && l.app.sessions[index].remote_id.is_none() {
        l.app.sessions[index].error =
            Some("Open an OpenCode session before asking a side question.".into());
        return true;
    }
    l.app.sessions[index].error = None;
    l.drafts[index].clear();
    l.app.sessions[index].draft.clear();
    l.commands.owner = Some(index);
    l.commands.search.clear();
    match command {
        LocalCommand::New => l.new_session(),
        LocalCommand::Files => panels::show(s, Screen::Files),
        LocalCommand::Changes => panels::show(s, Screen::Changes),
        LocalCommand::Settings => panels::show(s, Screen::Settings),
        LocalCommand::Timeline => {
            l.panels.overlay = Some(Screen::Timeline);
            l.commands.search.focused = true;
        }
        LocalCommand::Usage => {
            l.panels.overlay = Some(Screen::Usage);
            let project = l.app.sessions[index].scope.project.clone();
            l.app.task(Task::Usage { project });
        }
        LocalCommand::Stats => {
            l.commands.stats_anchor = caelum_core::application::presentation::now();
            l.panels.overlay = Some(Screen::Stats);
            refresh_stats(s);
        }
        LocalCommand::SideQuestion => {
            l.app.dismiss_side_question(index);
            l.commands.side_owner = Some(index);
            l.commands.side_editor = TextEdit::new(args);
            l.commands.side_editor.focused = true;
            let previous = l
                .commands
                .previous_drafts
                .get(index)
                .cloned()
                .unwrap_or_default();
            l.drafts[index].replace_text(previous);
            l.drafts[index].focused = false;
            if !args.is_empty() {
                ask(s, tx);
            }
        }
    }
    true
}

pub fn close_side(s: &mut Workspace) {
    let l = live_mut(s);
    if let Some(index) = l.commands.side_owner.take() {
        l.app.dismiss_side_question(index);
    }
    l.focus();
}
pub fn side_key(s: &mut Workspace, ev: &KeyEvent, _: &mut Tx) -> bool {
    if ev.key == Key::Escape && live(s).commands.side_owner.is_some() {
        close_side(s);
        true
    } else {
        false
    }
}
pub fn ask(s: &mut Workspace, _: &mut Tx) {
    let l = live_mut(s);
    let Some(index) = l.commands.side_owner else {
        return;
    };
    if l.app.sessions[index].attaching == 0 && l.app.ask(index, l.commands.side_editor.text.clone())
    {
        l.commands.side_editor.focused = false;
    }
}
pub fn side_view(s: &Workspace, k: &Kit, padding: f32) -> El {
    let l = live(s);
    let index = l.commands.side_owner.unwrap();
    let session = &l.app.sessions[index];
    let mut body = col()
        .w_full()
        .max_w(840.0)
        .gap(8.0)
        .padding(12.0)
        .radius(10.0)
        .border(1.0, k.c.border_base)
        .fill(k.c.bg_base)
        .child(
            row()
                .w_full()
                .center_y()
                .child(
                    text("Side question")
                        .font(k.ty.ui_medium)
                        .color(k.c.text_base)
                        .grow(1.0),
                )
                .child(
                    quiet_button(&glyphs::CLOSE, "Close side question")
                        .on_press(close_side)
                        .build(k),
                ),
        );
    for attachment in &session.side_attachments {
        let id = attachment.id.clone();
        body = body.child(
            Button::new(format!("{} ×", attachment.label()))
                .variant(ButtonVariant::GhostMuted)
                .disabled(session.generating)
                .on_press(move |s: &mut Workspace| {
                    live_mut(s).app.sessions[index]
                        .side_attachments
                        .retain(|a| a.id != id)
                })
                .build(k),
        );
    }
    if let Some((question, answer)) = &session.side_question {
        body = body.child(ui::caption(question, k));
        if session.generating {
            body = body.child(ui::caption("Answering independently…", k).role("status"));
        } else {
            if !answer.is_empty() {
                body = body.child(
                    col()
                        .key("side-answer")
                        .w_full()
                        .max_h(150.0)
                        .scroll(ScrollSpec {
                            stick_bottom: false,
                        })
                        .child(
                            Markdown::new(answer)
                                .selectable(true)
                                .render(&theme::markdown(k)),
                        ),
                );
            }
            if let Some(error) = &session.side_error {
                body = body.child(ui::caption(error, k).color(k.c.danger.fg));
            }
            let copy = answer.clone();
            let mut actions = row().gap(8.0).child(
                Button::new("Edit question")
                    .variant(ButtonVariant::Outline)
                    .on_press(move |s: &mut Workspace| {
                        let l = live_mut(s);
                        l.app.sessions[index].side_question = None;
                        l.app.sessions[index].side_error = None;
                        l.commands.side_editor.focused = true;
                    })
                    .build(k)
                    .key("edit-side-question"),
            );
            if session.side_error.is_some() {
                actions = actions.child(
                    Button::new("Retry")
                        .on_press_tx(ask)
                        .build(k)
                        .key("retry-side-question"),
                );
            } else {
                actions = actions.child(
                    Button::new("Copy answer")
                        .on_press_tx(move |_: &mut Workspace, tx| tx.copy(copy.clone()))
                        .build(k),
                );
            }
            body = body.child(actions);
        }
    } else {
        body = body
            .child(crate::completion::view(s, k))
            .child(rune::auto_grow(
                text_input(&l.commands.side_editor, k.ty.ui)
                    .key("side-question-input")
                    .label("Question")
                    .placeholder("Ask about this session…", k.c.text_muted)
                    .color(k.c.text_base)
                    .w_full()
                    .on_edit(|s: &mut Workspace| &mut live_mut(s).commands.side_editor)
                    .on_input_change(crate::completion::changed)
                    .on_input_key(crate::on_key)
                    .on_submit(ask),
                2,
                4,
            ))
            .child(
                Button::new("Ask")
                    .disabled(
                        l.commands.side_editor.text.trim().is_empty() || session.attaching > 0,
                    )
                    .on_press_tx(ask)
                    .build(k),
            );
    }
    col().w_full().items_center().px(padding).pb(20.0).child(
        body.role("region")
            .label("Side question")
            .key("side-question")
            .focusable(true),
    )
}

pub fn view(s: &Workspace, k: &Kit, screen: Screen) -> El {
    let Some(index) = live(s).commands.owner else {
        return col();
    };
    match screen {
        Screen::Timeline => timeline(s, k, index),
        Screen::Usage => usage(s, k, index),
        Screen::Stats => stats(s, k, index),
        _ => col(),
    }
}

fn timeline(s: &Workspace, k: &Kit, index: usize) -> El {
    let l = live(s);
    let session = &l.app.sessions[index];
    let query = l.commands.search.text.to_lowercase();
    let matches: Vec<_> = session
        .conversation
        .entries
        .iter()
        .enumerate()
        .filter(|(_, e)| e.kind == EntryKind::User && e.text.to_lowercase().contains(&query))
        .map(|(i, _)| i)
        .collect();
    let kit = k.clone();
    let ids = matches.clone();
    let count = matches.len();
    let rows = col()
        .w_full()
        .growth(rune::GrowthSpec::default())
        .virtual_rows(
            count,
            54.0,
            move |s: &Workspace, i| {
                format!(
                    "timeline-{}",
                    live(s).app.sessions[index].conversation.entries[ids[i]].id
                )
                .into()
            },
            move |s: &Workspace, i| {
                let position = matches[i];
                let entry = &live(s).app.sessions[index].conversation.entries[position];
                Button::new(caelum_core::application::split_context(&entry.text).0)
                    .variant(ButtonVariant::GhostMuted)
                    .on_press_tx(move |s: &mut Workspace, tx| {
                        let l = live_mut(s);
                        l.app.select(index);
                        l.panels.overlay = None;
                        l.panels.screen = Screen::Conversation;
                        tx.scroll_center_in_instant(
                            format!("live-rows-{index}"),
                            format!(
                                "live-entry-{index}-{}",
                                l.app.sessions[index].conversation.entries[position].id
                            ),
                        );
                    })
                    .build(&kit)
                    .h(54.0)
                    .w_full()
            },
        );
    let mut body = col()
        .w_full()
        .h_full()
        .gap(10.0)
        .min_h(0.0)
        .child(
            text_input(&l.commands.search, k.ty.ui)
                .key("timeline-search")
                .label("Find a prompt")
                .placeholder("Find a prompt…", k.c.text_muted)
                .color(k.c.text_base)
                .w_full()
                .single_line()
                .on_edit(|s: &mut Workspace| &mut live_mut(s).commands.search),
        )
        .child(
            col()
                .w_full()
                .grow(1.0)
                .min_h(0.0)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .child(rows),
        );
    if let Some(error) = &session.error {
        body = body.child(ui::caption(error, k).color(k.c.danger.fg));
    }
    if session.next_cursor.is_some() {
        body = body.child(
            Button::new(if session.loading {
                "Loading…"
            } else {
                "Load earlier prompts"
            })
            .disabled(session.loading)
            .on_press(move |s: &mut Workspace| live_mut(s).app.older(index))
            .build(k),
        );
    }
    body
}

fn usage(s: &Workspace, k: &Kit, index: usize) -> El {
    let l = live(s);
    let project = &l.app.sessions[index].scope.project;
    let task = Task::Usage {
        project: project.clone(),
    };
    let busy = l.app.services.busy(&task);
    let value = l
        .app
        .services
        .checkouts
        .get(project)
        .and_then(|c| c.usage.as_ref());
    let mut body = col()
        .w_full()
        .gap(12.0)
        .child(crate::activity::context(s, k, index, true));
    if let Some(value) = value {
        body = body.child(ui::caption(
            value["workspace"]["name"].as_str().unwrap_or("OpenCode Go"),
            k,
        ));
        let mut reported = false;
        for name in ["rolling", "weekly", "monthly"] {
            let allowance = &value["go"][name];
            if let Some(percent) = allowance["percent"].as_f64() {
                reported = true;
                body = body
                    .child(ui::caption(
                        format!(
                            "{name} · {percent:.0}% used · {:.0}% remaining{}",
                            (100.0 - percent).max(0.0),
                            if allowance["limited"] == true {
                                " · Limit reached"
                            } else {
                                ""
                            }
                        ),
                        k,
                    ))
                    .child(
                        Progress::new(Some((percent / 100.0).clamp(0.0, 1.0) as f32))
                            .label(format!("{name} allowance used"))
                            .build(k),
                    );
                if let Some(reset) = allowance["resetsAt"].as_u64() {
                    body = body.child(ui::caption(
                        format!(
                            "Resets in {}",
                            caelum_core::application::presentation::duration(
                                reset.saturating_sub(caelum_core::application::presentation::now())
                            )
                        ),
                        k,
                    ));
                }
            }
        }
        if !reported {
            body = body.child(ui::caption("No OpenCode Go allowances reported", k));
        }
        if let Some(budgets) = value["budgets"].as_array() {
            for budget in budgets {
                body = body.child(ui::caption(
                    format!(
                        "Console budget · ${:.2} / ${:.2}",
                        budget["spent"].as_f64().unwrap_or(0.0),
                        budget["limit"].as_f64().unwrap_or(0.0)
                    ),
                    k,
                ));
            }
        }
        if let Some(url) = value["workspace"]["usageURL"].as_str() {
            let url = url.to_owned();
            body = body.child(
                Button::new("Open usage dashboard")
                    .on_press_tx(move |_: &mut Workspace, tx| {
                        let _ = tx.open_url(&url);
                    })
                    .build(k),
            );
        }
    } else {
        body = body.child(ui::caption(
            if busy {
                "Loading OpenCode Go usage…"
            } else {
                "No usage reported"
            },
            k,
        ));
    }
    service_status(body, s, k, task, value.is_some())
}

fn stats_task(s: &Workspace) -> Option<Task> {
    let l = live(s);
    let index = l.commands.owner?;
    Some(Task::Stats {
        project: l.app.sessions[index].scope.project.clone(),
        filter: OpenCodeStatsFilter {
            from: l
                .commands
                .stats_days
                .map(|days| l.commands.stats_anchor.saturating_sub(days * 86_400_000)),
            project: l.commands.stats_project.clone(),
            ..Default::default()
        },
    })
}
fn refresh_stats(s: &mut Workspace) {
    if let Some(task) = stats_task(s) {
        live_mut(s).app.task(task);
    }
}
fn stats(s: &Workspace, k: &Kit, index: usize) -> El {
    let l = live(s);
    let checkout = l
        .app
        .services
        .checkouts
        .get(&l.app.sessions[index].scope.project);
    let mut ranges = row().w_full().gap(5.0);
    for (name, days) in [
        ("All history", None),
        ("7 days", Some(7)),
        ("30 days", Some(30)),
    ] {
        ranges = ranges.child(
            Button::new(name)
                .variant(if l.commands.stats_days == days {
                    ButtonVariant::Outline
                } else {
                    ButtonVariant::GhostMuted
                })
                .on_press(move |s: &mut Workspace| {
                    live_mut(s).commands.stats_days = days;
                    refresh_stats(s);
                })
                .build(k),
        );
    }
    let mut body = col().w_full().gap(10.0).child(ranges).child(
        text_input(&l.commands.search, k.ty.ui)
            .single_line()
            .label("Find statistics project")
            .placeholder("Search projects by name or path…", k.c.text_muted)
            .color(k.c.text_base)
            .w_full()
            .on_edit(|s: &mut Workspace| &mut live_mut(s).commands.search),
    );
    let query = l.commands.search.text.to_lowercase();
    let mut projects = col().w_full().gap(3.0).child(
        Button::new("All projects")
            .variant(if l.commands.stats_project.is_none() {
                ButtonVariant::Outline
            } else {
                ButtonVariant::GhostMuted
            })
            .on_press(|s: &mut Workspace| {
                live_mut(s).commands.stats_project = None;
                refresh_stats(s);
            })
            .build(k),
    );
    if let Some(c) = checkout {
        for project in c.stats_projects.iter().filter(|p| {
            format!("{} {}", p.name.as_deref().unwrap_or_default(), p.canonical)
                .to_lowercase()
                .contains(&query)
        }) {
            let id = project.id.clone();
            projects = projects.child(
                Button::new(format!(
                    "{} · {}",
                    project.name.as_deref().unwrap_or("Project"),
                    project.canonical
                ))
                .variant(if l.commands.stats_project.as_ref() == Some(&id) {
                    ButtonVariant::Outline
                } else {
                    ButtonVariant::GhostMuted
                })
                .on_press(move |s: &mut Workspace| {
                    live_mut(s).commands.stats_project = Some(id.clone());
                    refresh_stats(s);
                })
                .build(k),
            );
        }
    }
    body = body.child(
        col()
            .w_full()
            .max_h(100.0)
            .scroll(ScrollSpec {
                stick_bottom: false,
            })
            .child(projects),
    );
    if let Some(stats) = checkout.and_then(|c| c.stats.as_ref()) {
        body = body.child(
            text(format!(
                "{} sessions · {} prompts · {} steps\n${:.2} · {} active days · {} day streak",
                stats.sessions,
                stats.prompts,
                stats.steps,
                stats.cost,
                stats.active_days,
                stats.streak
            ))
            .font(k.ty.ui_medium)
            .color(k.c.text_base)
            .wrap(),
        );
        for (name, n) in [
            ("Input", stats.tokens.input),
            ("Output", stats.tokens.output),
            ("Reasoning", stats.tokens.reasoning),
            ("Cache read", stats.tokens.cache.read),
            ("Cache write", stats.tokens.cache.write),
        ] {
            body = body.child(
                ui::caption(format!("{name}: {} tokens", compact(n)), k)
                    .help(format!("{n:.0} tokens")),
            );
        }
        for model in &stats.models {
            body = body.child(ui::caption(
                format!(
                    "{}/{} · {} steps · ${:.2}",
                    model.model.provider_id, model.model.id, model.steps, model.cost
                ),
                k,
            ));
        }
        for day in &stats.activity {
            body = body.child(ui::caption(
                format!("{} · {} steps", day.date, day.steps),
                k,
            ));
        }
    }
    service_status(
        body,
        s,
        k,
        stats_task(s).unwrap(),
        checkout.is_some_and(|c| c.stats.is_some()),
    )
}
fn compact(n: f64) -> String {
    if n >= 1e9 {
        format!("{:.1}B", n / 1e9)
    } else if n >= 1e6 {
        format!("{:.1}M", n / 1e6)
    } else if n >= 1e3 {
        format!("{:.1}K", n / 1e3)
    } else {
        format!("{n:.0}")
    }
}
fn service_status(mut body: El, s: &Workspace, k: &Kit, task: Task, cached: bool) -> El {
    let services = &live(s).app.services;
    if let Some(error) = services.error(&task) {
        body = body.child(
            ui::caption(
                format!(
                    "{error}{}",
                    if cached {
                        " · Showing the last reported result."
                    } else {
                        ""
                    }
                ),
                k,
            )
            .color(k.c.danger.fg),
        );
    }
    body = body.child(
        Button::new(if services.busy(&task) {
            "Loading…"
        } else if services.error(&task).is_some() {
            "Retry"
        } else {
            "Refresh"
        })
        .disabled(services.busy(&task))
        .on_press(move |s: &mut Workspace| live_mut(s).app.task(task.clone()))
        .build(k),
    );
    col()
        .w_full()
        .h_full()
        .min_h(0.0)
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .child(body)
}

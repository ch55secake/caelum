//! Native surfaces for local commands; provider requests and results stay in core.
use crate::context::EditMenu;
use crate::ui::Button;
use crate::{
    Workspace,
    live::{live, live_mut},
    panels::{self, Screen},
    quiet_button, theme, ui,
};
use caelum_core::application::{EntryKind, Task, commands::LocalCommand};
use caelum_protocol::OpenCodeStatsFilter;
use rune::{El, Key, KeyEvent, ScrollSpec, TextEdit, Tx, col, row, text, text_input};
use rune_kit::{ButtonVariant, Kit, Markdown, Progress, ScrollBar, glyphs};

pub struct State {
    pub owner: Option<usize>,
    pub search: TextEdit,
    pub stats_project: Option<String>,
    pub stats_days: Option<u64>,
    pub stats_anchor: u64,
    pub stats_projects_open: bool,
    pub timeline_selected: Option<String>,
    pub timeline_current: Option<String>,
    pub sessions_search: TextEdit,
    pub sessions_selected: Option<String>,
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
            stats_projects_open: false,
            timeline_selected: None,
            timeline_current: None,
            sessions_search: TextEdit::new(""),
            sessions_selected: None,
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
        LocalCommand::Sessions => {
            let scope = l.app.sessions[index].scope.clone();
            l.commands.sessions_search.clear();
            l.commands.sessions_selected = None;
            l.commands.sessions_search.focused = true;
            l.panels.overlay = Some(Screen::Sessions);
            l.app.discover_sessions(scope);
        }
        LocalCommand::Models => crate::models::open(s, None),
        LocalCommand::Variants => open_variants(s, index),
        LocalCommand::Effort => {
            if args.is_empty() {
                open_variants(s, index);
            } else {
                set_effort(s, index, args);
            }
        }
        LocalCommand::Files => panels::show(s, Screen::Files),
        LocalCommand::Changes => panels::show(s, Screen::Changes),
        LocalCommand::Settings => panels::show(s, Screen::Settings),
        LocalCommand::Timeline => {
            l.commands.timeline_selected = l.current_prompt.clone();
            l.commands.timeline_current = l.current_prompt.clone();
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

fn open_variants(s: &mut Workspace, index: usize) {
    let session = &live(s).app.sessions[index];
    let choices = live(s).app.choices.get(&session.scope);
    let model_id = session
        .options
        .model
        .as_ref()
        .or_else(|| choices.and_then(|choices| choices.default_model.as_ref()));
    let has_variants = choices
        .and_then(|choices| {
            choices
                .models
                .iter()
                .find(|model| Some(&model.id) == model_id)
        })
        .is_some_and(|model| !model.efforts.is_empty());
    if has_variants {
        crate::live::open_effort(s, index);
    } else {
        live_mut(s).app.sessions[index].error =
            Some("No model effort variants are available for the selected model.".into());
    }
}

fn set_effort(s: &mut Workspace, index: usize, value: &str) {
    let scope = live(s).app.sessions[index].scope.clone();
    let choices = live(s).app.choices.get(&scope);
    let model_id = live(s).app.sessions[index]
        .options
        .model
        .clone()
        .or_else(|| choices.and_then(|choices| choices.default_model.clone()));
    let reset = matches!(
        value.to_ascii_lowercase().as_str(),
        "default" | "model default"
    );

    let effort = if reset {
        Some(None)
    } else {
        choices
            .and_then(|choices| {
                choices
                    .models
                    .iter()
                    .find(|model| Some(&model.id) == model_id.as_ref())
            })
            .and_then(|model| {
                model
                    .efforts
                    .iter()
                    .find(|effort| {
                        effort.id.eq_ignore_ascii_case(value)
                            || effort.name.eq_ignore_ascii_case(value)
                    })
                    .map(|effort| Some(effort.id.clone()))
            })
    };

    if let Some(effort) = effort {
        let l = live_mut(s);
        l.app.sessions[index].options.effort = effort;
        l.app.options_changed(index);
        return;
    }

    let available = choices
        .and_then(|choices| {
            choices
                .models
                .iter()
                .find(|model| Some(&model.id) == model_id.as_ref())
        })
        .map(|model| {
            model
                .efforts
                .iter()
                .map(|effort| effort.id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        })
        .filter(|values| !values.is_empty())
        .unwrap_or_else(|| "none".into());
    live_mut(s).app.sessions[index].error = Some(format!(
        "Unknown effort ‘{value}’. Available efforts: {available}. Use /effort default to reset."
    ));
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
        body = body.child(ui::caption(
            caelum_core::application::presentation::visible_text(question),
            k,
        ));
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
                            Markdown::new(caelum_core::application::presentation::visible_text(
                                answer,
                            ))
                            .selectable(true)
                            .render(&theme::markdown(k)),
                        ),
                );
            }
            if let Some(error) = &session.side_error {
                body = body.child(ui::caption(error, k).color(k.c.danger.fg));
            }
            let copy = caelum_core::application::presentation::visible_text(answer);
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
                    .label("Question")
                    .placeholder("Ask about this session…", k.c.text_muted)
                    .color(k.c.text_base)
                    .w_full()
                    .edit_menu_changed(
                        "side-question-input",
                        |s| Some(&mut live_mut(s).commands.side_editor),
                        crate::completion::changed,
                    )
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
    if let Some(activity) = crate::activity::session_activity(s, index) {
        body = body.child(crate::activity::indicator(
            &activity,
            session.options.agent.as_deref(),
            k,
        ));
    }
    col().w_full().items_center().px(padding).pb(20.0).child(
        body.role("region")
            .label("Side question")
            .key("side-question")
            .focusable(true),
    )
}

pub fn view(s: &Workspace, k: &Kit, screen: Screen, layers: &mut Vec<El>) -> El {
    let Some(index) = live(s).commands.owner else {
        return col();
    };
    match screen {
        Screen::Timeline => timeline(s, k, index),
        Screen::Sessions => sessions(s, k, index),
        Screen::Usage => usage(s, k, index),
        Screen::Stats => stats(s, k, index, layers),
        _ => col(),
    }
}

fn session_matches(s: &Workspace, index: usize) -> Vec<usize> {
    let l = live(s);
    let current = &l.app.sessions[index];
    let query = l.commands.sessions_search.text.to_lowercase();
    let project_group = l.app.group_path(&current.scope.project);
    let matches: Vec<_> = l
        .app
        .sessions
        .iter()
        .enumerate()
        .filter(|(_, session)| {
            !session.forgotten
                && session.remote_id.is_some()
                && session.scope.provider == current.scope.provider
                && l.app.group_path(&session.scope.project) == project_group
                && format!(
                    "{} {} {} {}",
                    session.title,
                    session.scope.provider.as_str(),
                    session.scope.project,
                    session.remote_id.as_deref().unwrap_or_default()
                )
                .to_lowercase()
                .contains(&query)
        })
        .map(|(index, _)| index)
        .collect();
    matches
}

fn sessions(s: &Workspace, k: &Kit, index: usize) -> El {
    let l = live(s);
    let owner = &l.app.sessions[index];
    let project = owner.scope.project.clone();
    let scope = owner.scope.clone();
    let matches = session_matches(s, index);
    let selected = l.commands.sessions_selected.clone();
    let rows = col()
        .w_full()
        .gap(3.0)
        .key("provider-sessions")
        .role("list")
        .focus_group();
    let mut rows = rows;
    for session_index in &matches {
        let session_index = *session_index;
        let session = &l.app.sessions[session_index];
        let id = session.remote_id.as_deref().unwrap_or_default().to_owned();
        let title = if session.title.trim().is_empty() {
            "Untitled session"
        } else {
            &session.title
        };
        let project_name = session
            .scope
            .project
            .rsplit('/')
            .next()
            .unwrap_or("Project");
        let detail = if session.archived {
            format!("Archived · {project_name}")
        } else if session.working() {
            format!("Working · {project_name}")
        } else {
            project_name.to_owned()
        };
        let selected = selected.as_deref() == Some(&id);
        rows = rows.child(
            ui::choice(title, &detail, selected, k)
                .key(format!("provider-session-{session_index}"))
                .on_press(move |s: &mut Workspace| select_provider_session(s, session_index)),
        );
    }

    let mut body = col()
        .w_full()
        .h_full()
        .min_h(0.0)
        .gap(10.0)
        .child(
            ui::field(
                &l.commands.sessions_search,
                "Search provider sessions",
                "Search sessions…",
                k,
            )
            .w_full()
            .single_line()
            .edit_menu_changed(
                "provider-session-search",
                |s| Some(&mut live_mut(s).commands.sessions_search),
                |s, _| live_mut(s).commands.sessions_selected = None,
            )
            .on_input_key(sessions_key),
        )
        .child(
            col()
                .key("provider-sessions-scroll")
                .w_full()
                .grow(1.0)
                .min_h(0.0)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .child(rows),
        );
    if matches.is_empty() {
        body = body.child(ui::empty(
            "No matching sessions",
            "Refresh to search this provider’s active and archived sessions.",
            k,
        ));
    }
    if let Some(error) = l.app.notices.get(&scope) {
        body = body.child(ui::caption(error, k).color(k.c.danger.fg));
    }
    let refresh_scope = scope.clone();
    body.child(ui::divider(k)).child(
        row()
            .w_full()
            .center_y()
            .justify_between()
            .child(ui::caption(
                format!(
                    "{} · {} sessions · {} · ↑↓ Navigate · ↵ Open",
                    scope.provider.as_str(),
                    matches.len(),
                    project
                ),
                k,
            ))
            .child(
                Button::new("Refresh")
                    .variant(ButtonVariant::Outline)
                    .on_press(move |s: &mut Workspace| {
                        live_mut(s).app.discover_sessions(refresh_scope.clone())
                    })
                    .build(k),
            ),
    )
}

fn select_provider_session(s: &mut Workspace, index: usize) {
    let l = live_mut(s);
    l.app.select(index);
    l.panels.overlay = None;
    l.panels.screen = Screen::Conversation;
    l.commands.sessions_search.focused = false;
    l.commands.sessions_selected = None;
    l.current_prompt = None;
}

pub fn sessions_key(s: &mut Workspace, ev: &KeyEvent, tx: &mut Tx) -> bool {
    let Some(owner) = live(s).commands.owner else {
        return false;
    };
    let matches = session_matches(s, owner);
    let selected = live(s).commands.sessions_selected.as_ref();
    let current = matches
        .iter()
        .position(|index| live(s).app.sessions[*index].remote_id.as_ref() == selected);
    let at = match ev.key {
        Key::Down => Some(current.map_or(0, |i| (i + 1).min(matches.len().saturating_sub(1)))),
        Key::Up => Some(current.unwrap_or(0).saturating_sub(1)),
        Key::Enter if !matches.is_empty() => {
            let index = matches[current.unwrap_or(0)];
            select_provider_session(s, index);
            return true;
        }
        _ => return false,
    };
    if let Some(index) = matches.get(at.unwrap_or_default()) {
        let id = live(s).app.sessions[*index].remote_id.clone();
        live_mut(s).commands.sessions_selected = id;
        live_mut(s).commands.sessions_search.focused = true;
        tx.scroll_center_in_instant(
            "provider-sessions-scroll",
            format!("provider-session-{index}"),
        );
    }
    true
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
        .filter(|(_, e)| {
            e.kind == EntryKind::User && caelum_core::application::presentation::entry_visible(e)
        })
        .enumerate()
        .filter(|(_, (_, e))| {
            caelum_core::application::presentation::entry_text(e)
                .to_lowercase()
                .contains(&query)
        })
        .map(|(ordinal, (i, _))| (ordinal + 1, i))
        .collect();
    let kit = k.clone();
    let ids = matches.clone();
    let count = matches.len();
    let rows = col()
        .key("timeline-rows")
        .w_full()
        .growth(rune::GrowthSpec::default())
        .virtual_rows(
            count,
            82.0,
            move |s: &Workspace, i| {
                format!(
                    "timeline-{}",
                    live(s).app.sessions[index].conversation.entries[ids[i].1].id
                )
                .into()
            },
            move |s: &Workspace, i| {
                let (ordinal, position) = matches[i];
                let entry = &live(s).app.sessions[index].conversation.entries[position];
                let id = entry.id.clone();
                let current = live(s).commands.timeline_current.as_ref() == Some(&id);
                let selected = live(s).commands.timeline_selected.as_ref() == Some(&id);
                let shown = caelum_core::application::presentation::entry_text(entry);
                let excerpt = shown
                    .lines()
                    .find(|line| !line.trim().is_empty())
                    .unwrap_or("Attached context")
                    .trim()
                    .to_owned();
                let detail = shown
                    .lines()
                    .skip(1)
                    .filter(|l| !l.trim().is_empty())
                    .collect::<Vec<_>>()
                    .join(" ");
                let highlights = |value: &str| {
                    caelum_core::application::presentation::search_ranges(
                        value,
                        &live(s).commands.search.text,
                    )
                    .into_iter()
                    .map(|r| {
                        rune::Span::new(r.start, r.end)
                            .color(kit.c.text_accent)
                            .weight(600.0)
                            .bg(kit.c.layer_03, 1.0)
                    })
                    .collect::<Vec<_>>()
                };
                let group =
                    |e: &caelum_core::application::Entry| e.started_at.map(|at| at / 86_400_000);
                let day = group(entry);
                let start = i == 0
                    || group(&live(s).app.sessions[index].conversation.entries[matches[i - 1].1])
                        != day;
                let day_label = day.map_or_else(
                    || "This session".into(),
                    |day| match (caelum_core::application::presentation::now() / 86_400_000)
                        .saturating_sub(day)
                    {
                        0 => "Today".into(),
                        1 => "Yesterday".into(),
                        n => format!("{n} days ago"),
                    },
                );
                let mut item = col().w_full().gap(5.0);
                if start {
                    item = item.child(ui::caption(day_label, &kit).px(10.0).pt(8.0));
                }
                item.child(
                    kit.interactive(
                        row()
                            .w_full()
                            .min_h(62.0)
                            .padding(10.0)
                            .gap(12.0)
                            .radius(8.0)
                            .fill(if selected {
                                kit.c.hover
                            } else {
                                rune::Color::TRANSPARENT
                            })
                            .child(text(format!("{ordinal:02}")).font(kit.ty.mono).color(
                                if current {
                                    kit.c.text_accent
                                } else {
                                    kit.c.text_faint
                                },
                            ))
                            .child(
                                col()
                                    .grow(1.0)
                                    .min_w(0.0)
                                    .gap(4.0)
                                    .child(
                                        text(&excerpt)
                                            .spans(highlights(&excerpt))
                                            .font(kit.ty.ui)
                                            .color(kit.c.text_base)
                                            .truncate(),
                                    )
                                    .child(
                                        ui::caption(
                                            if current {
                                                "Reading here"
                                            } else if detail.is_empty() {
                                                "Prompt"
                                            } else {
                                                &detail
                                            },
                                            &kit,
                                        )
                                        .spans(if current {
                                            Vec::new()
                                        } else {
                                            highlights(&detail)
                                        })
                                        .truncate(),
                                    ),
                            ),
                    )
                    .role("button")
                    .label(&excerpt)
                    .description(if current {
                        "Current reading position"
                    } else {
                        "Reveal in conversation"
                    })
                    .focusable(true)
                    .focus_ring(1.5, 0.0, kit.c.border_focus)
                    .on_press_tx(move |s: &mut Workspace, tx| {
                        jump(s, index, &id, tx);
                    }),
                )
            },
        );
    let mut body = col()
        .w_full()
        .h_full()
        .gap(10.0)
        .min_h(0.0)
        .child(
            ui::field(
                &l.commands.search,
                "Find a prompt",
                "Search your prompts…",
                k,
            )
            .label("Find a prompt")
            .placeholder("Find a prompt…", k.c.text_muted)
            .color(k.c.text_base)
            .w_full()
            .single_line()
            .edit_menu_changed(
                "timeline-search",
                |s| Some(&mut live_mut(s).commands.search),
                |s, tx| {
                    live_mut(s).commands.timeline_selected = None;
                    tx.scroll_to_y("timeline-scroll", 0.0);
                },
            )
            .on_input_key(timeline_key),
        )
        .child(
            col()
                .key("timeline-scroll")
                .w_full()
                .grow(1.0)
                .min_h(0.0)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .child(rows),
        );
    if count == 0 {
        body = body.child(ui::empty(
            "No matching prompts",
            "Try another search or load earlier history.",
            k,
        ));
    }
    if let Some(error) = &session.error {
        body = body.child(ui::caption(error, k).color(k.c.danger.fg));
    }
    body = body.child(ui::divider(k)).child(ui::caption(
        format!("{count} prompts · ↑↓ Navigate · ↵ Jump · Esc Close"),
        k,
    ));
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

fn jump(s: &mut Workspace, index: usize, id: &str, tx: &mut Tx) {
    let l = live_mut(s);
    if !l.app.sessions[index]
        .conversation
        .entries
        .iter()
        .any(|e| e.id == id)
    {
        return;
    }
    l.app.select(index);
    l.current_prompt = Some(id.into());
    l.panels.overlay = None;
    l.panels.screen = Screen::Conversation;
    l.commands.search.focused = false;
    tx.scroll_center_in_instant(
        format!("live-rows-{index}"),
        format!("live-entry-{index}-{id}"),
    );
}

pub fn timeline_key(s: &mut Workspace, ev: &KeyEvent, tx: &mut Tx) -> bool {
    let l = live(s);
    let Some(index) = l.commands.owner else {
        return false;
    };
    let query = l.commands.search.text.to_lowercase();
    let ids: Vec<_> = l.app.sessions[index]
        .conversation
        .entries
        .iter()
        .filter(|e| {
            e.kind == EntryKind::User
                && caelum_core::application::presentation::entry_visible(e)
                && caelum_core::application::presentation::entry_text(e)
                    .to_lowercase()
                    .contains(&query)
        })
        .map(|e| e.id.clone())
        .collect();
    let current = ids
        .iter()
        .position(|id| Some(id) == l.commands.timeline_selected.as_ref());
    let at = match ev.key {
        Key::Down => current.map_or(0, |i| (i + 1).min(ids.len().saturating_sub(1))),
        Key::Up => current.unwrap_or(0).saturating_sub(1),
        Key::Enter => {
            if let Some(id) = ids.get(current.unwrap_or(0)) {
                jump(s, index, id, tx);
            }
            return true;
        }
        _ => return false,
    };
    live_mut(s).commands.timeline_selected = ids.get(at).cloned();
    live_mut(s).commands.search.focused = true;
    if let Some(id) = ids.get(at) {
        tx.scroll_center_in_instant("timeline-rows", format!("timeline-{id}"));
    }
    true
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
    let mut body = ui::card(k).child(crate::activity::context(s, k, index, true));
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
        body = body.child(if busy {
            ui::skeleton("Loading OpenCode Go usage", 3, k)
        } else {
            ui::empty(
                "No usage reported",
                "Refresh to check your subscription allowances.",
                k,
            )
        });
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
fn stats(s: &Workspace, k: &Kit, index: usize, layers: &mut Vec<El>) -> El {
    let l = live(s);
    let checkout = l
        .app
        .services
        .checkouts
        .get(&l.app.sessions[index].scope.project);
    let mut ranges = row().w_full().gap(4.0).center_y();
    for (name, days) in [
        ("All history", None),
        ("7 days", Some(7)),
        ("30 days", Some(30)),
    ] {
        ranges = ranges.child(ui::tab(name, l.commands.stats_days == days, k).on_press(
            move |s: &mut Workspace| {
                live_mut(s).commands.stats_days = days;
                refresh_stats(s);
            },
        ));
    }
    let project_name = checkout
        .and_then(|c| {
            c.stats_projects
                .iter()
                .find(|p| Some(&p.id) == l.commands.stats_project.as_ref())
        })
        .and_then(|p| p.name.as_deref())
        .unwrap_or("All projects");
    let query = l.commands.search.text.to_lowercase();
    let mut projects = vec![("All projects".to_owned(), None)];
    if let Some(c) = checkout {
        projects.extend(
            c.stats_projects
                .iter()
                .filter(|p| {
                    format!("{} {}", p.name.as_deref().unwrap_or_default(), p.canonical)
                        .to_lowercase()
                        .contains(&query)
                })
                .map(|p| {
                    (
                        format!(
                            "{} · {}",
                            p.name.as_deref().unwrap_or("Project"),
                            p.canonical
                        ),
                        Some(p.id.clone()),
                    )
                }),
        );
    }
    let options: Vec<_> = projects.iter().map(|p| p.0.clone()).collect();
    let current = projects
        .iter()
        .find(|p| p.1 == l.commands.stats_project)
        .map(|p| p.0.as_str());
    ranges = ranges.child(rune::el().grow(1.0)).child(
        crate::select::Select::new("stats-project", &options)
            .current(current)
            .display(project_name)
            .label("Statistics project")
            .appearance(rune_kit::SelectAppearance::Field)
            .width(180.0)
            .open(l.commands.stats_projects_open)
            .on_toggle(|s: &mut Workspace| {
                let c = &mut live_mut(s).commands;
                c.stats_projects_open = !c.stats_projects_open;
                c.search.clear();
                c.search.focused = c.stats_projects_open;
            })
            .on_close(|s: &mut Workspace| {
                live_mut(s).commands.stats_projects_open = false;
                live_mut(s).commands.search.focused = false;
            })
            .on_choose(move |s: &mut Workspace, value, _| {
                let c = &mut live_mut(s).commands;
                c.stats_project = projects
                    .iter()
                    .find(|p| p.0 == value)
                    .and_then(|p| p.1.clone());
                c.stats_projects_open = false;
                c.search.focused = false;
                refresh_stats(s);
            })
            .search(
                ui::field(
                    &l.commands.search,
                    "Find statistics project",
                    "Search projects…",
                    k,
                )
                .edit_menu("stats-project-search", |s| {
                    Some(&mut live_mut(s).commands.search)
                }),
            )
            .build(k, layers),
    );
    let mut body = col().w_full().gap(16.0).child(ranges);
    if let Some(stats) = checkout.and_then(|c| c.stats.as_ref()) {
        let mut totals = row().w_full().gap(8.0);
        for (name, value) in [
            ("Sessions", stats.sessions.to_string()),
            ("Prompts", stats.prompts.to_string()),
            ("Total cost", format!("${:.2}", stats.cost)),
        ] {
            totals = totals.child(
                ui::card(k)
                    .grow(1.0)
                    .min_w(0.0)
                    .gap(8.0)
                    .child(ui::caption(name, k))
                    .child(
                        text(value)
                            .font(rune::TextStyle::new(26.0, 550.0))
                            .color(k.c.text_base),
                    ),
            );
        }
        body = body.child(totals);
        let mut activity = ui::card(k).gap(12.0).child(
            row()
                .w_full()
                .center_y()
                .justify_between()
                .child(text("Activity").font(k.ty.ui_medium).color(k.c.text_base))
                .child(ui::caption(
                    format!(
                        "{} active days · {} day streak",
                        stats.active_days, stats.streak
                    ),
                    k,
                )),
        );
        if stats.activity.is_empty() {
            activity = activity.child(
                ui::caption("No activity in this period. Try a wider date range.", k).py(20.0),
            );
        } else {
            let days = &stats.activity[stats.activity.len().saturating_sub(60)..];
            let peak = days.iter().map(|d| d.steps).max().unwrap_or(1).max(1) as f32;
            let mut bars = row().w_full().h(92.0).gap(3.0);
            for day in days {
                let description = format!("{} · {} steps", day.date, day.steps);
                bars = bars.child(
                    col()
                        .grow(1.0)
                        .min_w(0.0)
                        .h_full()
                        .justify_end()
                        .child(
                            rune::el()
                                .w_full()
                                .h((day.steps as f32 / peak * 88.0).max(3.0))
                                .radius(3.0)
                                .fill(k.alpha(
                                    k.c.text_accent,
                                    if day.steps == 0 { 0.12 } else { 0.65 },
                                )),
                        )
                        .role("img")
                        .label(&description)
                        .help(description)
                        .focusable(true),
                );
            }
            activity = activity.child(bars).child(
                row()
                    .w_full()
                    .justify_between()
                    .child(ui::caption(&days[0].date, k))
                    .child(ui::caption(format!("{} steps", stats.steps), k))
                    .child(ui::caption(&days[days.len() - 1].date, k)),
            );
        }
        body = body.child(activity);
        let mut tokens = ui::card(k).gap(10.0).child(
            text("Token breakdown")
                .font(k.ty.ui_medium)
                .color(k.c.text_base),
        );
        let total = (stats.tokens.input
            + stats.tokens.output
            + stats.tokens.reasoning
            + stats.tokens.cache.read
            + stats.tokens.cache.write)
            .max(1.0);
        for (name, n) in [
            ("Input", stats.tokens.input),
            ("Output", stats.tokens.output),
            ("Reasoning", stats.tokens.reasoning),
            ("Cache read", stats.tokens.cache.read),
            ("Cache write", stats.tokens.cache.write),
        ] {
            tokens = tokens.child(
                row()
                    .w_full()
                    .gap(12.0)
                    .center_y()
                    .child(ui::caption(name, k).w(82.0))
                    .child(
                        row().grow(1.0).h(5.0).radius(3.0).fill(k.c.layer_03).child(
                            rune::el()
                                .w_frac((n / total).clamp(0.0, 1.0) as f32)
                                .h_full()
                                .radius(3.0)
                                .fill(k.alpha(k.c.text_accent, 0.65)),
                        ),
                    )
                    .child(
                        row()
                            .w(68.0)
                            .justify_end()
                            .child(text(compact(n)).font(k.ty.mono).color(k.c.text_base)),
                    )
                    .help(format!("{name}: {n:.0} tokens")),
            );
        }
        body = body.child(tokens);
        let mut models = ui::card(k).gap(10.0).child(
            row()
                .w_full()
                .gap(16.0)
                .child(
                    text("Models")
                        .font(k.ty.ui_medium)
                        .color(k.c.text_base)
                        .grow(1.0),
                )
                .child(ui::caption("Steps", k).w(54.0))
                .child(ui::caption("Cost", k).w(60.0)),
        );
        for model in &stats.models {
            models = models.child(ui::divider(k)).child(
                row()
                    .w_full()
                    .gap(16.0)
                    .center_y()
                    .child(
                        col()
                            .grow(1.0)
                            .min_w(0.0)
                            .gap(3.0)
                            .child(
                                text(&model.model.id)
                                    .font(k.ty.ui_medium)
                                    .color(k.c.text_base)
                                    .truncate(),
                            )
                            .child(ui::caption(&model.model.provider_id, k)),
                    )
                    .child(
                        text(model.steps.to_string())
                            .font(k.ty.mono)
                            .color(k.c.text_muted)
                            .w(54.0),
                    )
                    .child(
                        text(format!("${:.2}", model.cost))
                            .font(k.ty.mono)
                            .color(k.c.text_base)
                            .w(60.0),
                    ),
            );
        }
        if stats.models.is_empty() {
            models = models.child(ui::caption("No model usage reported in this period.", k));
        }
        body = body.child(models);
    }
    if checkout.is_none_or(|c| c.stats.is_none()) && l.app.services.busy(&stats_task(s).unwrap()) {
        body = body
            .child(ui::card(k).child(ui::skeleton("Loading summary", 2, k)))
            .child(ui::card(k).child(ui::skeleton("Loading activity", 3, k)));
    } else if checkout.is_none_or(|c| c.stats.is_none())
        && l.app.services.error(&stats_task(s).unwrap()).is_none()
    {
        body = body.child(ui::empty(
            "No statistics yet",
            "Recorded sessions, tokens and model usage will appear here.",
            k,
        ));
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
        body = body.child(ui::notice(
            "Couldn’t load results",
            &format!(
                "{error}{}",
                if cached {
                    " · Showing the last reported result."
                } else {
                    ""
                }
            ),
            k,
        ));
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
        .key("command-results")
        .w_full()
        .h_full()
        .min_h(0.0)
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .child(body)
        .scroll_bar(k)
}

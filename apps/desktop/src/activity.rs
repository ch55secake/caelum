use crate::ui::Button;
use crate::{
    Workspace,
    live::{live, live_mut},
    panels, quiet_button, theme, ui,
};
use caelum_core::application::{
    Entry, EntryKind,
    presentation::{Outcome, Tool, now},
};
use rune::{El, col, icon, row, text};
use rune_kit::{ButtonVariant, Kit, Markdown, ProgressCircle, glyphs};

pub fn shells(s: &Workspace, k: &Kit, index: usize) -> El {
    use caelum_core::application::Task;
    let l = live(s);
    let session = &l.app.sessions[index];
    if session.shells.is_empty() {
        return col();
    }
    let expanded = l.expanded.contains(&(index, "background-shells".into()));
    let mut body = col().w_full().gap(6.0).child(
        ui::tab(
            &format!(
                "{} background command{} {}",
                session.shells.len(),
                if session.shells.len() == 1 { "" } else { "s" },
                if expanded { "⌃" } else { "⌄" }
            ),
            expanded,
            k,
        )
        .key("background-shells")
        .expanded(expanded)
        .w_full()
        .on_press(move |s: &mut Workspace| {
            let l = live_mut(s);
            if !l.expanded.remove(&(index, "background-shells".into())) {
                l.expanded.insert((index, "background-shells".into()));
                let session = &l.app.sessions[index];
                let tasks: Vec<_> = session
                    .shells
                    .iter()
                    .filter_map(|shell| {
                        Some(Task::Shell {
                            project: session.scope.project.clone(),
                            session: session.remote_id.clone()?,
                            id: shell.id.clone(),
                            stop: false,
                        })
                    })
                    .collect();
                for task in tasks {
                    l.app.task(task);
                }
            }
        }),
    );
    if expanded {
        let mut list = col().w_full().gap(8.0);
        for shell in &session.shells {
            let Some(remote) = &session.remote_id else {
                continue;
            };
            let task = Task::Shell {
                project: session.scope.project.clone(),
                session: remote.clone(),
                id: shell.id.clone(),
                stop: true,
            };
            let read = Task::Shell {
                project: session.scope.project.clone(),
                session: remote.clone(),
                id: shell.id.clone(),
                stop: false,
            };
            let stop = task.clone();
            let refresh = read.clone();
            let mut command = ui::card(k).padding(8.0).gap(4.0).child(
                row()
                    .w_full()
                    .center_y()
                    .gap(6.0)
                    .child(ui::outcome(Outcome::Running, k))
                    .child(
                        text(&shell.command)
                            .font(k.ty.mono)
                            .color(k.c.text_base)
                            .truncate()
                            .grow(1.0)
                            .min_w(0.0)
                            .help(&shell.command),
                    )
                    .child(ui::caption(
                        caelum_core::application::presentation::duration(
                            now().saturating_sub(shell.started_at),
                        ),
                        k,
                    ))
                    .child(
                        quiet_button(&ui::REFRESH, "Refresh command output")
                            .disabled(l.app.services.busy(&read))
                            .on_press(move |s: &mut Workspace| {
                                live_mut(s).app.task(refresh.clone())
                            })
                            .build(k),
                    )
                    .child(
                        Button::new("Stop")
                            .variant(ButtonVariant::GhostMuted)
                            .disabled(l.app.services.busy(&task))
                            .on_press(move |s: &mut Workspace| live_mut(s).app.task(stop.clone()))
                            .build(k)
                            .key(format!("stop-shell-{}", shell.id)),
                    ),
            );
            if let Some(output) = l
                .app
                .services
                .checkouts
                .get(&session.scope.project)
                .and_then(|c| c.shell_output.get(&(remote.clone(), shell.id.clone())))
            {
                command = command.child(
                    text(if output.is_empty() {
                        "Waiting for output…"
                    } else {
                        output
                    })
                    .font(k.ty.mono)
                    .color(k.c.text_muted)
                    .wrap()
                    .selectable(true),
                );
            }
            if let Some(error) = l
                .app
                .services
                .error(&task)
                .or_else(|| l.app.services.error(&read))
            {
                command = command.child(ui::caption(error, k).color(k.c.danger.fg));
            }
            list = list.child(command);
        }
        body = body.child(
            col()
                .w_full()
                .max_h(140.0)
                .scroll(rune::ScrollSpec {
                    stick_bottom: false,
                })
                .child(list),
        );
    }
    body
}

pub fn tool(s: &Workspace, k: &Kit, index: usize, entry: &Entry) -> El {
    let presentation = Tool::from_entry(entry, now());
    let id = entry.id.clone();
    let expanded = live(s).expanded.contains(&(index, id.clone()));
    let mut header = row()
        .w_full()
        .gap(7.0)
        .center_y()
        .child(
            icon(
                if expanded {
                    &glyphs::CHEVRON_DOWN
                } else {
                    &glyphs::CARET_RIGHT
                },
                12.0,
            )
            .color(k.c.icon_muted),
        )
        .child(ui::outcome(presentation.outcome, k))
        .child(
            text(&presentation.name)
                .font(k.ty.small_medium)
                .color(k.c.text_muted),
        )
        .child(
            text(&presentation.summary)
                .font(if presentation.name == "Run" {
                    k.ty.mono
                } else {
                    k.ty.small
                })
                .color(k.c.text_muted)
                .truncate()
                .grow(1.0)
                .min_w(0.0),
        );
    let state = match presentation.outcome {
        Outcome::Running => "Running",
        Outcome::Failure => "Failed",
        Outcome::Interrupted => "Interrupted",
        _ => "",
    };
    header = header.child(ui::caption(
        format!("{state} {}", presentation.timing).trim(),
        k,
    ));
    let mut content = col().w_full().gap(6.0).child(
        k.interactive(header.py(7.0).px(6.0).radius(7.0))
            .role("button")
            .label(format!("{} {}", presentation.name, presentation.summary))
            .expanded(expanded)
            .focusable(true)
            .focus_ring(1.5, 0.0, k.c.border_focus)
            .on_press(move |s: &mut Workspace| {
                let set = &mut live_mut(s).expanded;
                if !set.remove(&(index, id.clone())) {
                    set.insert((index, id.clone()));
                }
            })
            .key(format!("tool-{index}-{}", entry.id)),
    );
    if !presentation.details.is_empty() {
        content = content.child(ui::caption(presentation.details.join(" · "), k).pl(27.0));
    }
    if !expanded
        && presentation.outcome == Outcome::Running
        && presentation.name == "Run"
        && !entry.output.is_empty()
    {
        let recent = entry
            .output
            .lines()
            .rev()
            .take(3)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n");
        content = content.child(
            text(recent)
                .font(k.ty.mono)
                .color(k.c.text_muted)
                .wrap()
                .pl(27.0),
        );
    }
    if !presentation.patch.is_empty() {
        let owner = (index, entry.id.clone());
        if expanded && live(s).patches.contains_key(&owner) {
            let edit = owner.clone();
            let keyboard = owner.clone();
            let copy = presentation.patch.clone();
            let key = crate::code_view::source_key(&index.to_string(), &entry.id);
            let height = live(s).patches[&owner]
                .diff
                .files()
                .iter()
                .map(|f| {
                    44.0 + f
                        .hunks
                        .iter()
                        .map(|h| 24.0 + h.lines.len() as f32 * 22.0)
                        .sum::<f32>()
                })
                .sum::<f32>()
                .clamp(90.0, 300.0);
            content = content.child(
                col()
                    .w_full()
                    .h(height)
                    .min_h(0.0)
                    .radius(8.0)
                    .clip_rounded(8.0)
                    .border(1.0, k.c.border_base)
                    .child(
                        rune_kit::diff::DiffView::new(
                            move |s: &Workspace| &live(s).patches[&owner].diff,
                            move |s: &mut Workspace| {
                                &mut s
                                    .live
                                    .as_mut()
                                    .unwrap()
                                    .patches
                                    .get_mut(&edit)
                                    .unwrap()
                                    .diff
                            },
                        )
                        .key(format!("tool-{key}"))
                        .header_actions({
                            let kit = k.clone();
                            move |_, _| {
                                let copy = copy.clone();
                                quiet_button(&glyphs::COPY, "Copy patch")
                                    .on_press_tx(move |_: &mut Workspace, tx| tx.copy(copy.clone()))
                                    .build(&kit)
                            }
                        })
                        .build(s, k)
                        .focusable(true)
                        .on_key(move |s: &mut Workspace, ev, tx| {
                            crate::code_view::key(
                                &mut live_mut(s).patches.get_mut(&keyboard).unwrap().diff,
                                ev,
                                tx,
                            )
                        }),
                    ),
            );
        } else if !expanded {
            let added = presentation
                .patch
                .lines()
                .filter(|l| l.starts_with('+') && !l.starts_with("+++"))
                .count();
            let removed = presentation
                .patch
                .lines()
                .filter(|l| l.starts_with('-') && !l.starts_with("---"))
                .count();
            content = content.child(
                row()
                    .gap(8.0)
                    .pl(27.0)
                    .center_y()
                    .child(icon(&ui::FILE, 13.0).color(k.c.icon_muted))
                    .child(
                        ui::caption(
                            format!("{} · +{added} −{removed}", presentation.paths.join(", ")),
                            k,
                        )
                        .truncate()
                        .grow(1.0),
                    ),
            );
        }
    }
    if expanded {
        if matches!(entry.kind, EntryKind::Reasoning | EntryKind::Compaction) {
            content = content.child(
                Markdown::new(&entry.text)
                    .selectable(true)
                    .render(&theme::markdown(k))
                    .pl(27.0),
            );
        } else {
            let mut paths = row().gap(6.0);
            let has_paths = !presentation.paths.is_empty();
            for path in presentation.paths {
                paths = paths.child(
                    Button::new(&path)
                        .variant(ButtonVariant::GhostMuted)
                        .on_press_tx(move |s: &mut Workspace, tx| {
                            panels::open_link(s, index, &path, tx)
                        })
                        .build(k),
                );
            }
            if has_paths {
                content = content.child(col().w_full().scroll_x().child(paths.self_start()));
            }
            for (name, value) in [
                ("Input", &presentation.input),
                ("Output", &entry.output),
                ("Error", &entry.error),
            ] {
                if value.is_empty() {
                    continue;
                }
                let copy = value.clone();
                let full_id = format!("{}:{name}", entry.id);
                let full = live(s).expanded.contains(&(index, full_id.clone()));
                let displayed = if full {
                    value.clone()
                } else {
                    value.chars().take(12000).collect()
                };
                let mut section = col()
                    .w_full()
                    .gap(6.0)
                    .child(
                        row()
                            .w_full()
                            .center_y()
                            .child(ui::caption(name, k).grow(1.0))
                            .child(
                                quiet_button(
                                    &glyphs::COPY,
                                    &format!("Copy {}", name.to_lowercase()),
                                )
                                .on_press_tx(move |_: &mut Workspace, tx| tx.copy(copy.clone()))
                                .build(k),
                            ),
                    )
                    .child(
                        col()
                            .w_full()
                            .max_h(260.0)
                            .scroll(rune::ScrollSpec {
                                stick_bottom: false,
                            })
                            .child(
                                text(displayed)
                                    .font(k.ty.mono)
                                    .color(if name == "Error" {
                                        k.c.danger.fg
                                    } else {
                                        k.c.text_muted
                                    })
                                    .wrap()
                                    .selectable(true)
                                    .padding(10.0)
                                    .radius(6.0)
                                    .fill(k.c.layer_01),
                            ),
                    );
                if value.chars().count() > 12000 {
                    section = section.child(
                        Button::new(if full {
                            "Show less"
                        } else {
                            "Show full output"
                        })
                        .variant(ButtonVariant::GhostMuted)
                        .on_press(move |s: &mut Workspace| {
                            let set = &mut live_mut(s).expanded;
                            if !set.remove(&(index, full_id.clone())) {
                                set.insert((index, full_id.clone()));
                            }
                        })
                        .build(k),
                    );
                }
                content = content.child(section.pl(27.0));
            }
        }
    }
    content
}

pub fn context(s: &Workspace, k: &Kit, index: usize, wide: bool) -> El {
    let session = &live(s).app.sessions[index];
    let usage = &session.conversation;
    let model = live(s).app.choices.get(&session.scope).and_then(|c| {
        c.models
            .iter()
            .find(|m| Some(&m.id) == usage.usage_model.as_ref())
    });
    let limit = usage
        .context_window
        .or_else(|| {
            if session.scope.provider == caelum_core::application::Provider::OpenCode {
                model.and_then(|m| m.context_window)
            } else {
                None
            }
        })
        .filter(|n| *n > 0);
    let percent = usage
        .used_tokens
        .zip(limit)
        .map(|(used, max)| (used as f64 / max as f64) * 100.0);
    let summary = match (percent, usage.used_tokens) {
        (Some(p), _) => format!("Context {p:.0}%"),
        (_, Some(n)) => format!("Context {n}"),
        _ => "Context —".into(),
    };
    let details = match usage.used_tokens {
        Some(n) => format!(
            "Context window\n{n}{} tokens\n{}\nLatest reported model request, including cached input and output.",
            limit.map(|l| format!(" / {l}")).unwrap_or_default(),
            model
                .map(|m| m.name.as_str())
                .unwrap_or("Model capacity unavailable")
        ),
        None => "Context usage has not been reported by this provider yet.".into(),
    };
    let mut meter = row().gap(5.0).center_y().child(
        ProgressCircle::new(percent.unwrap_or(0.0).min(100.0) as f32 / 100.0)
            .label("Context usage")
            .build(k),
    );
    if wide {
        meter = meter.child(ui::caption(summary.clone(), k));
    }
    meter
        .key("context-usage")
        .label(summary)
        .description(&details)
        .help(details)
        .focusable(true)
}

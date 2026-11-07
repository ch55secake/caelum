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
use rune_kit::{Button, ButtonVariant, Kit, Markdown, ProgressCircle, glyphs};

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
        k.interactive(header.py(5.0))
            .role("button")
            .label(format!("{} {}", presentation.name, presentation.summary))
            .expanded(expanded)
            .focusable(true)
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
        let full_id = format!("{}:patch", entry.id);
        let full = expanded && live(s).expanded.contains(&(index, full_id.clone()));
        let mut patch = col()
            .w_full()
            .gap(0.0)
            .padding(8.0)
            .radius(6.0)
            .fill(k.c.layer_01);
        for line in presentation.patch.lines().take(if full {
            usize::MAX
        } else if expanded {
            80
        } else {
            6
        }) {
            patch = patch.child(
                text(line)
                    .font(k.ty.mono)
                    .color(if line.starts_with('+') {
                        k.c.success.fg
                    } else if line.starts_with('-') {
                        k.c.danger.fg
                    } else {
                        k.c.text_muted
                    })
                    .selectable(true)
                    .truncate(),
            );
        }
        if expanded {
            let copy = presentation.patch.clone();
            let mut actions = row().gap(6.0).child(
                quiet_button(&glyphs::COPY, "Copy patch")
                    .on_press_tx(move |_: &mut Workspace, tx| tx.copy(copy.clone()))
                    .build(k),
            );
            if presentation.patch.lines().count() > 80 {
                actions = actions.child(
                    Button::new(if full { "Show less" } else { "Show full patch" })
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
            patch = patch.child(actions);
        }
        content = content.child(patch);
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
            let mut paths = row().w_full().scroll_x().gap(6.0);
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
            content = content.child(paths);
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

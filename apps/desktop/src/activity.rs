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

pub fn session_activity(
    s: &Workspace,
    index: usize,
) -> Option<caelum_core::application::presentation::Activity> {
    let app = &live(s).app;
    let session = &app.sessions[index];
    let input = app.approvals.iter().any(|p| {
        p.project_path == session.scope.project
            && p.provider == session.scope.provider.as_str()
            && (p.session_id.is_none() || p.session_id == session.remote_id)
    }) || app.questions.iter().any(|p| {
        p.project_path == session.scope.project
            && p.provider == session.scope.provider.as_str()
            && Some(&p.session_id) == session.remote_id.as_ref()
    });
    caelum_core::application::presentation::activity(session, input)
}

pub fn indicator(
    activity: &caelum_core::application::presentation::Activity,
    agent: Option<&str>,
    k: &Kit,
) -> El {
    use caelum_core::application::presentation::ActivityKind;
    static RING: rune::Icon = rune::Icon {
        name: "active-agent",
        view: 16.0,
        body: r#"<path d="M8 2a6 6 0 1 1-6 6" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>"#,
    };
    let waiting = matches!(
        activity.kind,
        ActivityKind::Input | ActivityKind::Background
    );
    let color = ui::activity_color(activity, k);
    let glyph = if waiting {
        rune::el().size(7.0, 7.0).radius(4.0).fill(color)
    } else {
        icon(&RING, 14.0)
            .color(color)
            .looping(rune::LoopSpec::rotation(1.6))
    };
    row()
        .key("session-activity")
        .role("status")
        .label(&activity.label)
        .w_full()
        .min_h(24.0)
        .pb(6.0)
        .gap(8.0)
        .center_y()
        .child(glyph)
        .child(text(&activity.label).font(k.ty.small_medium).color(color))
        .child(
            ui::caption(agent.unwrap_or("Agent"), k)
                .grow(1.0)
                .truncate(),
        )
}

pub fn shells(s: &Workspace, k: &Kit, index: usize) -> El {
    crate::shells::view(s, k, index)
}

pub fn tool(s: &Workspace, k: &Kit, index: usize, entry: &Entry) -> El {
    let presentation = Tool::from_entry(entry, now());
    let id = entry.id.clone();
    let expanded = live(s).expanded.contains(&(index, id.clone()));
    let patch = !presentation.patch.is_empty();
    let preview = patch && !live(s).collapsed_patches.contains(&(index, id.clone()));
    let open = if patch { preview } else { expanded };
    let summary = if patch {
        let count = live(s)
            .patches
            .get(&(index, id.clone()))
            .map_or(presentation.paths.len().max(1), |p| p.diff.files().len());
        format!("{count} file{}", if count == 1 { "" } else { "s" })
    } else {
        presentation.summary.clone()
    };
    let glyph = match presentation.name.as_str() {
        "Run" => &ui::TERMINAL,
        "Read" => &ui::FILE,
        "Search" => &glyphs::SEARCH,
        "Patch" => &ui::EDIT,
        "Thinking" | "Thought" | "Context compaction" => &crate::ORBIT,
        _ => &ui::TOOL,
    };
    let mut header = row()
        .w_full()
        .gap(7.0)
        .center_y()
        .child(
            icon(glyph, 15.0)
                .color(k.c.icon_muted)
                .key(format!("tool-icon-{index}-{}", entry.id)),
        )
        .child(
            text(&presentation.name)
                .font(k.ty.small_medium)
                .color(k.c.text_base),
        )
        .child(
            text(summary)
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
    header = header
        .child(ui::outcome(presentation.outcome, k))
        .child(
            ui::caption(format!("{state} {}", presentation.timing).trim(), k).color(
                match presentation.outcome {
                    Outcome::Failure => k.c.danger.fg,
                    Outcome::Running | Outcome::Interrupted => k.c.warning.fg,
                    _ => k.c.text_muted,
                },
            ),
        )
        .child(
            ui::caption(
                if open {
                    "Hide"
                } else if patch {
                    "Show patch"
                } else {
                    "Details"
                },
                k,
            )
            .pl(6.0),
        );
    let mut heading = row().w_full().center_y().gap(4.0).child(
        k.interactive(header.py(7.0).px(6.0).radius(7.0))
            .grow(1.0)
            .min_w(0.0)
            .role("button")
            .label(format!("{} {}", presentation.name, presentation.summary))
            .expanded(open)
            .focusable(true)
            .focus_ring(1.5, 0.0, k.c.border_focus)
            .on_press(move |s: &mut Workspace| {
                let set = if patch {
                    &mut live_mut(s).collapsed_patches
                } else {
                    &mut live_mut(s).expanded
                };
                if !set.remove(&(index, id.clone())) {
                    set.insert((index, id.clone()));
                }
            })
            .key(format!("tool-{index}-{}", entry.id)),
    );
    if patch {
        let details_key = entry.id.clone();
        heading = heading.child(
            quiet_button(
                &ui::MORE,
                if expanded {
                    "Hide tool details"
                } else {
                    "Show tool details"
                },
            )
            .on_press(move |s: &mut Workspace| {
                let set = &mut live_mut(s).expanded;
                if !set.remove(&(index, details_key.clone())) {
                    set.insert((index, details_key.clone()));
                }
            })
            .build(k)
            .key(format!("tool-details-{index}-{}", entry.id)),
        );
    }
    let mut content = col().w_full().gap(6.0).child(heading);
    if !presentation.details.is_empty() {
        content = content.child(ui::caption(presentation.details.join(" · "), k).pl(27.0));
    }
    if !expanded
        && presentation.outcome == Outcome::Running
        && presentation.name == "Run"
        && !entry.output.is_empty()
    {
        let recent = presentation
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
            col().w_full().max_h(54.0).clip_rounded(0.0).child(
                text(recent)
                    .font(k.ty.mono)
                    .color(k.c.text_muted)
                    .wrap()
                    .pl(27.0),
            ),
        );
    }
    if !presentation.patch.is_empty() {
        let owner = (index, entry.id.clone());
        if preview && live(s).patches.contains_key(&owner) {
            let edit = owner.clone();
            let keyboard = owner.clone();
            let copy = presentation.patch.clone();
            let key = crate::code_view::source_key(&index.to_string(), &entry.id);
            let full_key = format!("{}:patch-full", entry.id);
            let full = live(s).expanded.contains(&(index, full_key.clone()));
            let natural = crate::code_view::inline_height(&live(s).patches[&owner].diff);
            let expanded_height = rune::memo::window_layout("shell")
                .map_or(400.0, |r| (r[3] * 0.5).clamp(240.0, 480.0));
            let height = natural.min(if full { expanded_height } else { 204.0 });
            let palette = theme::inline_diff(k);
            content = content.child(
                col()
                    .w_full()
                    .h(height + 2.0)
                    .min_h(0.0)
                    .padding(1.0)
                    .key(format!("patch-preview-{index}-{}", entry.id))
                    .radius(6.0)
                    .clip_rounded(6.0)
                    .border(1.0, k.c.border_muted)
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
                            let kit = palette.clone();
                            move |_, _| {
                                let copy = copy.clone();
                                quiet_button(&glyphs::COPY, "Copy patch")
                                    .on_press_tx(move |_: &mut Workspace, tx| tx.copy(copy.clone()))
                                    .build(&kit)
                            }
                        })
                        .build(s, &palette)
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
            if natural > 204.0 {
                content = content.child(
                    row()
                        .w_full()
                        .center_y()
                        .gap(4.0)
                        .child(
                            ui::caption(
                                if full {
                                    "Expanded patch"
                                } else {
                                    "Scroll to see the full patch"
                                },
                                k,
                            )
                            .grow(1.0),
                        )
                        .child(
                            Button::new(if full {
                                "Compact preview"
                            } else {
                                "Expand preview"
                            })
                            .variant(ButtonVariant::GhostMuted)
                            .on_press(move |s: &mut Workspace| {
                                let set = &mut live_mut(s).expanded;
                                if !set.remove(&(index, full_key.clone())) {
                                    set.insert((index, full_key.clone()));
                                }
                            })
                            .build(k)
                            .key(format!("expand-patch-{index}-{}", entry.id)),
                        ),
                );
            }
        } else if !preview {
            let paths = live(s).patches.get(&owner).map_or_else(
                || presentation.paths.join(", "),
                |document| {
                    document
                        .diff
                        .files()
                        .iter()
                        .map(|file| file.path())
                        .collect::<Vec<_>>()
                        .join(", ")
                },
            );
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
                        ui::caption(format!("{paths} · +{added} −{removed}"), k)
                            .truncate()
                            .grow(1.0),
                    ),
            );
        }
    }
    if expanded {
        if matches!(entry.kind, EntryKind::Reasoning | EntryKind::Compaction) {
            content = content.child(
                Markdown::new(caelum_core::application::presentation::entry_text(entry))
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
                ("Output", &presentation.output),
                ("Error", &presentation.error),
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

mod live;
mod state;
mod theme;

#[cfg(test)]
mod tests;

use rune::{
    AppSpec, Chrome, Cx, El, Icon, Key, KeyEvent, ScrollSpec, Spring, TextStyle, Tx, col, icon,
    row, text, text_input,
};
use rune_kit::diff::{DiffMode, DiffView};
use rune_kit::shell::{
    self, PaneMotion, RowStatus, Sidebar, SidebarHeader, SidebarRow, SidebarToggle, SplitView,
    Toolbar,
};
use rune_kit::{
    Button, ButtonVariant, ControlSize, IconButton, IconButtonVariant, Kit, Markdown, glyphs,
};
use state::Workspace;

static PLUS: Icon = Icon {
    name: "plus",
    view: 16.0,
    body: r#"<path d="M8 3v10M3 8h10" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>"#,
};
static ORBIT: Icon = Icon {
    name: "orbit",
    view: 24.0,
    body: r#"<circle cx="12" cy="12" r="3" fill="none" stroke="currentColor" stroke-width="1.5"/><ellipse cx="12" cy="12" rx="10" ry="6" transform="rotate(-45 12 12)" fill="none" stroke="currentColor" stroke-width="1.5"/><circle cx="18" cy="5" r="2" fill="currentColor"/>"#,
};
static SEND: Icon = Icon {
    name: "send",
    view: 16.0,
    body: r#"<path d="M8 13V3m-5 5 5-5 5 5" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/>"#,
};
static STOP: Icon = Icon {
    name: "stop",
    view: 16.0,
    body: r#"<rect x="4" y="4" width="8" height="8" rx="1" fill="currentColor"/>"#,
};
static APPEARANCE: Icon = Icon {
    name: "appearance",
    view: 16.0,
    body: r#"<circle cx="8" cy="8" r="5.5" fill="none" stroke="currentColor" stroke-width="1.2"/><path d="M8 2.5a5.5 5.5 0 0 1 0 11Z" fill="currentColor"/>"#,
};

fn quiet_button(glyph: &'static Icon, label: &str) -> IconButton {
    IconButton::new(glyph)
        .variant(IconButtonVariant::GhostMuted)
        .label(label)
        .control_size(ControlSize::Large)
}

fn sidebar(s: &Workspace, k: &Kit) -> El {
    if s.live.is_some() {
        return live::sidebar(s, k);
    }
    let mut sidebar = Sidebar::new()
        .label("Sessions")
        .inset(0.0)
        .row_gap(5.0)
        .header_gap(12.0)
        .header_padding(12.0, 12.0)
        .header(
            row()
                .gap(10.0)
                .center_y()
                .px(8.0)
                .pb(12.0)
                .child(icon(&ORBIT, 20.0).color(k.c.text_accent))
                .child(text("caelum").font(k.ty.body_medium).color(k.c.text_base)),
        )
        .header(
            text("Projects")
                .font(k.ty.small_medium)
                .color(k.c.text_muted)
                .px(8.0),
        )
        .header(
            col()
                .w_full()
                .padding(12.0)
                .gap(5.0)
                .radius(10.0)
                .fill(k.c.layer_02)
                .border(1.0, k.c.border_base)
                .child(text("caelum").font(k.ty.ui_medium).color(k.c.text_base))
                .child(
                    text("main  ·  1 changed file")
                        .font(k.ty.small)
                        .color(k.c.text_muted),
                ),
        )
        .header(
            Button::new("New session")
                .icon(&PLUS)
                .variant(ButtonVariant::Outline)
                .on_press(|s: &mut Workspace| s.new_session())
                .build(k)
                .key("new-session")
                .w_full(),
        )
        .header(
            SidebarHeader::new("Sessions")
                .count(s.sessions.len())
                .build(k),
        )
        .footer(
            col()
                .gap(6.0)
                .px(8.0)
                .py(8.0)
                .child(
                    text("Rune prototype")
                        .font(k.ty.small_medium)
                        .color(k.c.text_accent),
                )
                .child(
                    text("Local sample · simulated replies")
                        .font(k.ty.small)
                        .color(k.c.text_muted)
                        .wrap(),
                ),
        );
    for (i, session) in s.sessions.iter().enumerate() {
        let mut item = SidebarRow::new(session.title.clone())
            .height(52.0)
            .subtitle(if session.working() {
                "Generating sample reply…"
            } else {
                "caelum  /  main"
            })
            .selected(s.selected == i)
            .on_select(move |s: &mut Workspace, _| s.selected = i);
        if session.working() {
            item = item.status(RowStatus::Busy);
        }
        let mut item = item.build(k).key(format!("session-{i}")).radius(10.0);
        if s.selected == i {
            item = item
                .fill(k.alpha(k.c.text_accent, 0.08))
                .border(1.0, k.alpha(k.c.text_accent, 0.18));
        }
        sidebar = sidebar.child(item);
    }
    sidebar.build(k)
}

fn transcript(s: &Workspace, k: &Kit, reduced_motion: bool, padding: f32) -> El {
    if s.live.is_some() {
        return live::transcript(s, k, reduced_motion, padding);
    }
    let mut content = col().w_full().max_w(840.0).px(padding).py(28.0).gap(28.0);
    if s.active().turns.is_empty() {
        content = content.child(
            col()
                .w_full()
                .py(64.0)
                .gap(10.0)
                .items_center()
                .child(icon(&ORBIT, 32.0).color(k.c.text_accent))
                .child(
                    text("A little room to think")
                        .font(TextStyle::new(22.0, 550.0))
                        .color(k.c.text_base)
                        .wrap(),
                )
                .child(
                    text("Send a message to try the simulated stream.")
                        .font(k.ty.ui)
                        .color(k.c.text_muted)
                        .wrap(),
                ),
        );
    }
    for (i, turn) in s.active().turns.iter().enumerate() {
        let user = col()
            .w_full()
            .padding(16.0)
            .radius(10.0)
            .fill(k.c.layer_01)
            .border(1.0, k.c.border_base)
            .child(
                text(turn.prompt.clone())
                    .font(TextStyle::new(14.0, 500.0).lh(23.0))
                    .color(k.c.text_base)
                    .wrap()
                    .selectable(true),
            );
        let mut entry = col()
            .key(format!("turn-{i}"))
            .w_full()
            .gap(22.0)
            .child(user);
        if turn.tool {
            let toggle = Button::new(format!("Run {}", s.sample.command))
                .icon(if turn.expanded {
                    &glyphs::CHEVRON_DOWN
                } else {
                    &glyphs::CARET_RIGHT
                })
                .variant(ButtonVariant::GhostMuted)
                .on_press(move |s: &mut Workspace| {
                    let turn = &mut s.active_mut().turns[i];
                    turn.expanded = !turn.expanded;
                })
                .build(k)
                .key(format!("tool-{i}"))
                .expanded(turn.expanded);
            let mut tool = col().w_full().gap(8.0).child(toggle.self_start());
            if turn.expanded {
                tool = tool.child(
                    text(s.sample.output.clone())
                        .font(k.ty.mono)
                        .color(k.c.text_muted)
                        .wrap()
                        .selectable(true)
                        .padding(12.0)
                        .fill(k.c.layer_01)
                        .radius(8.0),
                );
            }
            entry = entry.child(tool);
        }
        let working = s.active().working() && i + 1 == s.active().turns.len();
        let answer = col()
            .w_full()
            .gap(10.0)
            .child(
                row()
                    .w_full()
                    .center_y()
                    .justify_between()
                    .child(
                        text(if working {
                            "Rune · generating sample…"
                        } else if turn.stopped {
                            "Rune · stopped"
                        } else {
                            "Rune · sample response"
                        })
                        .font(k.ty.small)
                        .color(k.c.text_muted),
                    )
                    .child(
                        quiet_button(&glyphs::COPY, "Copy response")
                            .on_press_tx(move |s: &mut Workspace, tx| {
                                tx.copy(s.active().turns[i].response.clone());
                            })
                            .build(k)
                            .key(format!("copy-{i}")),
                    ),
            )
            .child(
                Markdown::new(&turn.response)
                    .selectable(true)
                    .streaming(working)
                    .reveal(if reduced_motion { 0.0 } else { 0.12 })
                    .render(&theme::markdown(k))
                    .key(format!("answer-{i}")),
            );
        content = content.child(entry.child(answer));
    }
    col()
        .key(format!("transcript-{}", s.selected))
        .role("region")
        .label("Conversation")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .items_center()
        .scroll(ScrollSpec { stick_bottom: true })
        .child(content)
}

fn composer(s: &Workspace, k: &Kit, padding: f32, wide: bool) -> El {
    if s.live.is_some() {
        return live::composer(s, k, padding, wide);
    }
    let session = s.active();
    let input = text_input(&session.draft, TextStyle::new(14.0, 400.0).lh(23.0))
        .key(format!("draft-{}", s.selected))
        .label("Message")
        .placeholder("Message the sample workspace…", k.c.text_muted)
        .color(k.c.text_base)
        .selection_color(k.selection())
        .w_full()
        .min_h(56.0)
        .max_h(180.0)
        .on_edit(|s: &mut Workspace| &mut s.active_mut().draft)
        // Workspace shortcuts run before the editor's Escape-to-blur handling.
        .on_input_key(on_key)
        .on_submit(Workspace::send);
    let submit = Button::new("")
        .icon(if session.working() { &STOP } else { &SEND })
        .variant(ButtonVariant::Submit)
        .disabled(!session.working() && session.draft.text.trim().is_empty())
        .on_press_tx(|s: &mut Workspace, tx| {
            if s.active().working() {
                s.active_mut().stop();
            } else {
                s.send(tx);
            }
        })
        .build(k)
        .key("send")
        .label(if session.working() {
            "Stop generation"
        } else {
            "Send message"
        })
        .size(32.0, 32.0);
    let mut controls = row()
        .w_full()
        .center_y()
        .gap(8.0)
        .child(text("Demo model").font(k.ty.small).color(k.c.text_muted))
        .child(rune::el().grow(1.0));
    if wide {
        controls = controls.child(
            text("↵ Send · ⇧↵ Newline")
                .font(k.ty.small)
                .color(k.c.text_muted),
        );
    }
    let surface = col()
        .w_full()
        .gap(8.0)
        .padding(14.0)
        .radius(14.0)
        .fill(k.c.layer_02)
        .border(
            1.0,
            if session.draft.focused {
                k.c.border_focus
            } else {
                k.c.border_base
            },
        )
        .child(input)
        .child(controls.child(submit));
    col()
        .w_full()
        .max_w(840.0)
        .px(padding)
        .pb(24.0)
        .child(surface)
}

fn inspector(s: &Workspace, k: &Kit) -> El {
    if s.live.is_some() {
        return live::inspector(s, k);
    }
    col()
        .w_full()
        .h_full()
        .min_h(0.0)
        .gap(12.0)
        .child(
            row()
                .w_full()
                .center_y()
                .justify_between()
                .px(12.0)
                .child(text("Changes").font(k.ty.ui_medium).color(k.c.text_base))
                .child(
                    quiet_button(&glyphs::CLOSE, "Hide changes")
                        .on_press(|s: &mut Workspace| s.inspector.set_open(false))
                        .build(k)
                        .key("close-inspector"),
                ),
        )
        .child(
            text("caelum / main · sample patch")
                .font(k.ty.small)
                .color(k.c.text_muted)
                .px(12.0),
        )
        .child(
            Button::new(if s.diff.mode() == DiffMode::Unified {
                "Split diff"
            } else {
                "Unified diff"
            })
            .variant(ButtonVariant::GhostMuted)
            .on_press(|s: &mut Workspace| {
                s.diff.set_mode(if s.diff.mode() == DiffMode::Unified {
                    DiffMode::Split
                } else {
                    DiffMode::Unified
                });
            })
            .build(k)
            .key("diff-mode")
            .self_start()
            .ml(8.0),
        )
        .child(DiffView::new(|s: &Workspace| &s.diff, |s: &mut Workspace| &mut s.diff).build(s, k))
}

fn view(s: &Workspace, cx: &mut Cx<Workspace>) -> El {
    let dark = s.dark.unwrap_or(cx.system_dark());
    let k = theme::kit(dark);
    let width = cx.layout("main-pane").map_or(840.0, |r| r[2]);
    let padding = if width < 540.0 { 16.0 } else { 32.0 };
    let main = col()
        .w_full()
        .h_full()
        .min_h(0.0)
        .items_center()
        .child(transcript(s, &k, cx.is_reduced_motion(), padding))
        .child(composer(s, &k, padding, width > 560.0));
    let toolbar = Toolbar::new()
        .leading(
            text(s.title().to_owned())
                .font(k.ty.ui_medium)
                .color(k.c.text_base)
                .truncate(),
        )
        .trailing(
            quiet_button(
                &APPEARANCE,
                if dark {
                    "Use light theme"
                } else {
                    "Use dark theme"
                },
            )
            .on_press(move |s: &mut Workspace| s.dark = Some(!dark))
            .build(&k)
            .key("theme"),
        )
        .trailing(
            quiet_button(
                &shell::INSPECTOR,
                if s.live.is_some() {
                    "Session options"
                } else if s.inspector.is_open() {
                    "Hide changes"
                } else {
                    "Show changes"
                },
            )
            .expanded(s.inspector.is_open())
            .on_press(|s: &mut Workspace| s.inspector.toggle())
            .build(&k)
            .key("inspector-toggle"),
        );
    // SplitView's card styling is theme-driven. Keep the shell flush while
    // retaining rounded controls and composer surfaces inside the panes.
    let mut shell_kit = k.clone();
    shell_kit.radius.lg = 0.0;
    shell_kit.c.border_muted = k.c.bg_base;
    SplitView::new(main)
        .sidebar(
            &s.sidebar,
            |s: &mut Workspace| &mut s.sidebar,
            sidebar(s, &k),
        )
        .inspector(
            &s.inspector,
            |s: &mut Workspace| &mut s.inspector,
            inspector(s, &k),
        )
        .toolbar(toolbar)
        .titlebar_inset(cx.titlebar_inset())
        .frame_inset(0.0)
        .toolbar_in_panel()
        .sidebar_padding(12.0, 12.0)
        .sidebar_toggle(SidebarToggle::new().in_header())
        .pane_motion(PaneMotion::new().spring(Spring::new(0.22, 0.0)))
        .reduced_motion(cx.is_reduced_motion())
        .build(&shell_kit)
}

fn on_key(s: &mut Workspace, ev: &KeyEvent, tx: &mut Tx) -> bool {
    if shell::is_sidebar_toggle(ev) {
        s.sidebar.toggle();
    } else if ev.is_command('i') && ev.mods.alt {
        s.inspector.toggle();
    } else if ev.is_command('n') {
        s.new_session();
    } else if ev.is_command('o') && s.live.is_some() {
        live::open_project(s, tx);
    } else if ev.is_command('l') {
        s.focus();
    } else if ev.key == Key::Escape && s.working() {
        s.stop();
    } else {
        return false;
    }
    true
}

fn spec() -> AppSpec<Workspace> {
    AppSpec {
        min_size: Some((800.0, 620.0)),
        families: ("Inter", "ui-monospace"),
        poll: Some(state::poll),
        setup: Some(live::setup),
        on_key: Some(on_key),
        scenes: &[
            ("inspector", |s| s.inspector.set_open(true)),
            ("compact", |s| {
                s.sidebar.set_open(false);
                s.inspector.set_open(true);
                s.inspector.set_width(300.0);
            }),
        ],
        ..AppSpec::new("Caelum · Rune", (1280.0, 1000.0), Workspace::new, view)
    }
}

fn main() {
    let mut fixtures = false;
    let mut project = None;
    let mut args = Vec::new();
    let mut input = std::env::args().skip(1);
    while let Some(arg) = input.next() {
        match arg.as_str() {
            "--fixtures" => fixtures = true,
            "--project" => {
                let Some(path) = input.next() else {
                    eprintln!("--project requires a directory");
                    std::process::exit(2);
                };
                project = Some(path);
            }
            _ => args.push(arg),
        }
    }
    rune::App::new(spec())
        .init(move || {
            if fixtures {
                Workspace::new()
            } else {
                Workspace::live(project)
            }
        })
        .args(args)
        .chrome(Chrome {
            full_size_content: true,
            traffic_lights: Some((16.0, 17.0)),
            drag_regions: true,
        })
        .run();
}

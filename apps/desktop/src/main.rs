mod activity;
mod app;
mod checkouts;
mod code_view;
mod commands;
mod completion;
mod composer;
mod context;
mod evaluation;
mod home;
mod lifecycle;
mod live;
mod media;
mod merge;
mod models;
mod notifications;
mod panels;
mod questions;
mod select;
mod settings;
mod shells;
mod state;
mod theme;
mod ui;
mod worktree;

#[cfg(test)]
mod tests;

use rune::{
    Chrome, Cx, El, Icon, Key, KeyEvent, ScrollSpec, Spring, TextStyle, Tx, col, icon, row, text,
    text_input,
};
use rune_kit::diff::{DiffMode, DiffView};
use rune_kit::shell::{
    self, PaneMotion, RowStatus, Sidebar, SidebarHeader, SidebarToggle, SplitView, Toolbar,
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
    // Lucide's Orbit, as used by the original workspace. See THIRD_PARTY_NOTICES.md.
    body: r#"<path d="M20.341 6.484A10 10 0 0 1 10.266 21.85M3.659 17.516A10 10 0 0 1 13.74 2.152" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/><circle cx="12" cy="12" r="3" fill="none" stroke="currentColor" stroke-width="2"/><circle cx="19" cy="5" r="2" fill="none" stroke="currentColor" stroke-width="2"/><circle cx="5" cy="19" r="2" fill="none" stroke="currentColor" stroke-width="2"/>"#,
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
        .control_size(ControlSize::Regular)
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
        .header(ui::brand(s, k))
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
                    text("Offline fixture")
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
        let mut item = ui::session_row(
            &session.title,
            if session.working() {
                "Generating sample reply…"
            } else {
                "caelum  /  main"
            },
            s.selected == i,
            k,
        )
        .on_select(move |s: &mut Workspace, _| s.selected = i);
        if session.working() {
            item = item.status(RowStatus::Other(ui::busy_icon(&ui::CLOCK, 12.0, k)));
        }
        let item = ui::selected_row(
            item.build(k).key(format!("session-{i}")),
            s.selected == i,
            k,
        );
        sidebar = sidebar.child(item);
    }
    sidebar.build(k)
}

fn transcript(
    s: &Workspace,
    k: &Kit,
    reduced_motion: bool,
    padding: f32,
    layers: &mut Vec<El>,
) -> El {
    if s.live.is_some() {
        return live::transcript(s, k, reduced_motion, padding, layers);
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
        let user = ui::user_message(&turn.prompt, k);
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

fn composer(s: &Workspace, k: &Kit, padding: f32, wide: bool, layers: &mut Vec<El>) -> El {
    if s.live.is_some() {
        return live::composer(s, k, padding, wide, layers);
    }
    let session = s.active();
    let input = text_input(&session.draft, TextStyle::new(14.0, 400.0).lh(23.0))
        .key(format!("draft-{}", s.selected))
        .label("Message")
        .placeholder("Message the sample workspace…", k.c.text_muted)
        .color(k.c.text_base)
        .selection_color(k.selection())
        .w_full()
        .on_edit(|s: &mut Workspace| &mut s.active_mut().draft)
        // Workspace shortcuts run before the editor's Escape-to-blur handling.
        .on_input_key(on_key)
        .on_submit(Workspace::send);
    let submit = ui::Button::new("Send")
        .icon(&SEND)
        .variant(ButtonVariant::Contrast)
        .disabled(session.working() || session.draft.text.trim().is_empty())
        .on_press_tx(Workspace::send)
        .build(k)
        .key("send")
        .label("Send message");
    let mut controls = row()
        .w_full()
        .center_y()
        .gap(8.0)
        .child(text("Demo model").font(k.ty.small).color(k.c.text_muted))
        .child(rune::el().grow(1.0));
    if session.working() {
        controls = controls.child(
            ui::Button::new("Stop")
                .icon(&STOP)
                .variant(ButtonVariant::GhostMuted)
                .on_press(|s: &mut Workspace| s.active_mut().stop())
                .build(k)
                .key("stop")
                .label("Stop generation"),
        );
    } else if wide {
        controls = controls.child(
            text("↵ Send · ⇧↵ Newline")
                .font(k.ty.small)
                .color(k.c.text_muted),
        );
    }
    ui::composer(
        ui::composer_editor(input, 56.0, 180.0),
        controls.child(submit),
        rune::el(),
        padding,
        k,
    )
}

fn inspector(s: &Workspace, k: &Kit) -> El {
    if s.live.is_some() {
        return panels::inspector(s, k);
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
    let dark = s
        .live
        .as_ref()
        .map(|live| match live.app.preferences.theme.as_str() {
            "dark" => true,
            "light" => false,
            _ => cx.system_dark(),
        })
        .or(s.dark)
        .unwrap_or(cx.system_dark());
    let k = s.live.as_ref().map_or_else(
        || theme::kit(dark),
        |live| {
            theme::appearance(
                dark,
                &live.app.preferences.theme_family,
                &live.app.preferences.accent_color,
            )
        },
    );
    let vercel = s
        .live
        .as_ref()
        .is_some_and(|l| l.app.preferences.theme_family == "vercel");
    let pane_border = if vercel {
        k.alpha(k.c.text_base, 0.12)
    } else {
        k.c.border_base
    };
    let mut layers = Vec::new();
    let window_width = cx.layout("shell").map_or(1280.0, |r| r[2]);
    // Keep both panes usable at the minimum window size without overwriting the
    // user's preferred widths; they return as the window grows.
    let mut sidebar_pane = s.sidebar;
    let mut inspector_pane = s.inspector;
    inspector_pane.set_width(
        s.inspector
            .width()
            .min(window_width - s.sidebar.shown_width() - 280.0),
    );
    if inspector_pane.is_open() {
        sidebar_pane.set_width(
            s.sidebar
                .width()
                .min(window_width - inspector_pane.width() - 280.0),
        );
    }
    let width = cx.layout("main-pane").map_or(840.0, |r| r[2]);
    let padding = if width < 540.0 { 16.0 } else { 32.0 };
    let main = if home::is_home(s) {
        home::view(s, &mut layers)
    } else {
        col()
            .w_full()
            .h_full()
            .min_h(0.0)
            .items_center()
            .child(rune::el().w_full().h(1.0).fill(pane_border))
            .child(transcript(
                s,
                &k,
                cx.is_reduced_motion(),
                padding,
                &mut layers,
            ))
    };
    let main = if s.live.as_ref().is_some_and(|live| {
        matches!(
            live.panels.screen,
            panels::Screen::Home | panels::Screen::Review
        )
    }) {
        main
    } else {
        main.child(composer(s, &k, padding, width > 760.0, &mut layers))
    };
    let main = main.children((vercel && sidebar_pane.is_open()).then(|| {
        // Separate the black sidebar from the canvas with a softly fading hairline.
        rune::el()
            .key("sidebar-divider")
            .w(1.0)
            .pin(Some(0.0), Some(8.0), None, Some(12.0))
            .linear_gradient(
                std::f32::consts::PI,
                [
                    (0.0, rune::Color::TRANSPARENT),
                    (0.04, pane_border),
                    (0.96, pane_border),
                    (1.0, rune::Color::TRANSPARENT),
                ],
            )
            .a11y_hidden()
    }));
    let toolbar = Toolbar::new()
        .leading(
            row()
                .gap(10.0)
                .min_w(0.0)
                .center_y()
                .child(
                    text(
                        s.live
                            .as_ref()
                            .filter(|_| !home::is_home(s))
                            .and_then(|live| live.app.active_project.as_ref())
                            .and_then(|path| std::path::Path::new(path).file_name())
                            .and_then(|name| name.to_str())
                            .unwrap_or("caelum"),
                    )
                    .font(k.ty.ui)
                    .color(k.c.text_muted),
                )
                .child(text("/").font(k.ty.ui).color(k.c.text_faint))
                .child(
                    text(s.title().to_owned())
                        .font(k.ty.ui_medium)
                        .color(k.c.text_base)
                        .truncate()
                        .shrink(),
                ),
        )
        .trailing(
            quiet_button(&ui::SETTINGS, "Settings · ⌘,")
                .on_press(|s: &mut Workspace| {
                    if s.live.is_some() {
                        panels::show(s, panels::Screen::Settings);
                    }
                })
                .build(&k)
                .key("nav-settings"),
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
            .on_press(move |s: &mut Workspace| {
                s.dark = Some(!dark);
                if let Some(live) = &mut s.live {
                    live.app.preferences.theme = if dark { "light" } else { "dark" }.into();
                }
            })
            .build(&k)
            .key("theme"),
        )
        .trailing(
            quiet_button(
                &shell::INSPECTOR,
                if s.live.is_some() {
                    "Files and changes"
                } else if s.inspector.is_open() {
                    "Hide changes"
                } else {
                    "Show changes"
                },
            )
            .expanded(s.inspector.is_open())
            .on_press(|s: &mut Workspace| {
                if s.live.is_some() && !s.inspector.is_open() {
                    panels::show(s, panels::Screen::Files);
                } else {
                    s.inspector.toggle();
                }
            })
            .build(&k)
            .key("inspector-toggle"),
        );
    // SplitView's card styling is theme-driven. Keep the shell flush while
    // retaining rounded controls and composer surfaces inside the panes.
    let mut shell_kit = k.clone();
    shell_kit.radius.lg = 0.0;
    shell_kit.c.border_muted = k.c.bg_base;
    let shell = SplitView::new(main)
        .sidebar(
            &sidebar_pane,
            |s: &mut Workspace| &mut s.sidebar,
            sidebar(s, &k),
        )
        .inspector(
            &inspector_pane,
            |s: &mut Workspace| &mut s.inspector,
            inspector(s, &k),
        )
        .toolbar(toolbar)
        .titlebar_inset(cx.titlebar_inset())
        .frame_inset(0.0)
        .sidebar_padding(12.0, 12.0)
        .sidebar_toggle(SidebarToggle::new())
        .pane_motion(PaneMotion::new().spring(Spring::new(0.22, 0.0)))
        .reduced_motion(cx.is_reduced_motion());
    let shell = if s.sidebar.is_open() {
        shell.hide_sidebar_toggle()
    } else {
        shell
    }
    .build(&shell_kit);
    if s.live.is_some() {
        live::overlays(s, &k, &mut layers);
        panels::overlays(s, &k, &mut layers);
        models::overlay(s, &k, &mut layers);
        context::overlay(s, &k, &mut layers);
        live::delete_confirmation(s, &k, &mut layers);
    }
    col()
        .w_full()
        .h_full()
        .on_key(on_key)
        .on_secondary_press(context::workspace)
        .child(shell)
        .children(layers)
}

fn on_key(s: &mut Workspace, ev: &KeyEvent, tx: &mut Tx) -> bool {
    if context::is_open(s) {
        return context::key(s, ev);
    }
    if s.live.as_ref().is_some_and(|l| l.session_menu.is_some()) {
        if ev.key == Key::Escape {
            live::close_session_menu(s);
            return true;
        }
        return false;
    }
    if s.live.as_ref().is_some_and(|l| l.models.is_open()) {
        return models::key(s, ev, tx);
    }
    if s.live.is_some() && live::live(s).panels.overlay.is_some() {
        return panels::on_key(s, ev, tx);
    }
    if s.live.is_some()
        && (home::on_key(s, ev, tx)
            || questions::on_key(s, ev, tx)
            || live::sidebar_key(s, ev, tx)
            || completion::on_key(s, ev, tx)
            || panels::on_key(s, ev, tx)
            || commands::side_key(s, ev, tx))
    {
        return true;
    } else if shell::is_sidebar_toggle(ev) || ev.is_command('\\') {
        s.sidebar.toggle();
    } else if ev.is_command('i') && ev.mods.alt {
        s.inspector.toggle();
    } else if ev.is_command('n') && ev.mods.shift && s.live.is_some() {
        panels::open_launcher(s);
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

fn spec() -> rune::AppSpec<Workspace> {
    app::spec()
}

fn main() {
    let mut fixtures = false;
    let mut evaluation = false;
    let mut design = false;
    let mut project = None;
    let mut args = Vec::new();
    let mut input = std::env::args().skip(1);
    while let Some(arg) = input.next() {
        match arg.as_str() {
            "--version" | "-V" => {
                println!("Caelum {}", env!("CARGO_PKG_VERSION"));
                return;
            }
            "--fixtures" => fixtures = true,
            "--evaluation-workload" => evaluation = true,
            "--design-fixture" => design = true,
            "--project" => {
                let Some(path) = input.next() else {
                    eprintln!("--project requires a directory");
                    std::process::exit(2);
                };
                project = Some(path);
            }
            _ if args.is_empty() && project.is_none() && !arg.starts_with('-') => {
                project = Some(arg)
            }
            _ => args.push(arg),
        }
    }
    let app = lifecycle::configure(
        rune::App::new(spec())
            .font_smoothing(rune::FontSmoothing::Off)
            .on_frame_drawn(live::frame_drawn),
    );
    let handle = app.handle();
    let notifications =
        notifications::State::connect(handle.clone(), !fixtures && !evaluation && !design);
    app.init(move || {
        let mut state = if design {
            evaluation::design()
        } else if evaluation {
            evaluation::workspace()
        } else if fixtures {
            Workspace::new()
        } else {
            home::workspace(project)
        };
        if let Some(live) = &mut state.live {
            live.notifications = notifications;
        }
        state.handle = Some(handle);
        state
    })
    .args(args)
    .chrome(Chrome {
        full_size_content: true,
        traffic_lights: Some((16.0, 17.0)),
        drag_regions: true,
    })
    .run();
}

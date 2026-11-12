//! Caelum's shared visual vocabulary, used by live views and deterministic previews.
use rune::{El, Icon, TextStyle, col, icon, row, text};
use rune_kit::{Kit, ScrollBar, shell::SidebarRow};

/// App-sized controls, including the same keyboard and disabled treatment everywhere.
pub struct Button(rune_kit::Button, rune_kit::ButtonVariant);
impl Button {
    pub fn new(label: impl Into<String>) -> Self {
        Self(
            rune_kit::Button::new(label).control_size(rune_kit::ControlSize::Large),
            rune_kit::ButtonVariant::Neutral,
        )
    }
    pub fn variant(mut self, value: rune_kit::ButtonVariant) -> Self {
        self.0 = self.0.variant(value);
        self.1 = value;
        self
    }
    pub fn disabled(mut self, value: bool) -> Self {
        self.0 = self.0.disabled(value);
        self
    }
    pub fn icon(mut self, value: &'static Icon) -> Self {
        self.0 = self.0.icon(value);
        self
    }
    pub fn on_press<S: 'static>(mut self, f: impl Fn(&mut S) + 'static) -> Self {
        self.0 = self.0.on_press(f);
        self
    }
    pub fn on_press_tx<S: 'static>(mut self, f: impl Fn(&mut S, &mut rune::Tx) + 'static) -> Self {
        self.0 = self.0.on_press_tx(f);
        self
    }
    pub fn build(self, k: &Kit) -> El {
        use rune_kit::ButtonVariant as V;
        let mut skin = k.clone();
        skin.ty.ui = TextStyle::new(12.0, 450.0);
        skin.ty.ui_medium = TextStyle::new(12.0, 550.0);
        skin.elevation.button_neutral.clear();
        skin.elevation.button_contrast.clear();
        skin.c.bg_button_neutral = k.c.layer_02;
        skin.c.bg_contrast = k.c.bg_accent;
        skin.c.text_contrast = k.c.submit_fg;
        let border = match self.1 {
            V::Neutral | V::Outline | V::Loading => k.c.border_strong,
            V::Danger => k.alpha(k.c.danger.fg, 0.28),
            V::Warning => k.alpha(k.c.warning.fg, 0.28),
            _ => rune::Color::TRANSPARENT,
        };
        let element = self
            .0
            .build(&skin)
            .h(32.0)
            .px(12.0)
            .radius(8.0)
            .border(1.0, border)
            .focus_ring(1.5, 1.0, k.c.border_focus)
            .self_start();
        if self.1 == V::Contrast {
            element.gradient(rune::Color::TRANSPARENT, rune::Color::TRANSPARENT)
        } else {
            element
        }
    }
}

pub fn field(editor: &rune::TextEdit, label: &str, placeholder: &str, k: &Kit) -> El {
    rune::text_input(editor, k.ty.ui)
        .single_line()
        .label(label)
        .placeholder(placeholder, k.c.text_muted)
        .color(k.c.text_base)
        .selection_color(k.selection())
        .w_full()
        .min_h(34.0)
        .px(10.0)
        .py(6.0)
        .radius(7.0)
        .fill(k.c.bg_base)
        .border(
            1.0,
            if editor.focused {
                k.c.border_focus
            } else {
                k.c.border_base
            },
        )
}

pub fn card(k: &Kit) -> El {
    col()
        .w_full()
        .gap(12.0)
        .padding(14.0)
        .radius(10.0)
        .fill(k.c.layer_01)
        .border(1.0, k.c.border_base)
}

pub fn divider(k: &Kit) -> El {
    rune::el().w_full().h(1.0).fill(k.c.border_base)
}

pub fn popup<S: 'static>(
    key: &str,
    anchor: &str,
    content: El,
    placement: rune_kit::Placement,
    close: impl Fn(&mut S) + Clone + 'static,
    k: &Kit,
    layers: &mut Vec<El>,
) {
    layers.push(
        rune::el()
            .key(format!("{key}-scrim"))
            .absolute(0.0, 0.0)
            .w_full()
            .h_full()
            .on_press(close.clone()),
    );
    let content = content
        .key(key)
        .padding(6.0)
        .role("menu")
        .focus_group()
        .modal(true, close)
        .focus_return(anchor);
    layers.push(rune_kit::popover::anchored(
        k,
        rune_kit::popover::raised(k, content, 10.0),
        anchor,
        placement,
        6.0,
    ));
}

pub fn choice(title: &str, detail: &str, selected: bool, k: &Kit) -> El {
    k.interactive(
        row()
            .w_full()
            .min_h(46.0)
            .px(10.0)
            .py(8.0)
            .gap(10.0)
            .center_y()
            .radius(7.0)
            .fill(if selected {
                k.c.hover
            } else {
                rune::Color::TRANSPARENT
            })
            .child(
                col()
                    .grow(1.0)
                    .min_w(0.0)
                    .gap(2.0)
                    .child(
                        text(title)
                            .font(k.ty.ui_medium)
                            .color(k.c.text_base)
                            .truncate(),
                    )
                    .children((!detail.is_empty()).then(|| caption(detail, k).truncate())),
            )
            .child(
                icon(k.glyph.check_small, 14.0)
                    .color(k.c.text_accent)
                    .opacity(if selected { 1.0 } else { 0.0 }),
            ),
    )
    .role("button")
    .label(title)
    .description(if detail.is_empty() { title } else { detail })
    .focusable(true)
    .focus_ring(1.5, 0.0, k.c.border_focus)
}

pub fn tab(title: &str, selected: bool, k: &Kit) -> El {
    k.interactive(
        row()
            .h(30.0)
            .px(10.0)
            .center_y()
            .radius(4.0)
            .fill(if selected {
                k.c.hover
            } else {
                rune::Color::TRANSPARENT
            })
            .child(text(title).font(k.ty.ui_medium).color(if selected {
                k.c.text_accent
            } else {
                k.c.text_muted
            }))
            .children(selected.then(|| {
                rune::el().h(2.0).radius(1.0).fill(k.c.text_accent).pin(
                    Some(8.0),
                    None,
                    Some(8.0),
                    Some(0.0),
                )
            })),
    )
    .role("tab")
    .label(title)
    .focusable(true)
    .focus_selected(selected)
    .focus_ring(1.5, 0.0, k.c.border_focus)
}

pub fn notice(title: &str, detail: &str, k: &Kit) -> El {
    card(k)
        .gap(4.0)
        .child(text(title).font(k.ty.ui_medium).color(k.c.danger.fg))
        .child(caption(detail, k).selectable(true))
}

pub fn skeleton(label: &str, lines: usize, k: &Kit) -> El {
    let mut body = col()
        .w_full()
        .gap(12.0)
        .py(12.0)
        .role("status")
        .label(label);
    for i in 0..lines {
        body = body.child(
            row()
                .w_full()
                .gap(12.0)
                .child(rune::el().size(18.0, 18.0).radius(5.0).fill(k.c.layer_03))
                .child(
                    col()
                        .grow(1.0)
                        .gap(7.0)
                        .child(
                            rune::el()
                                .w_full()
                                .max_w(if i % 2 == 0 { 260.0 } else { 190.0 })
                                .h(9.0)
                                .radius(4.0)
                                .fill(k.c.layer_03),
                        )
                        .child(
                            rune::el()
                                .w_full()
                                .max_w(130.0)
                                .h(7.0)
                                .radius(3.0)
                                .fill(k.c.layer_03),
                        ),
                ),
        );
    }
    body
}

/// A moving highlight over stationary ink. The two slides share a clock and cancel
/// each other's translation; Rune holds them still under reduced motion.
fn shimmer(base: El, highlight: El, width: f32, height: f32) -> El {
    let band = (width * 0.45).clamp(10.0, 42.0);
    let slide =
        |from, to| rune::LoopSpec::slide(2.2, [from, 0.0], [to, 0.0]).easing(rune::Easing::LINEAR);
    row()
        .w(width)
        .h(height)
        .clip_rounded(0.0)
        .child(base)
        .child(
            rune::el()
                .key("shimmer-band")
                .absolute(0.0, 0.0)
                .size(band, height)
                .a11y_hidden()
                .clip_fade(band / 2.0, 0.0, band / 2.0, 0.0)
                .looping(slide(-1.0, width / band))
                .child(
                    rune::el()
                        .key("shimmer-ink")
                        .absolute(0.0, 0.0)
                        .size(band, height)
                        .looping(slide(1.0, -width / band))
                        .child(highlight.absolute(0.0, 0.0)),
                ),
        )
}

pub fn busy_text(value: &str, font: TextStyle, k: &Kit) -> El {
    let (width, height) = rune::text::measure(value, &[], font, None);
    shimmer(
        text(value).font(font).color(k.c.text_muted),
        text(value).font(font).color(k.c.text_base),
        width.ceil(),
        height.ceil(),
    )
    .label(value)
}

pub fn busy_icon(glyph: &'static Icon, size: f32, k: &Kit) -> El {
    shimmer(
        icon(glyph, size).color(k.c.icon_muted),
        icon(glyph, size).color(k.c.icon_base),
        size,
        size,
    )
    .label("Running")
    .key("status-running")
}

pub fn disclosure(open: bool, k: &Kit) -> El {
    icon(&rune_kit::glyphs::CHEVRON_DOWN, 12.0)
        .color(k.c.icon_muted)
        .rotate(if open {
            0.0
        } else {
            -std::f32::consts::FRAC_PI_2
        })
        .a11y_hidden()
}

pub fn outcome(value: caelum_core::application::presentation::Outcome, k: &Kit) -> El {
    use caelum_core::application::presentation::Outcome;
    static CHECK: Icon = Icon {
        name: "success",
        view: 16.0,
        body: r#"<path d="m3 8 3 3 7-7" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>"#,
    };
    static FAIL: Icon = Icon {
        name: "failure",
        view: 16.0,
        body: r#"<path d="m4 4 8 8m0-8-8 8" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>"#,
    };
    static MINUS: Icon = Icon {
        name: "neutral",
        view: 16.0,
        body: r#"<path d="M3 8h10" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>"#,
    };
    static HELP: Icon = Icon {
        name: "unknown",
        view: 16.0,
        body: r#"<circle cx="8" cy="8" r="6" fill="none" stroke="currentColor"/><path d="M6 6a2 2 0 1 1 3 1.7L8 9m0 2v.5" fill="none" stroke="currentColor" stroke-width="1.2"/>"#,
    };
    let (glyph, color, label) = match value {
        Outcome::Running => {
            return busy_icon(&CLOCK, 14.0, k);
        }
        Outcome::Success => (&CHECK, k.c.success.fg, "Succeeded"),
        Outcome::Failure => (&FAIL, k.c.danger.fg, "Failed"),
        Outcome::Interrupted => (&MINUS, k.c.warning.fg, "Interrupted"),
        Outcome::Neutral => (&MINUS, k.c.text_muted, "Neutral"),
        Outcome::Unknown => (&HELP, k.c.text_muted, "Unknown"),
    };
    icon(glyph, 14.0)
        .color(color)
        .label(label)
        .key(format!("status-{}", glyph.name))
}

pub static FOLDER: Icon = Icon {
    name: "folder",
    view: 16.0,
    body: r#"<path d="M2 4h4l1.5 2H14v7H2V4Z" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linejoin="round"/>"#,
};
pub static FILE: Icon = Icon {
    name: "file",
    view: 16.0,
    body: r#"<path d="M4 1.5h5l3 3v10H4zM9 1.5v3h3M6 8h4M6 10.5h4" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linejoin="round"/>"#,
};
pub static TERMINAL: Icon = Icon {
    name: "terminal",
    view: 16.0,
    body: r#"<rect x="1.5" y="2.5" width="13" height="11" rx="2" fill="none" stroke="currentColor" stroke-width="1.2"/><path d="m4 6 2 2-2 2m4 0h3" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"/>"#,
};
pub static EDIT: Icon = Icon {
    name: "edit",
    view: 16.0,
    body: r#"<path d="m9.5 2.5 4 4m-10 7 1-4 7-7a1.4 1.4 0 0 1 2 2l-7 7-4 1Z" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linejoin="round"/>"#,
};
pub static TOOL: Icon = Icon {
    name: "tool",
    view: 16.0,
    body: r#"<path d="M9 2a4 4 0 0 0-4 5l-3 4a1.8 1.8 0 0 0 3 2l4-4a4 4 0 0 0 5-4l-3 2-2-2 2-3Z" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linejoin="round"/>"#,
};
pub static BRAIN: Icon = Icon {
    name: "brain",
    view: 24.0,
    // Lucide's Brain. See THIRD_PARTY_NOTICES.md.
    body: r#"<path d="M12 18V5M15 13a4.17 4.17 0 0 1-3-4 4.17 4.17 0 0 1-3 4M17.707 6.294A3 3 0 1 0 12 5a3 3 0 1 0-5.707 1.294M17.996 5.125a4 4 0 0 1 2.526 5.77M18 18a4 4 0 0 0 2-7.465M19.967 17.482a4 4 0 1 1-7.967.517 4 4 0 1 1-7.967-.517M6 18a4 4 0 0 1-2-7.465M6.004 5.125a4 4 0 0 0-2.526 5.77" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>"#,
};
pub static CLOCK: Icon = Icon {
    name: "clock",
    view: 16.0,
    body: r#"<circle cx="8" cy="8" r="6" fill="none" stroke="currentColor" stroke-width="1.2"/><path d="M8 4.5V8l2.5 1.5" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"/>"#,
};
pub static BRANCH: Icon = Icon {
    name: "branch",
    view: 16.0,
    body: r#"<circle cx="4" cy="3" r="1.7" fill="none" stroke="currentColor"/><circle cx="4" cy="13" r="1.7" fill="none" stroke="currentColor"/><circle cx="12" cy="4" r="1.7" fill="none" stroke="currentColor"/><path d="M4 5v6m0-2c6 0 8-1 8-3" fill="none" stroke="currentColor" stroke-width="1.2"/>"#,
};
pub static SETTINGS: Icon = Icon {
    name: "settings",
    view: 16.0,
    body: r#"<path d="m6 2 .5-1h3L10 3l2 1 1.5-.1 1.5 2.6L14 8l.5 1.5-1.5 2.6-2-.1-1 1-.5 2h-3L6 13l-2-1-1.5.1L1 9.5 2 8l-.5-1.5L3 3.9l2 .1 1-1Z" fill="none" stroke="currentColor" stroke-width="1"/><circle cx="8" cy="8" r="2.3" fill="none" stroke="currentColor"/>"#,
};
pub static BELL: Icon = Icon {
    name: "notifications",
    view: 16.0,
    body: r#"<path d="M12 6a4 4 0 0 0-8 0v3l-1.5 2h11L12 9V6ZM6.5 13a1.5 1.5 0 0 0 3 0" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"/>"#,
};
pub static REFRESH: Icon = Icon {
    name: "refresh",
    view: 16.0,
    body: r#"<path d="M13 6a5 5 0 0 0-8-3L2 6m0-4v4h4M3 10a5 5 0 0 0 8 3l3-3m0 4v-4h-4" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"/>"#,
};
pub static CHOICE: Icon = Icon {
    name: "choice",
    view: 16.0,
    body: r#"<path d="m5 6 3-3 3 3m-6 4 3 3 3-3" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"/>"#,
};
pub static EFFORT: Icon = Icon {
    name: "reasoning-effort",
    view: 16.0,
    body: r#"<path d="M3 12.5a6 6 0 1 1 10 0M8 9l3-4M4 6l1 1M8 3v1M12 6l-1 1" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"/><circle cx="8" cy="9" r="1" fill="currentColor"/>"#,
};
pub static AGENT: Icon = Icon {
    name: "agent",
    view: 16.0,
    body: r#"<rect x="3" y="5" width="10" height="8" rx="2" fill="none" stroke="currentColor" stroke-width="1.2"/><path d="M8 2v3M1 8v2M15 8v2M6 11h4" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/><circle cx="6" cy="8" r=".7" fill="currentColor"/><circle cx="10" cy="8" r=".7" fill="currentColor"/>"#,
};
pub static QUEUE: Icon = Icon {
    name: "queue",
    view: 16.0,
    body: r#"<path d="M2 4h9M2 8h6M2 12h6m4-5v6m-3-3h6" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>"#,
};
pub static EXTERNAL: Icon = Icon {
    name: "external",
    view: 16.0,
    body: r#"<path d="M9 2h5v5m0-5L7 9M6 3H3v10h10v-3" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"/>"#,
};
pub static ARCHIVE: Icon = Icon {
    name: "archive",
    view: 16.0,
    body: r#"<path d="M2 2h12v3H2zM3 5v9h10V5M6 8h4" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linejoin="round"/>"#,
};
pub static MORE: Icon = Icon {
    name: "more",
    view: 16.0,
    body: r#"<circle cx="3" cy="8" r="1" fill="currentColor"/><circle cx="8" cy="8" r="1" fill="currentColor"/><circle cx="13" cy="8" r="1" fill="currentColor"/>"#,
};
pub static BACK: Icon = Icon {
    name: "back",
    view: 16.0,
    body: r#"<path d="m10 3-5 5 5 5" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

pub fn provider_name(provider: caelum_core::application::Provider) -> &'static str {
    match provider {
        caelum_core::application::Provider::Codex => "Codex",
        caelum_core::application::Provider::OpenCode => "OpenCode",
    }
}

pub fn brand(s: &crate::Workspace, k: &Kit) -> El {
    let mut brand = row()
        .key("brand")
        .w_full()
        .h(28.0)
        .mb(8.0)
        .gap(10.0)
        .center_y()
        .px(8.0)
        .child(icon(&crate::ORBIT, 18.0).color(k.c.text_accent))
        .child(
            text("caelum")
                .font(TextStyle::new(15.0, 600.0))
                .color(k.c.text_base)
                .grow(1.0),
        );
    if s.sidebar.is_open() {
        brand = brand.child(
            crate::quiet_button(&rune_kit::shell::SIDEBAR, "Hide sidebar")
                .on_press(|s: &mut crate::Workspace| s.sidebar.set_open(false))
                .build(k)
                .expanded(true)
                .key("sidebar-toggle"),
        );
    }
    brand
}

pub fn caption(value: impl Into<String>, k: &Kit) -> El {
    text(value.into())
        .font(k.ty.small)
        .color(k.c.text_muted)
        .wrap()
}

pub fn section(title: &str, description: &str, k: &Kit) -> El {
    col()
        .w_full()
        .gap(6.0)
        .pb(12.0)
        .child(
            text(title)
                .font(TextStyle::new(16.0, 600.0))
                .color(k.c.text_base)
                .wrap(),
        )
        .child(caption(description, k))
}

pub fn session_row(title: &str, subtitle: &str, selected: bool, k: &Kit) -> SidebarRow {
    SidebarRow::new(title)
        .height(56.0)
        .avatar(rune::el())
        .title_style(TextStyle::new(12.0, 400.0).lh(20.0))
        .subtitle(subtitle)
        .subtitle_style(TextStyle::new(10.0, 400.0).lh(18.0))
        .subtitle_color(k.c.text_muted)
        .selected(selected)
}

pub fn selected_row(element: El, selected: bool, k: &Kit) -> El {
    let element = element.radius(10.0);
    if selected {
        element.fill(k.c.pressed)
    } else {
        element
    }
}

pub fn user_message(message: &str, k: &Kit) -> El {
    user_bubble(message, false, k).child(
        rune_kit::Markdown::new(message)
            .selectable(true)
            .render(&user_markdown(k)),
    )
}

pub fn user_markdown(k: &Kit) -> rune_kit::MarkdownStyle {
    crate::theme::markdown(k)
        .body_font(TextStyle::new(14.0, 400.0).lh(23.0))
        .code_block_padding(10.0, 12.0)
        .list(18.0, 3.0)
}

pub fn user_bubble(message: &str, attachments: bool, k: &Kit) -> El {
    let maximum =
        rune::memo::window_layout("main-pane").map_or(660.0, |r| (r[2] - 96.0).clamp(140.0, 660.0));
    let width = if message.len() < 180 && !message.contains('\n') {
        rune::text::measure(message, &[], TextStyle::new(14.0, 400.0).lh(23.0), None).0 + 30.0
    } else {
        maximum
    };
    col()
        .self_end()
        .w(width
            .max(if attachments { 320.0 } else { 48.0 })
            .min(maximum))
        .max_w(maximum)
        .gap(10.0)
        .px(14.0)
        .py(10.0)
        .radius(14.0)
        .fill(k.c.layer_02.mix(k.c.bg_base, 0.25))
}

pub fn composer_editor(input: El, minimum: f32, maximum: f32) -> El {
    rune::auto_grow(input, 2, 24)
        .key("composer-editor")
        .px(4.0)
        .py(4.0)
        .min_h(minimum)
        .max_h(maximum)
}

pub fn composer(input: El, controls: El, context: El, padding: f32, k: &Kit) -> El {
    composer_dock(
        col()
            .key("message-composer")
            .role("form")
            .label("Message composer")
            .focus_return("question-composer")
            .child(
                context
                    .scroll(rune::ScrollSpec {
                        stick_bottom: false,
                    })
                    .scroll_bar(k),
            )
            .child(input)
            .child(divider(k).opacity(0.5).mt(8.0).mb(8.0))
            .child(controls.min_h(32.0)),
        padding,
        k,
    )
}

pub fn composer_dock(content: El, padding: f32, k: &Kit) -> El {
    let rim =
        k.c.border_strong
            .mix(rune::Color::WHITE, if k.dark { 0.12 } else { 0.35 });
    // The dock sits over a uniform surface. Tint and top lighting give it depth
    // without a backdrop pass on every streamed frame.
    col().w_full().max_w(840.0).px(padding).pb(24.0).child(
        content
            .w_full()
            .padding(12.0)
            .radius(16.0)
            .fill(k.alpha(k.c.layer_02, 0.94))
            .gradient(
                k.alpha(rune::Color::WHITE, if k.dark { 0.035 } else { 0.4 }),
                k.alpha(k.c.bg_deep, if k.dark { 0.04 } else { 0.12 }),
            )
            .border_gradient(1.0, rim, k.c.border_base)
            .box_shadow(rune::Shadow {
                y: 1.0,
                color: k.alpha(rune::Color::WHITE, if k.dark { 0.045 } else { 0.8 }),
                inset: true,
                ..Default::default()
            })
            .shadow(
                6.0,
                24.0,
                k.alpha(rune::Color::BLACK, if k.dark { 0.2 } else { 0.07 }),
            ),
    )
}

pub fn activity_color(
    activity: &caelum_core::application::presentation::Activity,
    k: &Kit,
) -> rune::Color {
    use caelum_core::application::presentation::ActivityKind;
    match activity.kind {
        ActivityKind::Input
        | ActivityKind::Background
        | ActivityKind::Retrying
        | ActivityKind::Stopping => k.c.warning.fg,
        _ => k.c.text_accent,
    }
}

pub fn empty(title: &str, detail: &str, k: &Kit) -> El {
    col()
        .w_full()
        .py(48.0)
        .px(24.0)
        .gap(12.0)
        .items_center()
        .child(icon(&crate::ORBIT, 28.0).color(k.c.text_accent))
        .child(
            text(title)
                .font(TextStyle::new(20.0, 550.0))
                .color(k.c.text_base)
                .wrap(),
        )
        .child(caption(detail, k))
}

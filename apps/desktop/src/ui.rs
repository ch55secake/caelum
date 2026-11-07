//! Caelum's shared visual vocabulary, used by live views and deterministic previews.
use rune::{El, Icon, TextStyle, col, icon, row, text};
use rune_kit::{Kit, shell::SidebarRow};

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
            return rune_kit::Spinner::new()
                .size(14.0)
                .build(k)
                .label("Running")
                .key("status-running");
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
pub static REFRESH: Icon = Icon {
    name: "refresh",
    view: 16.0,
    body: r#"<path d="M13 6a5 5 0 0 0-8-3L2 6m0-4v4h4M3 10a5 5 0 0 0 8 3l3-3m0 4v-4h-4" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"/>"#,
};
pub static MORE: Icon = Icon {
    name: "more",
    view: 16.0,
    body: r#"<circle cx="3" cy="8" r="1" fill="currentColor"/><circle cx="8" cy="8" r="1" fill="currentColor"/><circle cx="13" cy="8" r="1" fill="currentColor"/>"#,
};
pub static ATTACH: Icon = Icon {
    name: "attach",
    view: 16.0,
    body: r#"<path d="m5 9 5-5a2 2 0 0 1 3 3l-6 6a3.5 3.5 0 0 1-5-5l6-6" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/>"#,
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
                .color(k.c.text_base),
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
        element
            .fill(k.alpha(k.c.text_accent, 0.08))
            .border(1.0, k.alpha(k.c.text_accent, 0.18))
    } else {
        element
    }
}

pub fn user_message(message: &str, k: &Kit) -> El {
    row()
        .w_full()
        .radius(10.0)
        .clip_rounded(10.0)
        .fill(k.alpha(k.c.text_accent, 0.025))
        .border(1.0, k.c.border_muted)
        .child(rune::el().w(2.0).h_full().fill(k.c.text_accent).radius(2.0))
        .child(
            text(message)
                .font(TextStyle::new(14.0, 400.0).lh(23.0))
                .color(k.c.text_base)
                .wrap()
                .selectable(true)
                .grow(1.0)
                .min_w(0.0)
                .padding(16.0),
        )
}

pub fn composer(input: El, controls: El, context: El, focused: bool, padding: f32, k: &Kit) -> El {
    col().w_full().max_w(840.0).px(padding).pb(24.0).child(
        col()
            .w_full()
            .padding(14.0)
            .radius(14.0)
            .fill(k.c.layer_02)
            .border(
                1.0,
                if focused {
                    k.alpha(k.c.text_accent, 0.55)
                } else {
                    k.c.border_strong
                },
            )
            .child(context)
            .child(
                rune::auto_grow(input, 2, 8)
                    .key("composer-editor")
                    .min_h(56.0)
                    .max_h(180.0),
            )
            .child(controls.min_h(32.0).mt(8.0)),
    )
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

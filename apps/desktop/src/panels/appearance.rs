use rune::{El, col, row};
use rune_kit::Kit;

use crate::live::{live, live_mut};
use crate::{Workspace, theme, ui};

pub(crate) fn appearance_settings(s: &Workspace, k: &Kit) -> El {
    let preferences = &live(s).app.preferences;
    let mut content = col().w_full().gap(8.0).child(crate::settings::heading(
        "Appearance",
        "Choose a theme and an accent for your workspace.",
        k,
    ));
    let mut themes = row().gap(4.0);
    for (mode, name) in [("system", "System"), ("dark", "Dark"), ("light", "Light")] {
        themes = themes.child(
            ui::tab(name, preferences.theme == mode, k)
                .on_press(move |s: &mut Workspace| {
                    live_mut(s).app.preferences.theme = mode.into();
                    s.dark = None;
                })
                .key(format!("theme-{mode}"))
                .grow(1.0),
        );
    }
    content = content
        .child(crate::settings::setting_row(
            "Colour mode",
            "Follow your system or choose a mode.",
            themes,
            k,
        ))
        .child(ui::divider(k))
        .child(ui::caption("Theme", k).pt(8.0));
    let mut families = row().w_full().flex_wrap().gap(8.0);
    for (id, name) in theme::FAMILIES {
        let palette = theme::appearance(k.dark, id, "theme");
        families = families.child(
            k.interactive(
                col()
                    .grow(1.0)
                    .min_w(84.0)
                    .gap(6.0)
                    .padding(8.0)
                    .radius(6.0)
                    .fill(k.c.bg_base)
                    .border(
                        1.0,
                        if preferences.theme_family == id {
                            k.c.border_focus
                        } else {
                            k.c.border_base
                        },
                    )
                    .child(
                        row()
                            .w_full()
                            .h(30.0)
                            .radius(5.0)
                            .fill(palette.c.bg_base)
                            .child(rune::el().w(22.0).h_full().fill(palette.c.bg_deep))
                            .child(
                                rune::el()
                                    .w(30.0)
                                    .h(5.0)
                                    .ml(8.0)
                                    .mt(12.0)
                                    .radius(2.0)
                                    .fill(palette.c.text_accent),
                            ),
                    )
                    .child(ui::caption(name, k)),
            )
            .role("button")
            .label(name)
            .focusable(true)
            .focus_ring(1.0, 0.0, k.c.border_focus)
            .on_press(move |s: &mut Workspace| {
                let p = &mut live_mut(s).app.preferences;
                if id == "vercel" && p.theme_family != id {
                    p.accent_color = "theme".into();
                }
                p.theme_family = id.into();
            })
            .key(format!("family-{id}")),
        );
    }
    content = content.child(families).child(ui::divider(k).mt(8.0));
    let mut accents = row().gap(6.0).center_y();
    for accent in theme::ACCENTS {
        let palette = theme::appearance(k.dark, &preferences.theme_family, accent);
        accents = accents.child(
            k.interactive(
                rune::el()
                    .size(26.0, 26.0)
                    .radius(13.0)
                    .fill(palette.c.text_accent)
                    .border(
                        if preferences.accent_color == accent {
                            3.0
                        } else {
                            1.0
                        },
                        k.c.text_base,
                    ),
            )
            .role("button")
            .label(format!("{accent} accent"))
            .focusable(true)
            .focus_ring(2.0, 2.0, k.c.border_focus)
            .on_press(move |s: &mut Workspace| {
                live_mut(s).app.preferences.accent_color = accent.into()
            })
            .key(format!("accent-{accent}")),
        );
    }
    content = content.child(crate::settings::setting_row(
        "Accent colour",
        "Highlights and active controls.",
        accents,
        k,
    ));
    let mut scopes = row().gap(4.0);
    for (scope, key, label) in [
        (
            caelum_core::application::SessionScope::AllProjects,
            "all",
            "All projects",
        ),
        (
            caelum_core::application::SessionScope::CurrentProject,
            "current",
            "Current project",
        ),
    ] {
        scopes = scopes.child(
            ui::tab(label, preferences.session_scope == scope, k)
                .key(format!("session-scope-{key}"))
                .grow(1.0)
                .on_press(move |s: &mut Workspace| {
                    let l = live_mut(s);
                    l.app.preferences.session_scope = scope;
                    l.feed.project = None;
                }),
        );
    }
    content
        .child(ui::divider(k).mt(8.0))
        .child(ui::caption("Sidebar", k).pt(8.0))
        .child(crate::settings::setting_row(
            "Session scope",
            "All projects or the current project.",
            scopes,
            k,
        ))
}

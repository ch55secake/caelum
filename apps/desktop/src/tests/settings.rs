use super::*;

#[test]
fn vercel_theme_is_keyboard_selectable_and_retains_mode_and_accent_on_restore() {
    use caelum_core::application::{Event, Reply, Snapshot};
    for size in [(800.0, 620.0), (1280.0, 1000.0)] {
        let mut rt = design_runtime("vercel-settings", size);
        click(&mut rt, "accent-violet");
        click(&mut rt, "family-caelum");
        key(&mut rt, Key::Tab, Mods::default());
        advance(&mut rt, 0.1);
        assert!(element(&rt, "family-vercel").focused);
        key(&mut rt, Key::Text(" ".into()), Mods::default());
        advance(&mut rt, 0.2);
        assert_eq!(
            rt.state.live.as_ref().unwrap().app.preferences.accent_color,
            "theme"
        );
        for (family, _) in crate::theme::FAMILIES {
            let card = element(&rt, &format!("family-{family}"));
            assert!(
                card.rect
                    .iter()
                    .zip(card.visible)
                    .all(|(a, b)| (*a - b).abs() < 1.0),
                "{card:?}"
            );
        }
        click(&mut rt, "accent-teal");
        click(&mut rt, "family-vercel");
        for mode in ["light", "dark", "system"] {
            click(&mut rt, &format!("theme-{mode}"));
            advance(&mut rt, 0.3);
            let p = &rt.state.live.as_ref().unwrap().app.preferences;
            assert_eq!(p.theme, mode);
            assert_eq!(p.theme_family, "vercel");
            assert_eq!(p.accent_color, "teal");
        }
        let snapshot: Snapshot = serde_json::from_slice(
            &serde_json::to_vec(&rt.state.live.as_ref().unwrap().app.snapshot()).unwrap(),
        )
        .unwrap();
        let mut restored = caelum_core::application::Workspace::default();
        restored.enable_store("unused-fixture-state.json".into());
        let load = restored.commands().pop().unwrap().0;
        restored.apply(Event::Completed(
            load,
            Ok(Reply::Snapshot(Some(Box::new(snapshot)))),
        ));
        assert_eq!(restored.preferences.theme_family, "vercel");
        assert_eq!(restored.preferences.accent_color, "teal");
    }
    let luminance = |c: rune::Color| {
        let linear = |v: f32| {
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(c.r) + 0.7152 * linear(c.g) + 0.0722 * linear(c.b)
    };
    for dark in [false, true] {
        for accent in crate::theme::ACCENTS {
            let k = crate::theme::appearance(dark, "vercel", accent);
            let foreground = luminance(k.c.submit_fg);
            let background = luminance(k.c.bg_icon_button_contrast);
            assert!(
                (foreground.max(background) + 0.05) / (foreground.min(background) + 0.05) >= 4.5,
                "unreadable Vercel submit control: {dark}, {accent}"
            );
        }
    }
}

#[test]
fn settings_pages_keep_controls_visible_and_reset_scrolling_on_navigation() {
    for size in [(800.0, 620.0), (1280.0, 1000.0)] {
        let mut rt = design_runtime("vercel-settings", size);
        for page in [
            "appearance",
            "providers",
            "worktrees",
            "notifications",
            "about",
        ] {
            click(&mut rt, &format!("settings-{page}"));
            advance(&mut rt, 0.3);
            for control in rt
                .elements()
                .into_iter()
                .filter(|e| !e.exiting && e.focusable && e.path.contains("settings-scroll"))
            {
                if control.visible[3] < control.rect[3] {
                    rt.update(|_, tx| {
                        tx.scroll_into_view(
                            control.key.clone().unwrap_or_else(|| control.path.clone()),
                        )
                    });
                    advance(&mut rt, 0.3);
                }
                let control = rt
                    .elements()
                    .into_iter()
                    .find(|e| e.path == control.path && !e.exiting)
                    .unwrap();
                assert!(
                    control
                        .rect
                        .iter()
                        .zip(control.visible)
                        .all(|(a, b)| (*a - b).abs() < 1.0),
                    "{page}: {control:?}"
                );
            }
        }
        click(&mut rt, "settings-appearance");
        key(&mut rt, Key::Down, Mods::default());
        assert!(element(&rt, "settings-providers").focused);
        key(&mut rt, Key::Text(" ".into()), Mods::default());
        assert!(element(&rt, "settings-providers").item_selected);
        rt.update(|s, _| {
            let app = &mut s.live.as_mut().unwrap().app;
            let scope = app.sessions[0].scope.clone();
            app.choices.get_mut(&scope).unwrap().models[0].name = "Friendly model name".into();
            app.preferences
                .providers
                .entry("codex".into())
                .or_default()
                .model = Some("Test model".into());
            app.notices
                .insert(scope, "Provider connection needs attention.\n".repeat(24));
        });
        advance(&mut rt, 0.3);
        assert!(
            rt.elements()
                .iter()
                .any(|e| e.path.contains("settings-model-trigger")
                    && e.text.as_deref() == Some("Friendly model name"))
        );
        rt.update(|_, tx| tx.scroll_to_y("settings-scroll", 240.0));
        advance(&mut rt, 0.4);
        assert!(element(&rt, "settings-model-trigger").scrolled[1] > 100.0);
        click(&mut rt, "settings-appearance");
        click(&mut rt, "settings-providers");
        advance(&mut rt, 0.4);
        let heading = rt
            .elements()
            .into_iter()
            .find(|e| {
                !e.exiting
                    && e.path.contains("settings-scroll")
                    && e.text.as_deref() == Some("Providers")
            })
            .unwrap();
        assert!(heading.scrolled[1].abs() < 1.0 && heading.visible[3] > 0.0);
    }
}

#[test]
fn about_update_checks_hide_stale_downloads_and_keep_errors_scoped() {
    use caelum_core::application::{Command, Event, Reply, Task, TaskReply};
    use caelum_protocol::{UpdateCheck, UpdateStatus};
    let mut rt = design_runtime("about", (800.0, 620.0));
    let clipboard = rune::clipboard::memory();
    rt.use_clipboard(&clipboard);
    click(&mut rt, "copy-app-details");
    let details = clipboard.text().unwrap();
    for value in [
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        "MIT licence",
        caelum_core::updates::REPOSITORY_URL,
    ] {
        assert!(details.contains(value));
    }
    key(&mut rt, Key::Tab, Mods::default());
    assert!(element(&rt, "check-updates").focused);
    key(&mut rt, Key::Text(" ".into()), Mods::default());
    advance(&mut rt, 0.2);
    let request = |rt: &mut Runtime<Workspace>| {
        rt.state
            .live
            .as_mut()
            .unwrap()
            .app
            .commands()
            .into_iter()
            .find_map(|(id, command)| matches!(command, Command::Task(Task::Updates)).then_some(id))
            .unwrap()
    };
    let first = request(&mut rt);
    assert!(!element(&rt, "check-updates").focusable);
    let update = UpdateCheck {
        current_version: env!("CARGO_PKG_VERSION").into(),
        latest_version: "0.2.0".into(),
        status: UpdateStatus::Available,
        release_url: "https://github.com/ch55secake/caelum/releases/tag/v0.2.0".into(),
        download_url: Some("https://github.com/ch55secake/caelum/releases/download/v0.2.0/Caelum-v0.2.0-aarch64.dmg".into()),
    };
    rt.update(|s, _| {
        s.live.as_mut().unwrap().app.apply(Event::Completed(
            first,
            Ok(Reply::Task(TaskReply::Updates(update.clone()))),
        ))
    });
    advance(&mut rt, 0.3);
    assert!(element(&rt, "download-update").visible[3] >= 30.0);
    click(&mut rt, "check-updates");
    let retry = request(&mut rt);
    assert!(rt.find("download-update").is_none());
    rt.update(|s, _| {
        let app = &mut s.live.as_mut().unwrap().app;
        app.apply(Event::Completed(
            retry,
            Err("Update service is offline".into()),
        ));
        app.task(Task::ImportSettings {
            path: "old-settings.json".into(),
        });
        for (id, command) in app.commands() {
            if matches!(command, Command::Task(Task::ImportSettings { .. })) {
                app.apply(Event::Completed(id, Err("Invalid settings file".into())));
            }
        }
    });
    advance(&mut rt, 0.3);
    assert!(rt.find("download-update").is_none());
    let status = rt
        .elements()
        .into_iter()
        .find(|e| e.role == "status" && e.path.contains("about-updates"))
        .unwrap();
    assert!(
        status
            .label
            .as_ref()
            .unwrap()
            .contains("Update service is offline")
    );
    assert!(!status.label.unwrap().contains("Invalid settings file"));
    assert!(
        rt.elements()
            .iter()
            .any(|e| e.text.as_deref() == Some("Invalid settings file"))
    );
    click(&mut rt, "check-updates");
    let retry = request(&mut rt);
    rt.update(|s, _| {
        s.live.as_mut().unwrap().app.apply(Event::Completed(
            retry,
            Ok(Reply::Task(TaskReply::Updates(UpdateCheck {
                status: UpdateStatus::UpToDate,
                latest_version: env!("CARGO_PKG_VERSION").into(),
                ..update.clone()
            }))),
        ));
    });
    advance(&mut rt, 0.3);
    assert!(rt.find("download-update").is_none());
    assert!(
        rt.elements()
            .iter()
            .any(|e| e.text.as_deref() == Some("You’re up to date"))
    );
}

#[test]
fn about_release_states_fit_the_compact_dialog_and_offer_the_right_action() {
    for scene in [
        "about-checking",
        "about-current",
        "about-ahead",
        "about-release",
        "vercel-about-update",
    ] {
        let rt = design_runtime(scene, (800.0, 620.0));
        assert_eq!(
            rt.find("download-update").is_some(),
            scene == "vercel-about-update"
        );
        assert_eq!(rt.find("view-release").is_some(), scene == "about-release");
        for control in rt
            .elements()
            .into_iter()
            .filter(|e| !e.exiting && e.focusable && e.path.contains("settings-scroll"))
        {
            assert!(
                control
                    .rect
                    .iter()
                    .zip(control.visible)
                    .all(|(a, b)| (*a - b).abs() < 1.0),
                "{scene}: {control:?}"
            );
        }
    }
}

#[test]
fn settings_dropdown_escape_dismisses_only_the_foremost_surface() {
    for dark in [false, true] {
        let app = spec();
        let mut state = crate::evaluation::design();
        crate::evaluation::scene(&mut state, "providers");
        state.live.as_mut().unwrap().app.sessions[0]
            .conversation
            .working = true;
        let mut rt = Runtime::new(state, app.view, (800.0, 620.0), Env::new(dark, 1.0));
        rt.poll = app.poll;
        rt.on_key = app.on_key;
        advance(&mut rt, 0.4);
        click(&mut rt, "Default effort-trigger");
        advance(&mut rt, 0.3);
        assert!(element(&rt, "option1").visible[3] > 20.0);
        click(&mut rt, "option1");
        assert_eq!(
            rt.state.live.as_ref().unwrap().app.preferences.providers["codex"]
                .effort
                .as_deref(),
            Some("Medium")
        );
        click(&mut rt, "Default effort-trigger");
        key(&mut rt, Key::Escape, Mods::default());
        advance(&mut rt, 0.3);
        assert!(
            rt.state
                .live
                .as_ref()
                .unwrap()
                .panels
                .settings_menu
                .is_none()
        );
        assert_eq!(
            rt.state.live.as_ref().unwrap().panels.overlay,
            Some(crate::panels::Screen::Settings)
        );
        key(&mut rt, Key::Escape, Mods::default());
        advance(&mut rt, 0.3);
        assert!(rt.state.live.as_ref().unwrap().panels.overlay.is_none());
        assert!(!rt.state.live.as_ref().unwrap().app.sessions[0].stopping);
    }
}

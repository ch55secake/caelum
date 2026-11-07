//! Settings navigation and native controls. Preferences and async work live in core.
use crate::{
    Workspace,
    live::{live, live_mut},
    panels, ui,
};
use crate::{select::Select, ui::Button};
use caelum_core::{
    application::{Provider, Scope, Task},
    updates,
};
use rune::{El, FileDialogOptions, ScrollSpec, col, row, text};
use rune_kit::{ButtonVariant, Kit, ScrollBar, SelectAppearance, Switch};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    Appearance,
    Providers,
    Worktrees,
    Notifications,
    About,
}

pub fn view(s: &Workspace, k: &Kit, layers: &mut Vec<El>) -> El {
    let page = live(s).panels.settings_page;
    let mut nav = col().w(132.0).h_full().gap(4.0).pr(12.0);
    for (name, target) in [
        ("Appearance", Page::Appearance),
        ("Providers", Page::Providers),
        ("Worktrees", Page::Worktrees),
        ("Notifications", Page::Notifications),
        ("About", Page::About),
    ] {
        nav = nav.child(
            ui::tab(name, page == target, k)
                .on_press(move |s: &mut Workspace| {
                    let p = &mut live_mut(s).panels;
                    p.settings_page = target;
                    p.settings_menu = None;
                })
                .w_full()
                .key(format!("settings-{}", name.to_lowercase())),
        );
    }
    let body = match page {
        Page::Appearance => panels::appearance_settings(s, k),
        Page::Providers => providers(s, k, layers),
        Page::Worktrees => worktrees(s, k),
        Page::Notifications => notifications(s, k),
        Page::About => about(s, k),
    };
    let mut content = col().w_full().gap(12.0).child(body);
    if let Some(error) = &live(s).app.persistence_error {
        content = content
            .child(ui::caption(error, k).color(k.c.danger.fg))
            .child(
                Button::new("Retry save")
                    .on_press(|s: &mut Workspace| live_mut(s).app.retry_save())
                    .build(k),
            );
    }
    row()
        .w_full()
        .h_full()
        .min_h(0.0)
        .gap(16.0)
        .child(nav)
        .child(rune::el().w(1.0).h_full().fill(k.c.border_base))
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .h_full()
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .child(content)
                .key("settings-scroll")
                .scroll_bar(k),
        )
}

fn picker(
    s: &Workspace,
    k: &Kit,
    layers: &mut Vec<El>,
    id: &'static str,
    values: &[String],
    current: &str,
    change: impl Fn(&mut Workspace, &str) + Clone + 'static,
) -> El {
    Select::new(id, values)
        .appearance(SelectAppearance::Field)
        .width(260.0)
        .current(Some(current))
        .label(id)
        .open(live(s).panels.settings_menu == Some(id))
        .on_toggle(move |s: &mut Workspace| {
            let p = &mut live_mut(s).panels;
            p.settings_menu = if p.settings_menu == Some(id) {
                None
            } else {
                Some(id)
            };
        })
        .on_close(|s: &mut Workspace| live_mut(s).panels.settings_menu = None)
        .on_choose(move |s: &mut Workspace, value, _| {
            change(s, value);
            live_mut(s).panels.settings_menu = None;
        })
        .build(k, layers)
}

fn providers(s: &Workspace, k: &Kit, layers: &mut Vec<El>) -> El {
    let app = &live(s).app;
    let provider = live(s).panels.settings_provider;
    let mut body = col().w_full().gap(12.0).child(ui::section(
        "Providers",
        "Choose the provider and defaults for new sessions.",
        k,
    ));
    let mut defaults = row().gap(4.0);
    for p in Provider::ALL {
        defaults = defaults.child(
            ui::tab(
                ui::provider_name(p),
                app.preferences.default_provider == p,
                k,
            )
            .on_press(move |s: &mut Workspace| live_mut(s).app.preferences.default_provider = p)
            .key(format!("default-provider-{}", p.as_str())),
        );
    }
    body = body.child(
        ui::card(k)
            .gap(8.0)
            .child(ui::caption("Start new sessions with", k))
            .child(defaults),
    );
    let mut tabs = row().gap(6.0);
    for p in Provider::ALL {
        tabs = tabs.child(
            ui::tab(ui::provider_name(p), p == provider, k)
                .on_press(move |s: &mut Workspace| {
                    live_mut(s).panels.settings_provider = p;
                    live_mut(s).app.refresh_choices(p);
                })
                .key(format!("settings-provider-{}", p.as_str())),
        );
    }
    body = body.child(tabs);
    let Some(project) = &app.active_project else {
        return body.child(ui::caption(
            "Open a project to discover available models and agents.",
            k,
        ));
    };
    let scope = Scope {
        provider,
        project: project.clone(),
    };
    let choices = app.choices.get(&scope);
    body = body.child(
        row()
            .gap(8.0)
            .center_y()
            .child(
                ui::caption(
                    if app.notices.contains_key(&scope) {
                        "Unavailable"
                    } else if choices.is_some() {
                        "Available"
                    } else {
                        "Not connected"
                    },
                    k,
                )
                .grow(1.0),
            )
            .child(
                Button::new("Refresh status")
                    .variant(ButtonVariant::GhostMuted)
                    .on_press(move |s: &mut Workspace| live_mut(s).app.refresh_choices(provider))
                    .build(k),
            ),
    );
    if let Some(error) = app.notices.get(&scope) {
        body = body.child(ui::notice("Connection unavailable", error, k));
    }
    let options = app
        .preferences
        .providers
        .get(provider.as_str())
        .cloned()
        .unwrap_or_default();
    body = body.child(ui::caption("Default model", k)).child(
        Select::new::<&str>("settings-model", &[])
            .appearance(SelectAppearance::Field)
            .width(260.0)
            .current(options.model.as_deref())
            .placeholder("Provider default")
            .label("Default model")
            .open(live(s).models.is_open())
            .on_toggle(move |s: &mut Workspace| crate::models::open(s, Some(provider)))
            .trigger(k),
    );
    let model = choices.and_then(|c| {
        c.models.iter().find(|m| {
            Some(m.id.as_str()) == options.model.as_deref().or(c.default_model.as_deref())
        })
    });
    let mut efforts = vec!["Model default".into()];
    if let Some(model) = model {
        efforts.extend(model.efforts.iter().map(|e| e.id.clone()));
    }
    if let Some(saved) = &options.effort
        && !efforts.contains(saved)
    {
        efforts.push(saved.clone());
    }
    body = body.child(ui::caption("Reasoning effort", k)).child(picker(
        s,
        k,
        layers,
        "Default effort",
        &efforts,
        options.effort.as_deref().unwrap_or("Model default"),
        move |s, value| {
            live_mut(s)
                .app
                .preferences
                .providers
                .entry(provider.as_str().into())
                .or_default()
                .effort = (value != "Model default").then(|| value.into());
        },
    ));
    if provider == Provider::OpenCode {
        let mut agents = vec!["Provider default".into()];
        if let Some(choices) = choices {
            agents.extend(choices.agents.iter().map(|a| a.id.clone()));
        }
        body = body.child(ui::caption("Default agent", k)).child(picker(
            s,
            k,
            layers,
            "Default agent",
            &agents,
            options.agent.as_deref().unwrap_or("Provider default"),
            move |s, value| {
                live_mut(s)
                    .app
                    .preferences
                    .providers
                    .entry(provider.as_str().into())
                    .or_default()
                    .agent = (value != "Provider default").then(|| value.into());
            },
        ));
    }
    if let Some(model) = &options.model {
        let id = model.clone();
        let favorite = app
            .preferences
            .favorite_models
            .get(provider.as_str())
            .is_some_and(|f| f.contains(model));
        body = body.child(
            Button::new(if favorite {
                "★ Remove favourite"
            } else {
                "☆ Favourite model"
            })
            .variant(ButtonVariant::GhostMuted)
            .on_press(move |s: &mut Workspace| {
                let favorites = live_mut(s)
                    .app
                    .preferences
                    .favorite_models
                    .entry(provider.as_str().into())
                    .or_default();
                if favorites.contains(&id) {
                    favorites.retain(|f| f != &id);
                } else {
                    favorites.push(id.clone());
                }
            })
            .build(k),
        );
    }
    body.child(
        Button::new("Reset provider defaults")
            .variant(ButtonVariant::GhostMuted)
            .on_press(move |s: &mut Workspace| {
                live_mut(s)
                    .app
                    .preferences
                    .providers
                    .remove(provider.as_str());
            })
            .build(k),
    )
}

fn worktrees(s: &Workspace, k: &Kit) -> El {
    col()
        .w_full()
        .gap(12.0)
        .child(ui::section(
            "Worktrees",
            "Choose where Caelum creates independent checkouts.",
            k,
        ))
        .child(
            ui::field(
                &live(s).panels.directory,
                "Worktree storage directory",
                "~/.caelum/worktrees",
                k,
            )
            .on_edit(|s: &mut Workspace| &mut live_mut(s).panels.directory),
        )
        .child(
            row()
                .gap(8.0)
                .child(
                    Button::new("Choose folder…")
                        .variant(ButtonVariant::Outline)
                        .on_press_tx(|_: &mut Workspace, tx| {
                            tx.open_folder(
                                FileDialogOptions::default(),
                                |s: &mut Workspace, result, _| match result {
                                    Ok(Some(path)) => live_mut(s)
                                        .panels
                                        .directory
                                        .replace_text(path.to_string_lossy().into_owned()),
                                    Err(error) => live_mut(s).app.error = Some(error.to_string()),
                                    _ => {}
                                },
                            )
                        })
                        .build(k),
                )
                .child(
                    Button::new("Save directory")
                        .variant(ButtonVariant::Outline)
                        .disabled(
                            live(s).panels.directory.text
                                == live(s).app.preferences.worktree_directory,
                        )
                        .on_press(|s: &mut Workspace| {
                            let l = live_mut(s);
                            l.app.preferences.worktree_directory = l.panels.directory.text.clone();
                        })
                        .build(k),
                ),
        )
        .child(
            Button::new("Use default location")
                .variant(ButtonVariant::GhostMuted)
                .on_press(|s: &mut Workspace| {
                    let l = live_mut(s);
                    l.panels.directory.clear();
                    l.app.preferences.worktree_directory.clear();
                })
                .build(k),
        )
}

fn notifications(s: &Workspace, k: &Kit) -> El {
    let p = &live(s).app.preferences;
    let mut body = col().w_full().gap(12.0).child(ui::section(
        "Notifications",
        "A gentle sound when work finishes or needs your input.",
        k,
    ));
    for (name, detail, mode) in [
        ("Off", "Keep the workspace quiet", "off"),
        (
            "Background only",
            "Notify when the response isn’t visible",
            "background",
        ),
        ("Always", "Notify whenever work finishes", "always"),
    ] {
        body = body.child(
            ui::choice(
                name,
                detail,
                (p.notifications && p.notification_mode == mode)
                    || (!p.notifications && mode == "off"),
                k,
            )
            .on_press(move |s: &mut Workspace| {
                let p = &mut live_mut(s).app.preferences;
                p.notification_mode = mode.into();
                p.notifications = mode != "off";
            })
            .key(format!("notification-{mode}")),
        );
    }
    body = body.child(ui::caption(
        "Background only sounds when the latest response is not visible in the foreground.",
        k,
    ));
    let mut volume = row()
        .gap(6.0)
        .child(ui::caption(format!("Volume: {}%", p.sound_volume), k));
    for (name, delta) in [("−", -10i16), ("+", 10)] {
        volume = volume.child(
            Button::new(name)
                .variant(ButtonVariant::Outline)
                .disabled(if delta < 0 {
                    p.sound_volume == 0
                } else {
                    p.sound_volume == 100
                })
                .on_press(move |s: &mut Workspace| {
                    let p = &mut live_mut(s).app.preferences;
                    p.sound_volume = (p.sound_volume as i16 + delta).clamp(0, 100) as u8;
                })
                .build(k),
        );
    }
    body = body.child(volume).child(
        Button::new("Test sound")
            .disabled(p.sound_volume == 0)
            .on_press(|s: &mut Workspace| live_mut(s).app.test_sound())
            .build(k),
    );
    for (name, shell) in [
        ("Background shell completions", true),
        ("Approvals and questions needing input", false),
    ] {
        let enabled = if shell {
            p.background_shells
        } else {
            p.input_needed
        };
        body = body.child(
            row()
                .w_full()
                .gap(12.0)
                .py(8.0)
                .center_y()
                .child(ui::caption(name, k).grow(1.0))
                .child(
                    Switch::new(enabled)
                        .label(name)
                        .hide_label()
                        .on_toggle(move |s: &mut Workspace| {
                            let p = &mut live_mut(s).app.preferences;
                            if shell {
                                p.background_shells = !p.background_shells;
                            } else {
                                p.input_needed = !p.input_needed;
                            }
                        })
                        .build(k),
                ),
        );
    }
    body
}

fn about(s: &Workspace, k: &Kit) -> El {
    let details = format!(
        "Caelum {}\n{} · {}\nMIT licence\n{}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        updates::REPOSITORY_URL
    );
    let copy = details.clone();
    let mut body = col()
        .w_full()
        .gap(12.0)
        .child(ui::section(
            "Caelum",
            "A native home for AI coding agents",
            k,
        ))
        .child(
            text(details)
                .font(k.ty.ui)
                .color(k.c.text_base)
                .wrap()
                .selectable(true),
        )
        .child(
            Button::new("Copy app details")
                .variant(ButtonVariant::Outline)
                .on_press_tx(move |_: &mut Workspace, tx| tx.copy(copy.clone()))
                .build(k)
                .key("copy-app-details"),
        );
    for (name, url) in [
        ("Repository", updates::REPOSITORY_URL),
        ("Releases", updates::RELEASES_URL),
        ("Licence", updates::LICENSE_URL),
    ] {
        body = body.child(
            Button::new(name)
                .variant(ButtonVariant::GhostMuted)
                .on_press_tx(move |_: &mut Workspace, tx| {
                    let _ = tx.open_url(url);
                })
                .build(k),
        );
    }
    let busy = live(s).app.services.busy(&Task::Updates);
    body = body.child(
        Button::new(if busy {
            "Checking…"
        } else {
            "Check for updates"
        })
        .variant(ButtonVariant::Outline)
        .disabled(busy)
        .on_press(|s: &mut Workspace| live_mut(s).app.task(Task::Updates))
        .build(k)
        .key("check-updates"),
    );
    if let Some(update) = &live(s).app.services.update {
        let status = match update.status {
            caelum_protocol::UpdateStatus::Available => {
                format!("Version {} is available", update.latest_version)
            }
            caelum_protocol::UpdateStatus::UpToDate => "You’re up to date".into(),
            caelum_protocol::UpdateStatus::Ahead => {
                "This build is newer than the latest release".into()
            }
        };
        body = body.child(ui::caption(status, k));
        if let Some(url) = &update.download_url {
            let url = url.clone();
            body = body.child(
                Button::new("Download update")
                    .on_press_tx(move |_: &mut Workspace, tx| {
                        let _ = tx.open_url(&url);
                    })
                    .build(k),
            );
        }
    }
    if let Some(error) = live(s)
        .app
        .services
        .checkouts
        .get("")
        .and_then(|c| c.error.as_ref())
    {
        body = body.child(ui::caption(error, k).color(k.c.danger.fg));
    }
    body.child(
        Button::new("Import legacy settings JSON…")
            .variant(ButtonVariant::GhostMuted)
            .on_press_tx(|_: &mut Workspace, tx| {
                tx.open_file(
                    FileDialogOptions::default(),
                    |s: &mut Workspace, result, _| match result {
                        Ok(Some(path)) => live_mut(s).app.task(Task::ImportSettings {
                            path: path.to_string_lossy().into_owned(),
                        }),
                        Err(error) => live_mut(s).app.error = Some(error.to_string()),
                        _ => {}
                    },
                )
            })
            .build(k)
            .key("import-settings"),
    )
}

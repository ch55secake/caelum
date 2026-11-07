use crate::{
    Workspace,
    live::{live, live_mut},
    panels::Screen,
    ui,
};
use caelum_core::application::Provider;
use rune::{El, Key, KeyEvent, ScrollSpec, TextEdit, Tx, col, row, text_input};
use rune_kit::{Button, ButtonVariant, Kit};

pub struct Picker {
    pub query: TextEdit,
    pub settings: Option<Provider>,
    pub selected: usize,
}
impl Default for Picker {
    fn default() -> Self {
        Self {
            query: TextEdit::new(""),
            settings: None,
            selected: 0,
        }
    }
}

pub fn open(s: &mut Workspace, settings: Option<Provider>) {
    let l = live_mut(s);
    l.models.settings = settings;
    l.models.query.clear();
    l.models.query.focused = true;
    l.models.selected = 0;
    l.panels.overlay = Some(Screen::Models);
}
pub fn close(s: &mut Workspace) {
    let l = live_mut(s);
    l.panels.overlay = if l.models.settings.take().is_some() {
        Some(Screen::Settings)
    } else {
        None
    };
    l.focus();
}
fn provider(s: &Workspace) -> Option<Provider> {
    let l = live(s);
    l.models
        .settings
        .or_else(|| l.app.selected.map(|i| l.app.sessions[i].scope.provider))
}
fn matches(s: &Workspace) -> Vec<(String, String, bool)> {
    let l = live(s);
    let Some(provider) = provider(s) else {
        return Vec::new();
    };
    let Some(project) = &l.app.active_project else {
        return Vec::new();
    };
    let favorites = l
        .app
        .preferences
        .favorite_models
        .get(provider.as_str())
        .cloned()
        .unwrap_or_default();
    let scope = caelum_core::application::Scope {
        project: project.clone(),
        provider,
    };
    let mut options: Vec<_> = l
        .app
        .choices
        .get(&scope)
        .into_iter()
        .flat_map(|c| &c.models)
        .map(|m| {
            let source = m
                .provider
                .as_ref()
                .map(|p| p.name.as_str())
                .unwrap_or(provider.as_str());
            let detail = format!(
                "{} · {}{}",
                m.name,
                source,
                m.context_window
                    .map(|n| format!(" · {}K context", n / 1000))
                    .unwrap_or_default()
            );
            (m.id.clone(), detail, true)
        })
        .collect();
    for id in &favorites {
        if !options.iter().any(|(value, _, _)| value == id) {
            options.push((id.clone(), "Currently unavailable".into(), false));
        }
    }
    options.sort_by_key(|(id, _, _)| !favorites.contains(id));
    let query = l.models.query.text.to_lowercase();
    options.retain(|(id, detail, _)| format!("{id} {detail}").to_lowercase().contains(&query));
    options
}
fn choose(s: &mut Workspace, id: &str) {
    let l = live_mut(s);
    if let Some(provider) = l.models.settings {
        let defaults = l
            .app
            .preferences
            .providers
            .entry(provider.as_str().into())
            .or_default();
        defaults.model = Some(id.into());
        defaults.effort = None;
    } else if let Some(index) = l.app.selected {
        let session = &mut l.app.sessions[index];
        session.options.model = Some(id.into());
        session.options.effort = l
            .app
            .choices
            .get(&session.scope)
            .and_then(|c| c.models.iter().find(|m| m.id == id))
            .and_then(|m| m.default_effort.clone());
        l.app.options_changed(index);
    }
    close(s);
}
fn key(s: &mut Workspace, ev: &KeyEvent, _: &mut Tx) -> bool {
    let items = matches(s);
    match ev.key {
        Key::Down => {
            live_mut(s).models.selected =
                (live(s).models.selected + 1).min(items.len().saturating_sub(1));
            true
        }
        Key::Up => {
            live_mut(s).models.selected = live(s).models.selected.saturating_sub(1);
            true
        }
        Key::Enter => {
            if let Some((id, _, true)) = items.get(live(s).models.selected) {
                choose(s, id);
            }
            true
        }
        Key::Escape => {
            close(s);
            true
        }
        _ => false,
    }
}
pub fn view(s: &Workspace, k: &Kit) -> El {
    let l = live(s);
    let Some(provider) = provider(s) else {
        return col();
    };
    let favorites = l
        .app
        .preferences
        .favorite_models
        .get(provider.as_str())
        .cloned()
        .unwrap_or_default();
    let items = matches(s);
    let mut list = col().w_full().gap(4.0);
    // Bound the menu independently of provider catalogue size; typing narrows it.
    let start = l.models.selected.saturating_sub(8);
    for (i, (id, detail, available)) in items.into_iter().enumerate().skip(start).take(40) {
        let pick = id.clone();
        let favorite = id.clone();
        let is_favorite = favorites.contains(&id);
        list = list.child(
            row()
                .w_full()
                .gap(6.0)
                .child(
                    Button::new(format!("{id}\n{detail}"))
                        .variant(if i == l.models.selected {
                            ButtonVariant::Outline
                        } else {
                            ButtonVariant::GhostMuted
                        })
                        .disabled(!available)
                        .on_press(move |s: &mut Workspace| choose(s, &pick))
                        .build(k)
                        .key(format!("option{i}"))
                        .grow(1.0)
                        .min_w(0.0),
                )
                .child(
                    Button::new(if is_favorite { "★" } else { "☆" })
                        .variant(ButtonVariant::GhostMuted)
                        .on_press(move |s: &mut Workspace| {
                            let favorites = live_mut(s)
                                .app
                                .preferences
                                .favorite_models
                                .entry(provider.as_str().into())
                                .or_default();
                            if favorites.contains(&favorite) {
                                favorites.retain(|f| f != &favorite);
                            } else {
                                favorites.push(favorite.clone());
                            }
                            live_mut(s).models.query.focused = true;
                        })
                        .build(k)
                        .key(format!("model-favorite-{i}"))
                        .label(if is_favorite {
                            "Remove favourite"
                        } else {
                            "Favourite model"
                        }),
                ),
        );
    }
    col()
        .w_full()
        .h_full()
        .min_h(0.0)
        .gap(10.0)
        .child(
            text_input(&l.models.query, k.ty.ui)
                .single_line()
                .w_full()
                .label("Search models")
                .key("model-search")
                .placeholder(
                    "Search models, providers or context capacity…",
                    k.c.text_muted,
                )
                .color(k.c.text_base)
                .on_edit(|s: &mut Workspace| &mut live_mut(s).models.query)
                .on_input_change(|s: &mut Workspace, _| live_mut(s).models.selected = 0)
                .on_input_key(key),
        )
        .child(ui::caption("↑↓ Navigate · Enter Choose · ★ Favourite", k))
        .child(
            col()
                .w_full()
                .grow(1.0)
                .min_h(0.0)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .child(list),
        )
}

//! Searchable model catalogue. The picker owns its target until it closes.
use crate::{
    Workspace,
    live::{live, live_mut},
    panels::Screen,
    ui,
};
use caelum_core::application::{Provider, Scope};
use rune::{El, Key, KeyEvent, ScrollSpec, TextEdit, Tx, col, row};
use rune_kit::{ButtonVariant, Kit, ScrollBar};

#[derive(Clone)]
enum Owner {
    Session(usize, Scope),
    Defaults(Scope),
}
impl Owner {
    fn scope(&self) -> &Scope {
        match self {
            Self::Session(_, scope) | Self::Defaults(scope) => scope,
        }
    }
}
pub struct Picker {
    pub query: TextEdit,
    owner: Option<Owner>,
    selected: Option<String>,
    pub reveal: Option<usize>,
}
impl Default for Picker {
    fn default() -> Self {
        Self {
            query: TextEdit::new(""),
            owner: None,
            selected: None,
            reveal: None,
        }
    }
}

pub fn open(s: &mut Workspace, settings: Option<Provider>) {
    let l = live_mut(s);
    let owner = if let Some(provider) = settings {
        let Some(project) = l.app.active_project.clone() else {
            return;
        };
        Owner::Defaults(Scope { project, provider })
    } else {
        let Some(index) = l.app.selected else {
            return;
        };
        Owner::Session(index, l.app.sessions[index].scope.clone())
    };
    l.models.selected = match &owner {
        Owner::Session(i, _) => l.app.sessions[*i].options.model.clone(),
        Owner::Defaults(scope) => l
            .app
            .preferences
            .providers
            .get(scope.provider.as_str())
            .and_then(|o| o.model.clone()),
    };
    l.models.owner = Some(owner);
    l.models.query.clear();
    l.models.query.focused = true;
    l.panels.settings_menu = None;
    l.panels.overlay = Some(Screen::Models);
    let at = selected_index(s, &matches(s));
    live_mut(s).models.reveal = Some(at);
}
pub fn close(s: &mut Workspace) {
    let l = live_mut(s);
    l.models.query.focused = false;
    l.panels.overlay = if matches!(l.models.owner.take(), Some(Owner::Defaults(_))) {
        Some(Screen::Settings)
    } else {
        l.focus();
        None
    };
}

#[derive(Clone)]
struct Model {
    id: String,
    name: String,
    detail: String,
    available: bool,
}
fn matches(s: &Workspace) -> Vec<Model> {
    let l = live(s);
    let Some(scope) = l.models.owner.as_ref().map(Owner::scope) else {
        return Vec::new();
    };
    let favorites = l
        .app
        .preferences
        .favorite_models
        .get(scope.provider.as_str());
    let mut options: Vec<_> = l
        .app
        .choices
        .get(scope)
        .into_iter()
        .flat_map(|c| &c.models)
        .map(|m| Model {
            id: m.id.clone(),
            name: m.name.clone(),
            available: true,
            detail: format!(
                "{} · {}{}",
                m.provider
                    .as_ref()
                    .map(|p| p.name.as_str())
                    .unwrap_or(ui::provider_name(scope.provider)),
                m.id,
                m.context_window
                    .map(|n| format!(" · {}K context", n / 1000))
                    .unwrap_or_default()
            ),
        })
        .collect();
    let current = match l.models.owner.as_ref().unwrap() {
        Owner::Session(index, _) => l.app.sessions[*index].options.model.as_ref(),
        Owner::Defaults(scope) => l
            .app
            .preferences
            .providers
            .get(scope.provider.as_str())
            .and_then(|o| o.model.as_ref()),
    };
    for id in favorites.into_iter().flatten().chain(current) {
        if !options.iter().any(|m| &m.id == id) {
            options.push(Model {
                id: id.clone(),
                name: id.clone(),
                detail: if Some(id) == current {
                    "Unavailable · current model"
                } else {
                    "Unavailable · saved favourite"
                }
                .into(),
                available: false,
            });
        }
    }
    options.sort_by_key(|m| {
        (
            !favorites.is_some_and(|f| f.contains(&m.id)),
            m.name.to_lowercase(),
            m.id.clone(),
        )
    });
    options.insert(
        0,
        Model {
            id: String::new(),
            name: "Provider default".into(),
            detail: "Use the provider’s recommended model".into(),
            available: true,
        },
    );
    let query = l.models.query.text.trim().to_lowercase();
    options.retain(|m| {
        format!("{} {}", m.name, m.detail)
            .to_lowercase()
            .contains(&query)
    });
    options
}
fn selected_index(s: &Workspace, items: &[Model]) -> usize {
    items
        .iter()
        .position(|m| Some(&m.id) == live(s).models.selected.as_ref())
        .unwrap_or(0)
}
fn choose(s: &mut Workspace, id: &str) {
    let l = live_mut(s);
    let Some(owner) = l.models.owner.clone() else {
        return;
    };
    let model = if id.is_empty() && matches!(owner, Owner::Session(..)) {
        l.app
            .choices
            .get(owner.scope())
            .and_then(|c| c.default_model.clone())
    } else {
        (!id.is_empty()).then(|| id.to_owned())
    };
    let effort = l
        .app
        .choices
        .get(owner.scope())
        .and_then(|c| c.models.iter().find(|m| Some(&m.id) == model.as_ref()))
        .and_then(|m| m.default_effort.clone());
    match owner {
        Owner::Defaults(scope) => {
            let defaults = l
                .app
                .preferences
                .providers
                .entry(scope.provider.as_str().into())
                .or_default();
            defaults.model = model;
            defaults.effort = effort;
        }
        Owner::Session(index, scope) => {
            let session = &mut l.app.sessions[index];
            if session.scope != scope || session.archived || session.archiving {
                close(s);
                return;
            }
            session.options.model = model;
            session.options.effort = effort;
            l.app.options_changed(index);
        }
    }
    close(s);
}
pub fn key(s: &mut Workspace, ev: &KeyEvent, tx: &mut Tx) -> bool {
    let items = matches(s);
    let current = selected_index(s, &items);
    match ev.key {
        Key::Down | Key::Up => {
            let next = if ev.key == Key::Down {
                (current + 1).min(items.len().saturating_sub(1))
            } else {
                current.saturating_sub(1)
            };
            live_mut(s).models.selected = items.get(next).map(|m| m.id.clone());
            live_mut(s).models.query.focused = true;
            tx.scroll_center_in_instant("model-list", format!("option{next}"));
            true
        }
        Key::Enter => {
            if let Some(m) = items.get(current).filter(|m| m.available) {
                choose(s, &m.id);
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
    let Some(scope) = l.models.owner.as_ref().map(Owner::scope) else {
        return col();
    };
    let provider = scope.provider;
    let current = match l.models.owner.as_ref().unwrap() {
        Owner::Session(i, _) => l.app.sessions[*i].options.model.as_deref(),
        Owner::Defaults(scope) => l
            .app
            .preferences
            .providers
            .get(scope.provider.as_str())
            .and_then(|o| o.model.as_deref()),
    }
    .unwrap_or_default()
    .to_owned();
    let items = std::rc::Rc::new(matches(s));
    let selected = selected_index(s, &items);
    let count = items.len();
    let kit = k.clone();
    let rows = col()
        .key("model-list")
        .w_full()
        .growth(rune::GrowthSpec::default())
        .virtual_rows(
            count,
            56.0,
            |_: &Workspace, i| format!("option{i}").into(),
            move |s: &Workspace, i| {
                let m = &items[i];
                let id = m.id.clone();
                let favorite = id.clone();
                let is_favorite = live(s)
                    .app
                    .preferences
                    .favorite_models
                    .get(provider.as_str())
                    .is_some_and(|f| f.contains(&id));
                let mut entry = row().w_full().h(56.0).gap(4.0).center_y().child(
                    ui::choice(&m.name, &m.detail, m.id == current, &kit)
                        .h(52.0)
                        .grow(1.0)
                        .min_w(0.0)
                        .fill(if i == selected {
                            kit.alpha(kit.c.text_accent, 0.10)
                        } else {
                            rune::Color::TRANSPARENT
                        })
                        .disabled(!m.available)
                        .focusable(m.available)
                        .opacity(if m.available { 1.0 } else { 0.55 })
                        .on_press(move |s: &mut Workspace| choose(s, &id)),
                );
                if !m.id.is_empty() {
                    entry = entry.child(
                        ui::Button::new(if is_favorite { "★" } else { "☆" })
                            .variant(ButtonVariant::GhostMuted)
                            .on_press(move |s: &mut Workspace| {
                                let l = live_mut(s);
                                let favorites = l
                                    .app
                                    .preferences
                                    .favorite_models
                                    .entry(provider.as_str().into())
                                    .or_default();
                                if favorites.contains(&favorite) {
                                    favorites.retain(|id| id != &favorite);
                                } else {
                                    favorites.push(favorite.clone());
                                }
                                l.models.selected = Some(favorite.clone());
                                l.models.query.focused = true;
                                let at = selected_index(s, &matches(s));
                                live_mut(s).models.reveal = Some(at);
                            })
                            .build(&kit)
                            .self_center()
                            .key(format!("model-favorite-{i}"))
                            .label(if is_favorite {
                                "Remove favourite"
                            } else {
                                "Favourite model"
                            }),
                    );
                }
                entry
            },
        );
    let mut body = col().w_full().h_full().min_h(0.0).gap(10.0).child(
        ui::field(
            &l.models.query,
            "Search models",
            "Search models, providers or context capacity…",
            k,
        )
        .key("model-search")
        .on_edit(|s: &mut Workspace| &mut live_mut(s).models.query)
        .on_input_change(|s: &mut Workspace, tx| {
            live_mut(s).models.selected = None;
            tx.scroll_to_y("model-scroll", 0.0);
        })
        .on_input_key(key),
    );
    if let Some(error) = l.app.notices.get(scope) {
        body = body.child(ui::notice(
            &format!("{} unavailable", ui::provider_name(provider)),
            error,
            k,
        ));
    }
    body.child(if count == 0 {
        ui::empty("No matching models", "Try a model name or provider.", k).grow(1.0)
    } else {
        col()
            .key("model-scroll")
            .w_full()
            .grow(1.0)
            .min_h(0.0)
            .scroll(ScrollSpec {
                stick_bottom: false,
            })
            .child(rows)
            .scroll_bar(k)
    })
    .child(ui::divider(k))
    .child(ui::caption(
        format!(
            "{} · {count} results · ↑↓ Navigate · ↵ Choose · Esc Close",
            ui::provider_name(provider)
        ),
        k,
    ))
}

use rune::{Env, Key, KeyEvent, Mods, Runtime, runtime::ElementInfo};

use super::{Workspace, spec};

mod composer;
mod context;
mod files;
mod layout;
mod lifecycle;
mod models;
mod navigation;
mod questions;
mod review;
mod sessions;
mod settings;
mod smoke;
mod tools;
mod transcript;
mod worktrees;

fn runtime(size: (f32, f32)) -> Runtime<Workspace> {
    let app = spec();
    let mut rt = Runtime::new((app.init)(), app.view, size, Env::new(true, 1.0));
    rt.poll = app.poll;
    rt.on_key = app.on_key;
    rt.set_font_families(app.families.0, app.families.1);
    advance(&mut rt, 0.5);
    rt
}

fn advance(rt: &mut Runtime<Workspace>, seconds: f64) {
    for _ in 0..(seconds * 60.0).ceil() as usize {
        rt.frame(1.0 / 60.0);
        if let Some(focused) = crate::live::presented_focus(rt) {
            rt.update(|s, _| s.live.as_mut().unwrap().focus_key = focused);
        }
    }
}

fn element(rt: &Runtime<Workspace>, key: &str) -> ElementInfo {
    rt.elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some(key) && !e.exiting)
        .unwrap_or_else(|| panic!("missing {key}"))
}

fn click(rt: &mut Runtime<Workspace>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("missing {key}"));
    rt.pointer_move(x, y);
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    advance(rt, 0.1);
}

fn key(rt: &mut Runtime<Workspace>, key: Key, mods: Mods) {
    rt.key(KeyEvent { key, mods });
    rt.frame(1.0 / 60.0);
}

fn fill(rt: &mut Runtime<Workspace>, field: &str, value: &str) {
    click(rt, field);
    key(
        rt,
        Key::Command('a'),
        Mods {
            cmd: true,
            ..Default::default()
        },
    );
    rt.paste(value);
    advance(rt, 0.1);
}

fn secondary(rt: &mut Runtime<Workspace>, target: &str) {
    let (x, y) = rt
        .find(target)
        .unwrap_or_else(|| panic!("missing {target}"));
    rt.pointer_move(x, y);
    rt.secondary_down(x, y);
    rt.secondary_up(x, y);
    advance(rt, 0.2);
    assert!(rt.popup_menu().is_none(), "context menus are app-owned");
    assert!(rt.find("context-menu").is_some());
}

fn context_item(rt: &Runtime<Workspace>, label: &str) -> ElementInfo {
    rt.elements()
        .into_iter()
        .find(|e| {
            !e.exiting
                && e.role == "menuitem"
                && e.path.contains("context-menu")
                && e.label.as_deref() == Some(label)
        })
        .unwrap_or_else(|| panic!("missing context action {label}"))
}

fn choose_context(rt: &mut Runtime<Workspace>, label: &str) {
    let key = context_item(rt, label).key.unwrap();
    click(rt, &key);
    advance(rt, 0.3);
    assert!(!crate::context::is_open(&rt.state));
}

fn design_runtime(scene: &str, size: (f32, f32)) -> Runtime<Workspace> {
    let app = spec();
    let mut state = crate::evaluation::design();
    crate::evaluation::scene(&mut state, scene);
    let mut rt = Runtime::new(state, app.view, size, Env::new(true, 1.0));
    rt.poll = app.poll;
    rt.on_key = app.on_key;
    advance(&mut rt, 0.4);
    rt
}

fn live_runtime() -> Runtime<Workspace> {
    live_runtime_size((1280.0, 1000.0))
}

fn live_runtime_size(size: (f32, f32)) -> Runtime<Workspace> {
    use caelum_core::application::{Provider, Scope};
    let mut state = Workspace::live(None);
    let live = state.live.as_mut().unwrap();
    live.app.projects.push(caelum_protocol::ProjectInfo {
        name: "Project".into(),
        path: "/project".into(),
    });
    for provider in Provider::ALL {
        live.app.new_session(Scope {
            project: "/project".into(),
            provider,
        });
    }
    live.app.select(0);
    live.poll(&mut rune::Tx::default());
    let app = spec();
    let mut rt = Runtime::new(state, app.view, size, Env::new(true, 1.0));
    rt.poll = app.poll;
    rt.on_key = app.on_key;
    rt.set_font_families(app.families.0, app.families.1);
    advance(&mut rt, 0.5);
    rt
}

fn close_session(rt: &mut Runtime<Workspace>, index: usize) {
    let [x, y, width, height] = element(rt, &format!("live-session-{index}")).rect;
    // The hover close control is the trailing 20-point slot of this row.
    let (x, y) = (x + width - 18.0, y + height / 2.0);
    rt.pointer_move(x, y);
    rt.frame(1.0 / 60.0);
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    advance(rt, 0.3);
}

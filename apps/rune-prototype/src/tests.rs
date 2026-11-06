use rune::{Env, Key, KeyEvent, Mods, Runtime, runtime::ElementInfo};

use super::{Workspace, spec};

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

#[test]
fn editing_sending_background_streaming_and_stopping_stay_session_scoped() {
    let mut rt = runtime((1280.0, 1000.0));
    let original = rt.state.active().draft.text.clone();
    click(&mut rt, "session-1");
    click(&mut rt, "draft-1");
    rt.paste("Review café ☁️");
    key(
        &mut rt,
        Key::Enter,
        Mods {
            shift: true,
            ..Mods::default()
        },
    );
    rt.paste("Keep the draft isolated.");
    key(&mut rt, Key::Enter, Mods::default());
    assert_eq!(
        rt.state.active().turns[0].prompt,
        "Review café ☁️\nKeep the draft isolated."
    );
    assert!(rt.state.active().working());
    assert!(rt.state.active().draft.text.is_empty());

    rt.paste("Next message");
    key(&mut rt, Key::Enter, Mods::default());
    assert_eq!(
        rt.state.active().turns.len(),
        1,
        "no second send during a stream"
    );
    click(&mut rt, "session-0");
    advance(&mut rt, 6.0);
    assert_eq!(rt.state.active().draft.text, original);
    assert!(!rt.state.sessions[1].working());
    assert!(
        rt.state.sessions[1].turns[0]
            .response
            .ends_with("sample patch.")
    );

    click(&mut rt, "session-1");
    assert_eq!(rt.state.active().draft.text, "Next message");
    click(&mut rt, "send");
    advance(&mut rt, 0.5);
    click(&mut rt, "send");
    let stopped = rt.state.active().turns[1].response.clone();
    assert!(!stopped.is_empty());
    assert!(rt.state.active().turns[1].stopped);
    advance(&mut rt, 6.0);
    assert_eq!(
        rt.state.active().turns[1].response,
        stopped,
        "stop cancels future chunks"
    );
    click(&mut rt, "draft-1");
    rt.paste("Try the keyboard stop shortcut");
    key(&mut rt, Key::Enter, Mods::default());
    advance(&mut rt, 0.2);
    key(&mut rt, Key::Escape, Mods::default());
    assert!(!rt.state.active().working());
    assert!(rt.state.active().turns[2].stopped);
}

#[test]
fn panes_theme_tools_and_diff_preserve_the_conversation_and_draft() {
    let mut rt = runtime((1280.0, 1000.0));
    let draft = rt.state.active().draft.text.clone();
    let response = rt.state.active().turns[0].response.clone();
    let clipboard = rune::clipboard::memory();
    rt.use_clipboard(&clipboard);
    click(&mut rt, "copy-0");
    assert_eq!(
        clipboard.text().as_deref().map(String::as_str),
        Some(response.as_str())
    );
    click(&mut rt, "tool-0");
    assert!(rt.state.active().turns[0].expanded);
    click(&mut rt, "theme");
    assert_eq!(rt.state.dark, Some(false));
    click(&mut rt, "inspector-toggle");
    advance(&mut rt, 0.5);
    assert!(rt.state.inspector.is_open());
    click(&mut rt, "diff-mode");
    assert_eq!(rt.state.diff.mode(), rune_kit::diff::DiffMode::Split);

    let (x, y) = rt.find("sidebar-grip").unwrap();
    rt.pointer_down(x, y);
    rt.pointer_move(x + 40.0, y);
    rt.pointer_up(x + 40.0, y);
    advance(&mut rt, 0.5);
    assert_eq!(rt.state.sidebar.width(), 278.0);
    key(
        &mut rt,
        Key::Command('b'),
        Mods {
            cmd: true,
            shift: true,
            ..Mods::default()
        },
    );
    advance(&mut rt, 0.5);
    assert!(!rt.state.sidebar.is_open());
    click(&mut rt, "sidebar-toggle");
    advance(&mut rt, 0.5);
    assert!(rt.state.sidebar.is_open());
    assert_eq!(rt.state.sidebar.width(), 278.0);
    assert_eq!(rt.state.active().draft.text, draft);
    assert_eq!(rt.state.active().turns[0].response, response);
}

#[test]
fn compact_layout_keeps_the_composer_and_pane_controls_visible() {
    let mut rt = runtime((800.0, 620.0));
    click(&mut rt, "inspector-toggle");
    advance(&mut rt, 0.5);
    for key in [
        "draft-0",
        "send",
        "sidebar-toggle",
        "inspector-toggle",
        "close-inspector",
    ] {
        let e = element(&rt, key);
        assert!(
            e.rect[2] > 0.0 && e.rect[3] > 0.0,
            "{key} has a usable size"
        );
        for (actual, visible) in e.rect.into_iter().zip(e.visible) {
            assert!(
                (actual - visible).abs() < 1.0,
                "{key} is clipped: {:?} vs {:?}",
                e.rect,
                e.visible
            );
        }
    }
    click(&mut rt, "new-session");
    rt.paste("A new draft");
    rt.frame(1.0 / 60.0);
    assert_eq!(rt.state.selected, 2);
    assert_eq!(
        rt.state.active().draft.text,
        "A new draft",
        "new sessions focus their composer"
    );
}

#[test]
fn scrolling_up_during_a_reply_preserves_the_reading_position() {
    let mut rt = runtime((1000.0, 700.0));
    click(&mut rt, "send");
    advance(&mut rt, 1.0);
    assert_eq!(rt.scroll_at_bottom("transcript-0"), Some(true));
    let (x, y) = rt.find("transcript-0").unwrap();
    rt.wheel(x, y, 250.0);
    advance(&mut rt, 0.1);
    assert_eq!(rt.scroll_at_bottom("transcript-0"), Some(false));
    let reading = element(&rt, "turn-0").rect[1];
    advance(&mut rt, 6.0);
    assert_eq!(element(&rt, "turn-0").rect[1], reading);
    rt.wheel(x, y, -10000.0);
    advance(&mut rt, 0.5);
    assert_eq!(rt.scroll_at_bottom("transcript-0"), Some(true));
}

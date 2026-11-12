use super::*;

#[test]
fn command_completions_overlay_background_output_without_moving_the_editor() {
    for size in [(800.0, 620.0), (1280.0, 1000.0)] {
        for scene in ["shells", "shells-many"] {
            let mut rt = design_runtime(scene, size);
            fill(&mut rt, "live-draft-0", "");
            let editor = element(&rt, "composer-editor").rect;
            let shells = element(&rt, "background-shells").rect;
            rt.paste("/");
            advance(&mut rt, 0.2);

            let menu = element(&rt, "completion-menu");
            assert!(
                menu.rect
                    .iter()
                    .zip(menu.visible)
                    .all(|(a, b)| (*a - b).abs() < 1.0),
                "completion menu is clipped: {menu:?}"
            );
            assert!(menu.rect[1] + menu.rect[3] <= editor[1] + 1.0);
            assert!(menu.rect[1] < shells[1] + shells[3]);
            assert!(menu.rect[1] + menu.rect[3] > shells[1]);
            assert_eq!(element(&rt, "composer-editor").rect, editor);
            assert_eq!(element(&rt, "background-shells").rect, shells);
            assert!(element(&rt, "live-draft-0").focused);

            let choice = element(&rt, "completion-1");
            assert!(
                choice
                    .rect
                    .iter()
                    .zip(choice.visible)
                    .all(|(a, b)| (*a - b).abs() < 1.0)
            );
            let (x, y) = rt.find("completion-1").unwrap();
            rt.pointer_move(x, y);
            rt.pointer_down(x, y);
            advance(&mut rt, 0.1);
            assert!(element(&rt, "live-draft-0").focused);
            rt.pointer_up(x, y);
            advance(&mut rt, 0.3);
            let live = rt.state.live.as_ref().unwrap();
            assert_eq!(live.drafts[0].text.trim(), "/sessions");
            assert!(live.shell_panel.is_none());
            assert!(rt.find("completion-menu").is_none());
            assert!(element(&rt, "live-draft-0").focused);
        }
    }
}

#[test]
fn command_completions_scroll_complete_run_and_dismiss_over_background_output() {
    let mut rt = design_runtime("shells-many", (800.0, 620.0));
    fill(&mut rt, "live-draft-0", "/");
    for _ in 0..8 {
        key(&mut rt, Key::Down, Mods::default());
    }
    advance(&mut rt, 0.2);
    let choice = element(&rt, "completion-8");
    assert_eq!(choice.label.as_deref(), Some("/settings"));
    assert!(
        choice
            .rect
            .iter()
            .zip(choice.visible)
            .all(|(a, b)| (*a - b).abs() < 1.0)
    );
    key(&mut rt, Key::Tab, Mods::default());
    assert_eq!(
        rt.state.live.as_ref().unwrap().drafts[0].text.trim(),
        "/settings"
    );

    fill(&mut rt, "live-draft-0", "/");
    key(&mut rt, Key::Escape, Mods::default());
    advance(&mut rt, 0.3);
    assert!(rt.find("completion-menu").is_none());
    assert!(element(&rt, "live-draft-0").focused);
    assert_eq!(rt.state.live.as_ref().unwrap().drafts[0].text, "/");

    fill(&mut rt, "live-draft-0", "/models");
    key(&mut rt, Key::Enter, Mods::default());
    advance(&mut rt, 0.3);
    let live = rt.state.live.as_ref().unwrap();
    assert!(live.models.is_open());
    assert_eq!(live.app.sessions[0].shells.len(), 8);
    assert!(!live.app.sessions[0].stopping);
}

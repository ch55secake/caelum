use super::*;
use caelum_core::application::{Attachment, Event};

mod controls;
mod editing;

fn position(rt: &Runtime<Workspace>, index: usize) -> f32 {
    rt.state.live.as_ref().unwrap().composer.editors[index]
        .position
        .get()
        .offset[1]
}

#[test]
fn growing_composer_leaves_transcript_space_and_all_controls_reachable() {
    for size in [(800.0, 620.0), (1280.0, 1000.0)] {
        for inspector in [false, true] {
            let mut rt = design_runtime("composer-long", size);
            rt.update(|s, _| {
                s.inspector.set_open(inspector);
                let l = s.live.as_mut().unwrap();
                l.app.sessions[0].conversation.working = true;
                l.app.sessions[0].queue = (0..3)
                    .map(|i| caelum_core::application::QueuedPrompt {
                        text: format!("Follow-up {i}"),
                        options: Default::default(),
                    })
                    .collect();
                l.expanded_queues.insert(0);
            });
            advance(&mut rt, 0.4);
            let transcript = element(&rt, "live-transcript-0").rect;
            let composer = element(&rt, "message-composer").rect;
            assert!(
                transcript[3] >= 90.0,
                "{size:?}, inspector={inspector}: {transcript:?}"
            );
            assert!(transcript[1] + transcript[3] <= composer[1] + 1.0);
            for name in [
                "message-composer",
                "composer-editor",
                "send",
                "stop",
                "model-trigger",
                "effort-trigger",
                "agent-trigger",
            ] {
                let e = element(&rt, name);
                assert!(
                    e.rect
                        .iter()
                        .zip(e.visible)
                        .all(|(a, b)| (*a - b).abs() < 1.0),
                    "{size:?}, inspector={inspector}: {e:?}"
                );
            }
            assert!(rt.find("expand-composer").is_none());
            assert!(element(&rt, "composer-context-0").rect[3] <= 180.1);
            let grown = element(&rt, "composer-editor").rect[3];
            // The long input's un-clipped centre is below its scroll viewport.
            click(&mut rt, "composer-editor");
            key(
                &mut rt,
                Key::Command('a'),
                Mods {
                    cmd: true,
                    ..Default::default()
                },
            );
            rt.paste("Short prompt");
            advance(&mut rt, 0.5);
            assert_eq!(
                rt.state.live.as_ref().unwrap().drafts[0].text,
                "Short prompt"
            );
            assert!(element(&rt, "composer-editor").rect[3] <= 56.1);
            if size.1 > 620.0 {
                assert!(grown > 300.0);
            }
        }
    }
}

#[test]
fn composer_resize_preserves_unicode_selection_scroll_and_undo() {
    let mut rt = design_runtime("composer-multiline", (1280.0, 1000.0));
    let prompt = "Keep café ☁️ and its selection.\n".repeat(80);
    fill(&mut rt, "live-draft-0", &prompt);
    rt.update(|s, tx| {
        let draft = &mut s.live.as_mut().unwrap().drafts[0];
        draft.anchor = prompt.find("café").unwrap();
        draft.focus = draft.anchor + "café ☁️".len();
        tx.scroll_to_y("composer-editor", 260.0);
    });
    advance(&mut rt, 0.3);
    let selection = {
        let draft = &rt.state.live.as_ref().unwrap().drafts[0];
        (draft.anchor, draft.focus)
    };
    let before = position(&rt, 0);
    assert!(before > 200.0);
    for size in [(1280.0, 800.0), (1280.0, 1000.0), (1280.0, 760.0)] {
        rt.set_viewport(size, 1.0);
        advance(&mut rt, 0.3);
        let l = rt.state.live.as_ref().unwrap();
        assert_eq!((l.drafts[0].anchor, l.drafts[0].focus), selection);
        assert_eq!(l.drafts[0].text, prompt);
        assert!(element(&rt, "live-draft-0").focused);
        assert!(
            (position(&rt, 0) - before).abs() < 1.0,
            "{} -> {}",
            before,
            position(&rt, 0)
        );
    }
    rt.paste("replacement");
    advance(&mut rt, 0.1);
    key(
        &mut rt,
        Key::Command('z'),
        Mods {
            cmd: true,
            ..Default::default()
        },
    );
    advance(&mut rt, 0.1);
    assert_eq!(rt.state.live.as_ref().unwrap().drafts[0].text, prompt);
    // Window-aware resizing must not replace the editor's earlier history.
    key(
        &mut rt,
        Key::Command('z'),
        Mods {
            cmd: true,
            ..Default::default()
        },
    );
    advance(&mut rt, 0.1);
    assert_ne!(rt.state.live.as_ref().unwrap().drafts[0].text, prompt);
}

#[test]
fn composer_draft_height_and_scroll_are_session_owned() {
    let mut rt = design_runtime("composer-long", (1280.0, 1000.0));
    fill(&mut rt, "live-draft-0", &"First draft café\n".repeat(80));
    rt.update(|_, tx| tx.scroll_to_y("composer-editor", 230.0));
    advance(&mut rt, 0.2);
    let first = position(&rt, 0);
    click(&mut rt, "live-session-1");
    advance(&mut rt, 0.4);
    assert!(element(&rt, "composer-editor").rect[3] <= 56.1);
    fill(
        &mut rt,
        "live-draft-1",
        &"Second independent draft\n".repeat(80),
    );
    rt.update(|_, tx| tx.scroll_to_y("composer-editor", 460.0));
    advance(&mut rt, 0.2);
    let second = position(&rt, 1);
    click(&mut rt, "live-session-0");
    advance(&mut rt, 0.3);
    assert!(element(&rt, "composer-editor").rect[3] > 180.0);
    assert!(
        (position(&rt, 0) - first).abs() < 1.0,
        "first: {} -> {}",
        first,
        position(&rt, 0)
    );
    click(&mut rt, "live-session-1");
    advance(&mut rt, 0.3);
    assert!(
        (position(&rt, 1) - second).abs() < 1.0,
        "second: {} -> {}",
        second,
        position(&rt, 1)
    );
    rt.update(|s, _| s.live.as_mut().unwrap().panels.screen = crate::panels::Screen::Review);
    advance(&mut rt, 0.2);
    assert!(rt.find("composer-editor").is_none());
    click(&mut rt, "back-to-session");
    advance(&mut rt, 0.2);
    assert!((position(&rt, 1) - second).abs() < 1.0);
}

#[test]
fn attachment_preview_copies_the_captured_snapshot_and_removal_is_separate() {
    let mut rt = design_runtime("composer-attachment-status", (800.0, 620.0));
    let clipboard = rune::clipboard::memory();
    rt.use_clipboard(&clipboard);
    let snapshot = rt.state.live.as_ref().unwrap().app.sessions[0].attachments[0].clone();
    let prompt = "Keep this draft café\n".repeat(60);
    fill(&mut rt, "live-draft-0", &prompt);
    key(
        &mut rt,
        Key::Left,
        Mods {
            shift: true,
            ..Default::default()
        },
    );
    let selected = {
        let draft = &rt.state.live.as_ref().unwrap().drafts[0];
        (draft.anchor, draft.focus)
    };
    rt.update(|_, tx| tx.scroll_to_y("composer-editor", 210.0));
    advance(&mut rt, 0.2);
    let offset = position(&rt, 0);
    click(&mut rt, "composer-attachment-source");
    assert_eq!(
        rt.state.live.as_ref().unwrap().app.sessions[0]
            .attachments
            .len(),
        2
    );
    let dialog = element(&rt, "workspace-dialog");
    assert_eq!(dialog.label.as_deref(), Some("composer.rs"));
    assert!(dialog.visible[3] < 300.0);
    let viewport = element(&rt, "attachment-preview-scroll");
    let content = element(&rt, "attachment-preview-text");
    assert!(content.path.starts_with(&format!("{}/", viewport.path)));
    assert!((content.rect[1] - viewport.rect[1] - 12.0).abs() < 1.0);
    assert_eq!(
        element(&rt, "attachment-preview-lines").text.as_deref(),
        Some("40\n41\n42\n43\n44")
    );
    assert!(
        rt.elements()
            .iter()
            .any(|e| { e.text.as_deref().is_some_and(|t| t == "Partial") })
    );
    // Even if the original draft's attachment is replaced while the reader is
    // open, the captured preview and its Copy action must remain immutable.
    rt.update(|s, _| {
        s.live.as_mut().unwrap().app.attach(
            0,
            Attachment::new(
                snapshot.path.clone(),
                "newer source".into(),
                Some((40, 44)),
                false,
            ),
        );
    });
    click(&mut rt, "copy-attachment-snapshot");
    assert_eq!(clipboard.text().unwrap().as_str(), snapshot.content);
    assert_eq!(
        element(&rt, "attachment-preview-text").text.as_deref(),
        Some(
            snapshot
                .content
                .strip_suffix('\n')
                .unwrap_or(&snapshot.content)
        )
    );
    key(&mut rt, Key::Escape, Mods::default());
    advance(&mut rt, 0.2);
    assert!(element(&rt, "live-draft-0").focused);
    let l = rt.state.live.as_ref().unwrap();
    assert!(l.composer.preview.is_none());
    assert_eq!((l.drafts[0].anchor, l.drafts[0].focus), selected);
    assert_eq!(l.drafts[0].text, prompt);
    assert!((position(&rt, 0) - offset).abs() < 1.0);
    click(&mut rt, "remove-attachment-image");
    assert!(rt.find("workspace-dialog").is_none());
    assert_eq!(
        rt.state.live.as_ref().unwrap().app.sessions[0]
            .attachments
            .len(),
        1
    );
}

#[test]
fn attachment_status_is_named_dismissible_and_loading_blocks_submission() {
    let mut rt = design_runtime("composer-attachment-status", (800.0, 620.0));
    let request = rt.state.live.as_ref().unwrap().app.attachment_reads(0)[0].0;
    assert!(rt.find(&format!("attachment-loading-{request}")).is_some());
    assert!(!element(&rt, "send").focusable);
    click(&mut rt, "dismiss-attachment-error-0-0");
    assert!(
        rt.state.live.as_ref().unwrap().app.sessions[0]
            .attachment_errors
            .is_empty()
    );
    rt.update(|s, _| {
        s.live.as_mut().unwrap().apply(Event::Completed(
            request,
            Err("The file is unavailable".into()),
        ))
    });
    advance(&mut rt, 0.2);
    assert!(rt.find(&format!("attachment-loading-{request}")).is_none());
    assert!(element(&rt, "send").focusable);
    assert!(
        rt.elements()
            .iter()
            .any(|e| e.text.as_deref() == Some("Could not attach docs/design/notes.md"))
    );
    click(&mut rt, "live-session-1");
    assert!(rt.find("attachment-error-0-0").is_none());
    click(&mut rt, "live-session-0");
    assert!(rt.find("attachment-error-0-0").is_some());
}

#[test]
fn image_snapshot_uses_image_viewer_and_session_change_closes_preview() {
    let mut rt = design_runtime("composer-attachments", (800.0, 620.0));
    let snapshot = rt.state.live.as_ref().unwrap().app.sessions[0].attachments[1].clone();
    click(&mut rt, "composer-attachment-image");
    assert!(element(&rt, "image-canvas").visible[3] > 100.0);
    assert_eq!(
        rt.state
            .live
            .as_ref()
            .unwrap()
            .media
            .selected
            .as_ref()
            .unwrap()
            .1
            .source,
        snapshot.content
    );
    assert_eq!(
        rt.state.live.as_ref().unwrap().app.sessions[0]
            .attachments
            .len(),
        2
    );
    rt.update(|s, _| s.live.as_mut().unwrap().app.select(1));
    advance(&mut rt, 0.3);
    assert!(rt.find("workspace-dialog").is_none());
    assert!(rt.state.live.as_ref().unwrap().composer.preview.is_none());
}

#[test]
fn image_decode_failure_is_visible_and_does_not_remove_the_attachment() {
    let mut rt = design_runtime("composer-attachments", (800.0, 620.0));
    rt.update(|s, _| {
        s.live.as_mut().unwrap().app.sessions[0].attachments[1].content =
            "data:image/png;base64,AQ==".into();
    });
    advance(&mut rt, 0.2);
    assert!(
        rt.elements()
            .iter()
            .any(|e| e.label.as_deref() == Some("Image preview unavailable"))
    );
    click(&mut rt, "composer-attachment-image");
    assert!(
        rt.elements()
            .iter()
            .any(|e| e.text.as_deref() == Some("Image unavailable"))
    );
    assert_eq!(
        rt.state.live.as_ref().unwrap().app.sessions[0]
            .attachments
            .len(),
        2
    );
    key(&mut rt, Key::Escape, Mods::default());
    advance(&mut rt, 0.2);
    assert!(element(&rt, "live-draft-0").focused);
}

#[test]
fn long_attachment_snapshot_scrolls_inside_the_preview() {
    let mut rt = design_runtime("composer-attachments", (800.0, 620.0));
    rt.update(|s, _| {
        s.live.as_mut().unwrap().app.sessions[0].attachments[0].content =
            "Captured source line café\n".repeat(300);
    });
    click(&mut rt, "composer-attachment-source");
    let before = element(&rt, "attachment-preview-text").rect[1];
    let (x, y) = rt.find("attachment-preview-scroll").unwrap();
    rt.wheel(x, y, -240.0);
    advance(&mut rt, 0.2);
    let viewport = element(&rt, "attachment-preview-scroll");
    let content = element(&rt, "attachment-preview-text");
    assert!(content.rect[1] < before - 200.0);
    assert!(content.visible[1] >= viewport.rect[1]);
    assert!(content.visible[3] <= viewport.rect[3] + 1.0);
    assert!(element(&rt, "copy-attachment-snapshot").visible[3] >= 28.0);
}

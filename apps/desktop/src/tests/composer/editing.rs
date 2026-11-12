use super::*;

#[test]
fn composer_typing_during_streaming_preserves_transcript_reading_position() {
    use caelum_core::application::{Event, Provider};
    let mut rt = live_runtime_size((800.0, 620.0));
    rt.update(|s, _| {
        let session = &mut s.live.as_mut().unwrap().app.sessions[0];
        session.remote_id = Some("streaming".into());
        let turns: Vec<_> = (0..30).map(|n| serde_json::json!({"id":format!("turn-{n}"),"status":"completed","items":[
            {"id":format!("user-{n}"),"type":"userMessage","content":[{"text":format!("Review change {n}")}]},
            {"id":format!("reply-{n}"),"type":"agentMessage","text":"Keep each session’s draft and reading position independent."}
        ]})).collect();
        session.conversation.history(Provider::Codex, &turns, false);
        session.conversation.working = true;
    });
    fill(&mut rt, "live-draft-0", "My next prompt");
    let (x, y) = rt.find("live-transcript-0").unwrap();
    rt.wheel(x, y, 240.0);
    advance(&mut rt, 0.2);
    assert_eq!(rt.scroll_at_bottom("live-transcript-0"), Some(false));
    let reading = rt.state.live.as_ref().unwrap().app.sessions[0].reading;
    for _ in 0..12 {
        rt.update(|s, _| {
            s.live.as_mut().unwrap().apply(Event::Codex(caelum_protocol::CodexEvent {
                project_path: "/project".into(), method: "item/agentMessage/delta".into(),
                params: serde_json::json!({"threadId":"streaming","itemId":"active-reply","delta":"Another streamed update. "}), request_id: None,
            }));
        });
        rt.paste(" café");
        advance(&mut rt, 0.05);
    }
    assert!(element(&rt, "live-draft-0").focused);
    assert_eq!(
        rt.state.live.as_ref().unwrap().drafts[0].text,
        format!("My next prompt{}", " café".repeat(12))
    );
    assert_eq!(
        rt.state.live.as_ref().unwrap().app.sessions[0].reading,
        reading
    );
    let transcript = element(&rt, "live-transcript-0").rect;
    assert!(transcript[1] + transcript[3] <= element(&rt, "message-composer").rect[1] + 1.0);
}

#[test]
fn composer_content_stays_docked_and_bounded_in_both_themes() {
    for dark in [false, true] {
        for scene in [
            "composer-multiline",
            "composer-queued",
            "composer-attachments",
        ] {
            let mut rt = design_runtime(scene, (800.0, 620.0));
            rt.update(|s, _| {
                s.dark = Some(dark);
                s.inspector.set_open(true);
            });
            advance(&mut rt, 0.4);
            if scene == "composer-queued" {
                click(&mut rt, "toggle-queued-messages");
            }
            for key in [
                "message-composer",
                "composer-editor",
                "model-trigger",
                "effort-trigger",
                "agent-trigger",
                "send",
            ] {
                let e = element(&rt, key);
                assert!(
                    e.rect
                        .iter()
                        .zip(e.visible)
                        .all(|(a, b)| (*a - b).abs() < 1.0),
                    "{scene}: {e:?}"
                );
            }
            for attachment in rt.state.live.as_ref().unwrap().app.sessions[0]
                .attachments
                .iter()
            {
                let e = element(&rt, &format!("composer-attachment-{}", attachment.id));
                assert!(
                    e.rect
                        .iter()
                        .zip(e.visible)
                        .all(|(a, b)| (*a - b).abs() < 1.0),
                    "{e:?}"
                );
            }
            let composer = element(&rt, "message-composer").rect;
            let transcript = element(&rt, "live-transcript-0").rect;
            assert!(transcript[3] > 40.0, "{scene}: {transcript:?}");
            assert!(transcript[1] + transcript[3] <= composer[1] + 1.0);
            assert!(element(&rt, "composer-editor").rect[3] <= 420.1);
        }
    }
}

#[test]
fn long_drafts_and_crowded_sidebar_keep_controls_in_bounds() {
    let mut rt = live_runtime_size((800.0, 620.0));
    rt.update(|s, _| {
        let l = s.live.as_mut().unwrap();
        for n in 0..35 {
            let i = l.app.new_session(caelum_core::application::Scope {
                project: "/project".into(),
                provider: caelum_core::application::Provider::Codex,
            });
            l.app.sessions[i].title = format!("Session {n}");
        }
        l.app.select(0);
        l.poll(&mut rune::Tx::default());
        l.app.archive(2, true);
    });
    advance(&mut rt, 0.3);
    fill(
        &mut rt,
        "live-draft-0",
        &"A long Unicode draft café ☁️\n".repeat(30),
    );
    for name in ["nav-settings", "undo-archive", "send", "model-trigger"] {
        let e = element(&rt, name);
        assert!(
            e.visible[2] > 0.0 && e.visible[3] > 0.0,
            "{name} is clipped"
        );
        assert!(
            e.rect[1] >= 0.0 && e.rect[1] + e.rect[3] <= 620.1,
            "{name} outside the window: {:?}",
            e.rect
        );
    }
    let editor = element(&rt, "composer-editor");
    let send = element(&rt, "send");
    let transcript = element(&rt, "live-transcript-0");
    let composer = element(&rt, "message-composer");
    assert!(editor.rect[3] <= 420.1);
    assert!(transcript.rect[3] >= 140.0);
    assert!(transcript.rect[1] + transcript.rect[3] <= composer.rect[1] + 1.0);
    assert!(editor.rect[1] + editor.rect[3] <= send.rect[1] + 1.0);
    assert!(
        rt.state.live.as_ref().unwrap().drafts[0]
            .text
            .ends_with("café ☁️\n")
    );
    click(&mut rt, "nav-settings");
    advance(&mut rt, 0.2);
    for page in [
        "providers",
        "worktrees",
        "notifications",
        "about",
        "appearance",
    ] {
        click(&mut rt, &format!("settings-{page}"));
        assert!(element(&rt, &format!("settings-{page}")).visible[3] > 0.0);
    }
    key(&mut rt, Key::Escape, Mods::default());
    assert_eq!(
        rt.state.live.as_ref().unwrap().drafts[0]
            .text
            .lines()
            .count(),
        30
    );
}

#[test]
fn native_mentions_replace_only_the_caret_token_and_escape_cancels_pending_reads() {
    use caelum_core::application::{Command, Event, Reply, Task, TaskReply};
    let mut rt = live_runtime();
    fill(&mut rt, "live-draft-0", "Before café @ma after");
    rt.update(|s, _| {
        let l = s.live.as_mut().unwrap();
        let at = l.drafts[0].text.find(" after").unwrap();
        l.drafts[0].set_caret(at);
    });
    advance(&mut rt, 0.1);
    let requests = rt.state.live.as_mut().unwrap().app.commands();
    let (search, _) = requests
        .into_iter()
        .find(|(_, c)| matches!(c, Command::Task(Task::Completions { .. })))
        .unwrap();
    let results=serde_json::from_value(serde_json::json!({"query":"ma","matches":[{"path":"main.rs","line":null,"preview":null}],"truncated":false,"skipped":0})).unwrap();
    rt.update(|s, _| {
        s.live.as_mut().unwrap().apply(Event::Completed(
            search,
            Ok(Reply::Task(TaskReply::Search(results))),
        ))
    });
    advance(&mut rt, 0.2);
    key(&mut rt, Key::Tab, Mods::default());
    let (read, _) = rt
        .state
        .live
        .as_mut()
        .unwrap()
        .app
        .commands()
        .pop()
        .unwrap();
    key(&mut rt, Key::Escape, Mods::default());
    assert_eq!(rt.state.live.as_ref().unwrap().app.sessions[0].attaching, 0);
    rt.update(|s, _| {
        s.live.as_mut().unwrap().apply(Event::Completed(
            read,
            Ok(Reply::Task(TaskReply::File(
                caelum_protocol::ProjectFileContent {
                    path: "main.rs".into(),
                    content: "snapshot".into(),
                    size_bytes: 8,
                    truncated: false,
                },
            ))),
        ))
    });
    assert_eq!(
        rt.state.live.as_ref().unwrap().drafts[0].text,
        "Before café @ma after"
    );
    assert!(
        rt.state.live.as_ref().unwrap().app.sessions[0]
            .attachments
            .is_empty()
    );
}

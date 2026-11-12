use super::*;

#[test]
fn user_bubbles_size_to_content_group_images_and_copy_the_original_markdown() {
    let mut rt = design_runtime("messages", (1280.0, 1000.0));
    let clipboard = rune::clipboard::memory();
    rt.use_clipboard(&clipboard);
    let bubbles: Vec<_> = rt
        .elements()
        .into_iter()
        .filter(|e| {
            e.key
                .as_ref()
                .is_some_and(|k| k.starts_with("user-bubble-"))
                && !e.exiting
        })
        .collect();
    assert_eq!(bubbles.len(), 3);
    let short = bubbles
        .iter()
        .find(|b| b.key.as_ref().unwrap().contains("short"))
        .unwrap();
    assert!(short.rect[2] < 260.0);
    assert!(
        bubbles
            .iter()
            .all(|b| (b.rect[0] + b.rect[2] - (short.rect[0] + short.rect[2])).abs() < 1.0)
    );
    let formatted = rt
        .elements()
        .into_iter()
        .find(|e| {
            e.key
                .as_ref()
                .is_some_and(|k| k.starts_with("copy-user-") && k.contains("formatted"))
        })
        .unwrap()
        .key
        .unwrap();
    let (x, y) = rt.find(&formatted).unwrap();
    rt.pointer_move(x, y);
    advance(&mut rt, 0.2);
    click(&mut rt, &formatted);
    assert!(clipboard.text().unwrap().contains("**session owner**"));
    assert!(clipboard.text().unwrap().contains("```rust"));
    assert!(
        rt.elements()
            .iter()
            .any(|e| e.role == "img" && e.path.contains("user-bubble-"))
    );
    let file = rt
        .elements()
        .into_iter()
        .find(|e| e.text.as_deref() == Some("src/session.rs"))
        .expect("grouped file label");
    assert!(
        file.path.contains("user-bubble-") && file.visible[2] > 60.0,
        "{file:?}"
    );
}

#[test]
fn reminders_are_absent_from_transcript_copy_and_timeline_with_stable_jumps() {
    let app = spec();
    let mut state = crate::evaluation::design();
    let l = state.live.as_mut().unwrap();
    l.app.sessions[0].conversation = Default::default();
    let mut messages = vec![
        serde_json::json!({"id":"prompt","type":"user","text":"Visible café prompt"}),
        serde_json::json!({"id":"answer","type":"assistant","content":[{"id":"text","type":"text","text":"Visible answer<system-reminder>internal secret</system-reminder>"}]}),
    ];
    messages.extend((0..30).map(|i| serde_json::json!({"id":format!("hidden-{i}"),"type":"synthetic","text":"<system-reminder>internal secret</system-reminder>"})));
    l.app.sessions[0].conversation.history(
        caelum_core::application::Provider::OpenCode,
        &messages,
        false,
    );
    let mut rt = Runtime::new(state, app.view, (800.0, 620.0), Env::new(true, 1.0));
    rt.poll = app.poll;
    rt.on_key = app.on_key;
    let clipboard = rune::clipboard::memory();
    rt.use_clipboard(&clipboard);
    advance(&mut rt, 0.5);
    assert!(!rt.elements().iter().any(|e| {
        e.text
            .as_deref()
            .is_some_and(|t| t.contains("internal secret"))
    }));
    assert!(element(&rt, "live-entry-0-opencode-prompt").visible[3] > 20.0);
    click(&mut rt, "copy-response-0-opencode-answer");
    assert_eq!(
        clipboard.text().as_deref().map(String::as_str),
        Some("Visible answer")
    );
    fill(&mut rt, "live-draft-0", "/timeline");
    key(&mut rt, Key::Enter, Mods::default());
    advance(&mut rt, 0.3);
    fill(&mut rt, "timeline-search", "internal secret");
    assert!(!rt.elements().iter().any(|e| {
        !e.exiting
            && e.key
                .as_deref()
                .is_some_and(|k| k.starts_with("timeline-opencode"))
    }));
    fill(&mut rt, "timeline-search", "café");
    rt.update(|s, _| {
        s.live.as_mut().unwrap().app.sessions[0]
            .conversation
            .history(
                caelum_core::application::Provider::OpenCode,
                &[serde_json::json!({"id":"older","type":"user","text":"Earlier history"})],
                true,
            )
    });
    key(&mut rt, Key::Down, Mods::default());
    key(&mut rt, Key::Enter, Mods::default());
    advance(&mut rt, 0.5);
    assert!(rt.state.live.as_ref().unwrap().panels.overlay.is_none());
    assert!(element(&rt, "live-entry-0-opencode-prompt").visible[3] > 20.0);
}

#[test]
fn shimmer_keeps_ink_stationary_and_stops_for_reduced_motion_and_completion() {
    let _fonts = rune::text::with_families("Inter", "Inter");
    for dark in [false, true] {
        for reduced in [false, true] {
            let mut rt = Runtime::new(
                true,
                |running: &bool, cx: &mut rune::Cx<bool>| {
                    let k = crate::theme::appearance(cx.system_dark(), "vercel", "theme");
                    rune::col()
                        .padding(12.0)
                        .gap(8.0)
                        .child(
                            if *running {
                                crate::ui::busy_text("Compacting context…", k.ty.small_medium, &k)
                            } else {
                                rune::text("Context compacted").font(k.ty.small_medium)
                            }
                            .key("label"),
                        )
                        .child(
                            if *running {
                                crate::ui::busy_icon(&crate::ui::CLOCK, 14.0, &k)
                            } else {
                                crate::ui::outcome(
                                    caelum_core::application::presentation::Outcome::Success,
                                    &k,
                                )
                            }
                            .key("icon"),
                        )
                },
                (400.0, 100.0),
                Env::new(dark, 2.0),
            );
            rt.set_system_reduced_motion(reduced);
            let mut positions = Vec::new();
            for _ in 0..60 {
                rt.frame(1.0 / 60.0);
                let elements = rt.elements();
                let labels: Vec<_> = elements
                    .iter()
                    .filter(|e| !e.exiting && e.text.as_deref() == Some("Compacting context…"))
                    .collect();
                assert_eq!(labels.len(), 2);
                assert!((labels[0].rect[0] - labels[1].rect[0]).abs() < 1.0);
                assert!((labels[0].rect[1] - labels[1].rect[1]).abs() < 1.0);
                positions.push(
                    elements
                        .iter()
                        .find(|e| e.key.as_deref() == Some("shimmer-band"))
                        .unwrap()
                        .rect[0],
                );
            }
            if reduced {
                assert!((positions[10] - positions[59]).abs() < 0.1, "{positions:?}");
            } else {
                assert!(positions[59] - positions[10] > 20.0);
            }
            rt.update(|running, _| *running = false);
            for _ in 0..30 {
                rt.frame(1.0 / 60.0);
            }
            assert!(
                !rt.elements()
                    .iter()
                    .any(|e| !e.exiting && e.key.as_deref() == Some("shimmer-band"))
            );
        }
    }
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
    click(&mut rt, "stop");
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

#[test]
fn completed_assistant_response_shows_agent_and_duration_below_the_output() {
    use caelum_core::application::{Event, Provider};
    let mut rt = live_runtime();
    rt.update(|s, _| {
        let app = &mut s.live.as_mut().unwrap().app;
        {
            let session = &mut app.sessions[1];
            session.remote_id = Some("session-1".into());
            session.loaded = true;
            session.options.agent = Some("build".into());
            session
                .conversation
                .begin_turn(Some("build".into()), 1_000);
            session.conversation.optimistic("local-1".into(), "Build this".into());
        }
        app.select(1);
        for (event_type, data) in [
            (
                "session.execution.started",
                serde_json::json!({"sessionID":"session-1", "_created":2000}),
            ),
            (
                "session.text.started",
                serde_json::json!({"sessionID":"session-1", "assistantMessageID":"message-1", "ordinal":0, "agent":"build", "_created":3000}),
            ),
            (
                "session.text.delta",
                serde_json::json!({"sessionID":"session-1", "assistantMessageID":"message-1", "ordinal":0, "delta":"Finished response"}),
            ),
            (
                "session.text.ended",
                serde_json::json!({"sessionID":"session-1", "assistantMessageID":"message-1", "ordinal":0, "text":"Finished response", "agent":"build", "_created":4000}),
            ),
            (
                "session.execution.succeeded",
                serde_json::json!({"sessionID":"session-1", "_created":5000}),
            ),
        ] {
            app.apply(Event::OpenCode(caelum_protocol::OpenCodeEvent {
                project_path: "/project".into(),
                event_type: event_type.into(),
                data,
                id: None,
                created: None,
                location: None,
            }));
        }
        assert_eq!(app.sessions[1].scope.provider, Provider::OpenCode);
    });
    advance(&mut rt, 0.3);

    let output = element(&rt, "live-entry-1-opencode-message-1");
    let details = element(&rt, "output-details-1-opencode-message-1");
    assert_eq!(details.text.as_deref(), Some("build · 4.0s"));
    assert!(details.rect[1] > output.rect[1]);
}

#[test]
fn restored_response_duration_includes_work_before_the_final_answer() {
    use caelum_core::application::Provider;
    let mut rt = live_runtime();
    rt.update(|s, _| {
        let app = &mut s.live.as_mut().unwrap().app;
        let session = &mut app.sessions[1];
        session.remote_id = Some("session-1".into());
        session.loaded = true;
        session.conversation = Default::default();
        session.conversation.history(Provider::OpenCode, &[
            serde_json::json!({"id":"prompt", "type":"user", "text":"Build this", "time":{"created":1000}}),
            serde_json::json!({"id":"work", "type":"assistant", "time":{"created":2000,"completed":120000}, "content":[
                {"id":"build", "type":"tool", "name":"bash", "state":{"status":"completed", "input":{"command":"cargo build"}, "content":[{"type":"text","text":"Build finished"}]}}
            ]}),
            serde_json::json!({"id":"answer", "type":"assistant", "agent":"build", "time":{"created":121000,"completed":129000}, "content":[{"type":"text","text":"Finished response"}]}),
            serde_json::json!({"id":"idle", "type":"idle", "outcome":"succeeded", "time":{"created":130000}}),
        ], false);
        app.select(1);
    });
    advance(&mut rt, 0.3);
    assert_eq!(
        element(&rt, "output-details-1-opencode-answer")
            .text
            .as_deref(),
        Some("build · 2m 9s")
    );
}

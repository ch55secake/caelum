use super::*;

#[test]
fn narrow_composer_wraps_all_options_and_keeps_queue_and_stop_reachable() {
    for (size, inspector) in [
        ((800.0, 620.0), false),
        ((800.0, 620.0), true),
        ((1280.0, 1000.0), false),
        ((1280.0, 1000.0), true),
    ] {
        let mut rt = design_runtime("composer-options", size);
        rt.update(|s, _| s.inspector.set_open(inspector));
        for working in [false, true] {
            rt.update(|s, _| {
                s.live.as_mut().unwrap().app.sessions[0]
                    .conversation
                    .working = working
            });
            advance(&mut rt, 0.3);
            for name in ["model-trigger", "effort-trigger", "agent-trigger", "send"] {
                let e = element(&rt, name);
                assert!(e.rect[2] >= 32.0 && e.rect[3] >= 28.0, "{name}: {e:?}");
                assert!(
                    e.rect
                        .iter()
                        .zip(e.visible)
                        .all(|(a, b)| (*a - b).abs() < 1.0),
                    "{name} clipped: {e:?}"
                );
                assert!(e.rect[1] + e.rect[3] <= size.1, "{name} below window");
            }
            assert!(element(&rt, "composer-editor").visible[3] >= 46.0);
            let composer = element(&rt, "message-composer").rect;
            let send = element(&rt, "send").rect;
            for axis in [0, 1] {
                let inset = composer[axis] + composer[axis + 2] - send[axis] - send[axis + 2];
                assert!(
                    (inset - 12.0).abs() < 0.5,
                    "Send/Stop inset: {inset}; size={size:?}, inspector={inspector}, working={working}"
                );
            }
            if working {
                assert!(element(&rt, "stop").visible[2] >= 50.0);
                assert_eq!(element(&rt, "send").label.as_deref(), Some("Queue message"));
            } else {
                assert_eq!(element(&rt, "send").label.as_deref(), Some("Send message"));
                assert!(rt.find("stop").is_none());
            }
            assert!(rt.find("provider-trigger").is_none());
            for name in ["effort", "agent"] {
                let trigger = format!("{name}-trigger");
                click(&mut rt, &trigger);
                let menu = element(&rt, &format!("{name}-content"));
                assert!(
                    menu.rect
                        .iter()
                        .zip(menu.visible)
                        .all(|(a, b)| (*a - b).abs() < 1.0),
                    "{menu:?}"
                );
                key(&mut rt, Key::Escape, Mods::default());
                advance(&mut rt, 0.2);
                assert!(element(&rt, &trigger).focused);
            }
            assert!(!rt.state.live.as_ref().unwrap().app.sessions[0].stopping);
        }
    }
}

#[test]
fn composer_submission_matches_enter_and_stop_keeps_the_next_draft() {
    use caelum_core::application::{Attachment, Command, Provider};
    for provider in Provider::ALL {
        for working in [false, true] {
            for keyboard in [false, true] {
                for attachment_only in [false, true] {
                    let mut rt = live_runtime_size((800.0, 620.0));
                    rt.update(|s, _| {
                        let l = s.live.as_mut().unwrap();
                        let session = &mut l.app.sessions[0];
                        session.scope.provider = provider;
                        session.remote_id = Some("session".into());
                        session.conversation.working = working;
                        if attachment_only {
                            session.attachments.push(Attachment::new(
                                "src/main.rs".into(),
                                "fn main() {}".into(),
                                None,
                                false,
                            ));
                        }
                        l.app.commands();
                    });
                    fill(
                        &mut rt,
                        "live-draft-0",
                        if attachment_only {
                            ""
                        } else {
                            "Next message café"
                        },
                    );
                    assert!(element(&rt, "send").focusable);
                    assert_eq!(
                        element(&rt, "send").label.as_deref(),
                        Some(if working {
                            "Queue message"
                        } else {
                            "Send message"
                        })
                    );
                    if keyboard {
                        key(&mut rt, Key::Enter, Mods::default());
                    } else {
                        click(&mut rt, "send");
                    }
                    let l = rt.state.live.as_mut().unwrap();
                    let commands = l.app.commands();
                    let prompt = if working && provider == Provider::Codex {
                        assert_eq!(l.app.sessions[0].queue.len(), 1);
                        l.app.sessions[0].queue[0].text.clone()
                    } else {
                        commands
                            .iter()
                            .find_map(|(_, command)| match command {
                                Command::Enqueue(_, _, text) if working => Some(text.clone()),
                                Command::Send(_, text, _) if !working => Some(text.clone()),
                                _ => None,
                            })
                            .expect("the primary action must submit rather than cancel")
                    };
                    assert!(prompt.contains(if attachment_only {
                        "fn main() {}"
                    } else {
                        "Next message café"
                    }));
                    assert!(l.drafts[0].text.is_empty());
                    assert!(l.app.sessions[0].attachments.is_empty());
                    assert!(!l.app.sessions[0].stopping);
                    assert!(element(&rt, "live-draft-0").focused);
                    if working {
                        fill(&mut rt, "live-draft-0", "Keep this newer draft");
                        click(&mut rt, "stop");
                        let l = rt.state.live.as_mut().unwrap();
                        assert_eq!(l.drafts[0].text, "Keep this newer draft");
                        assert!(l.app.sessions[0].queue_paused);
                        assert!(
                            l.app
                                .commands()
                                .iter()
                                .any(|(_, c)| matches!(c, Command::Cancel(_)))
                        );
                        assert!(!element(&rt, "send").focusable);
                        assert!(!element(&rt, "stop").focusable);
                    }
                }
            }
        }
    }
}

#[test]
fn composer_queue_disclosure_preserves_ownership_and_restores_saved_options() {
    let mut rt = design_runtime("composer-queued", (800.0, 620.0));
    let draft = rt.state.live.as_ref().unwrap().drafts[0].text.clone();
    let options = rt.state.live.as_ref().unwrap().app.sessions[0].queue[0]
        .options
        .clone();
    assert_eq!(
        element(&rt, "toggle-queued-messages").label.as_deref(),
        Some("2 queued")
    );
    assert!(rt.find("queued-messages").is_none());
    click(&mut rt, "toggle-queued-messages");
    assert_eq!(element(&rt, "toggle-queued-messages").expanded, Some(true));
    assert!(
        !element(&rt, "edit-queued-0-0").focusable,
        "editing must not overwrite a newer draft"
    );
    click(&mut rt, "move-queued-0-1");
    assert_eq!(
        rt.state.live.as_ref().unwrap().app.sessions[0].queue[0].text,
        "Check attachments and multiline drafts."
    );
    click(&mut rt, "pause-queued-messages");
    assert_eq!(
        element(&rt, "toggle-queued-messages").label.as_deref(),
        Some("2 queued · Paused")
    );
    click(&mut rt, "live-session-1");
    assert!(rt.find("toggle-queued-messages").is_none());
    click(&mut rt, "live-session-0");
    assert_eq!(element(&rt, "toggle-queued-messages").expanded, Some(true));
    assert_eq!(rt.state.live.as_ref().unwrap().drafts[0].text, draft);
    fill(&mut rt, "live-draft-0", "");
    click(&mut rt, "edit-queued-0-0");
    assert!(element(&rt, "live-draft-0").focused);
    let l = rt.state.live.as_ref().unwrap();
    assert_eq!(l.drafts[0].text, "Check attachments and multiline drafts.");
    assert_eq!(l.app.sessions[0].options, options);
    assert_eq!(l.app.sessions[0].queue.len(), 1);
    click(&mut rt, "remove-local-queued-0-0");
    assert!(rt.find("toggle-queued-messages").is_none());
    assert!(rt.find("queued-messages").is_none());
}

#[test]
fn composer_inbox_errors_stay_visible_when_collapsed_and_retry_the_same_message() {
    use caelum_core::application::{Command, Event, Provider, Reply};
    let mut rt = live_runtime_size((800.0, 620.0));
    rt.update(|s, _| {
        let l = s.live.as_mut().unwrap();
        let session = &mut l.app.sessions[0];
        session.scope.provider = Provider::OpenCode;
        session.remote_id = Some("session".into());
        session.conversation.working = true;
        l.app.commands();
    });
    fill(&mut rt, "live-draft-0", "Keep this queued message");
    click(&mut rt, "send");
    assert_eq!(
        element(&rt, "toggle-queued-messages").label.as_deref(),
        Some("1 queued · Saving…")
    );
    let (request, id) = rt
        .state
        .live
        .as_mut()
        .unwrap()
        .app
        .commands()
        .into_iter()
        .find_map(|(request, command)| match command {
            Command::Enqueue(_, id, _) => Some((request, id)),
            _ => None,
        })
        .unwrap();
    rt.update(|s, _| {
        s.live.as_mut().unwrap().apply(Event::Completed(
            request,
            Err("Could not save the message".into()),
        ))
    });
    advance(&mut rt, 0.2);
    assert_eq!(
        element(&rt, "toggle-queued-messages").label.as_deref(),
        Some("1 queued · Needs attention")
    );
    assert!(rt.find("queued-messages").is_none());
    click(&mut rt, "toggle-queued-messages");
    key(&mut rt, Key::Text(" ".into()), Mods::default());
    assert_eq!(element(&rt, "toggle-queued-messages").expanded, Some(false));
    key(&mut rt, Key::Text(" ".into()), Mods::default());
    click(&mut rt, &format!("retry-queued-{id}"));
    let (retry, command) = rt
        .state
        .live
        .as_mut()
        .unwrap()
        .app
        .commands()
        .into_iter()
        .find(|(_, c)| matches!(c, Command::Enqueue(..)))
        .unwrap();
    assert!(matches!(command, Command::Enqueue(_, current, _) if current == id));
    assert!(!element(&rt, &format!("remove-queued-{id}")).focusable);
    rt.update(|s, _| {
        s.live
            .as_mut()
            .unwrap()
            .apply(Event::Completed(retry, Ok(Reply::Done)))
    });
    advance(&mut rt, 0.2);
    assert_eq!(
        element(&rt, "toggle-queued-messages").label.as_deref(),
        Some("1 queued")
    );
    click(&mut rt, &format!("remove-queued-{id}"));
    assert!(
        rt.state
            .live
            .as_mut()
            .unwrap()
            .app
            .commands()
            .iter()
            .any(|(_, c)| matches!(c, Command::RemoveQueued(_, current) if current == &id))
    );
}

#[test]
fn composer_menus_use_provider_ids_restore_defaults_and_dismiss_on_session_change() {
    let mut rt = design_runtime("composer-options", (800.0, 620.0));
    rt.update(|s, _| {
        let app = &mut s.live.as_mut().unwrap().app;
        let scope = app.sessions[0].scope.clone();
        app.choices
            .get_mut(&scope)
            .unwrap()
            .agents
            .push(caelum_protocol::ProviderChoice {
                id: "custom-review".into(),
                name: "Review assistant".into(),
            });
    });
    advance(&mut rt, 0.2);
    let draft = rt.state.live.as_ref().unwrap().drafts[0].text.clone();
    click(&mut rt, "agent-trigger");
    assert_eq!(
        element(&rt, "option3").label.as_deref(),
        Some("Review assistant")
    );
    click(&mut rt, "option3");
    assert_eq!(
        rt.state.live.as_ref().unwrap().app.sessions[0]
            .options
            .agent
            .as_deref(),
        Some("custom-review")
    );
    click(&mut rt, "effort-trigger");
    key(&mut rt, Key::End, Mods::default());
    key(&mut rt, Key::Enter, Mods::default());
    assert_eq!(
        rt.state.live.as_ref().unwrap().app.sessions[0]
            .options
            .effort
            .as_deref(),
        Some("Low")
    );
    for name in ["effort", "agent"] {
        click(&mut rt, &format!("{name}-trigger"));
        click(&mut rt, "option0");
    }
    let options = &rt.state.live.as_ref().unwrap().app.sessions[0].options;
    assert_eq!(options.effort.as_deref(), Some(""));
    assert!(options.agent.is_none());
    assert_eq!(rt.state.live.as_ref().unwrap().drafts[0].text, draft);
    let other = rt.state.live.as_ref().unwrap().app.sessions[1]
        .options
        .clone();
    click(&mut rt, "agent-trigger");
    rt.update(|s, _| s.live.as_mut().unwrap().app.select(1));
    advance(&mut rt, 0.3);
    assert!(rt.find("agent-content").is_none());
    assert_eq!(
        rt.state.live.as_ref().unwrap().app.sessions[1].options,
        other
    );
}

#[test]
fn composer_activity_is_visible_without_sidebar_and_settles_with_reduced_motion() {
    for reduced in [false, true] {
        let mut rt = live_runtime_size((800.0, 620.0));
        rt.reduced_motion = reduced;
        fill(&mut rt, "live-draft-0", "Start working");
        key(&mut rt, Key::Enter, Mods::default());
        advance(&mut rt, 0.3);
        assert!(element(&rt, "session-activity").visible[3] > 15.0);
        rt.update(|s, _| {
            s.sidebar.set_open(false);
        });
        advance(&mut rt, 0.4);
        assert!(element(&rt, "session-activity").visible[3] >= 24.0);
        assert!(element(&rt, "stop").focusable);
        rt.update(|s, _| {
            let session = &mut s.live.as_mut().unwrap().app.sessions[0];
            session.submitting = false;
            session.conversation.settle();
        });
        advance(&mut rt, 0.4);
        assert!(
            !rt.elements()
                .iter()
                .any(|e| !e.exiting && e.key.as_deref() == Some("session-activity"))
        );
    }
}

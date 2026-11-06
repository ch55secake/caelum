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

fn live_runtime() -> Runtime<Workspace> {
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
    let mut rt = Runtime::new(state, app.view, (1280.0, 1000.0), Env::new(true, 1.0));
    rt.poll = app.poll;
    rt.on_key = app.on_key;
    rt.set_font_families(app.families.0, app.families.1);
    advance(&mut rt, 0.5);
    rt
}

#[test]
fn live_creation_streaming_and_failed_submissions_preserve_independent_editors() {
    use caelum_core::application::{Command, Event, Options, Reply, SessionKey, Summary};
    let mut rt = live_runtime();
    click(&mut rt, "live-draft-0");
    rt.paste("First real prompt");
    key(&mut rt, Key::Enter, Mods::default());
    let (request, command) = rt
        .state
        .live
        .as_mut()
        .unwrap()
        .app
        .commands()
        .pop()
        .unwrap();
    let Command::Start(scope, _) = command else {
        panic!("expected creation")
    };
    rt.paste("Next draft");
    click(&mut rt, "live-session-1");
    click(&mut rt, "live-draft-1");
    rt.paste("Other provider draft");
    rt.update(|s, _| {
        s.live.as_mut().unwrap().app.apply(Event::Completed(
            request,
            Ok(Reply::Session(Summary {
                key: SessionKey {
                    scope,
                    id: "thread-1".into(),
                },
                title: "Created".into(),
                working: false,
                options: Options::default(),
            })),
        ))
    });
    let (send, command) = rt
        .state
        .live
        .as_mut()
        .unwrap()
        .app
        .commands()
        .pop()
        .unwrap();
    assert!(matches!(command, Command::Send(_, ref prompt, _) if prompt == "First real prompt"));
    rt.update(|s, _| {
        let app = &mut s.live.as_mut().unwrap().app;
        app.apply(Event::Completed(send, Ok(Reply::Done)));
        app.apply(Event::Codex(caelum_protocol::CodexEvent {
            project_path: "/project".into(), method: "item/agentMessage/delta".into(), request_id: None,
            params: serde_json::json!({ "threadId": "thread-1", "itemId": "answer", "delta": "Background response" }),
        }));
    });
    advance(&mut rt, 0.5);
    let live = rt.state.live.as_ref().unwrap();
    assert_eq!(live.app.selected, Some(1));
    assert_eq!(live.drafts[0].text, "Next draft");
    assert_eq!(live.drafts[1].text, "Other provider draft");
    assert_eq!(
        live.app.sessions[0].conversation.entries[1].text,
        "Background response"
    );
    key(&mut rt, Key::Enter, Mods::default());
    let (request, _) = rt
        .state
        .live
        .as_mut()
        .unwrap()
        .app
        .commands()
        .pop()
        .unwrap();
    rt.paste("Newer unsent draft");
    rt.update(|s, _| {
        s.live.as_mut().unwrap().app.apply(Event::Completed(
            request,
            Err("Provider unavailable".into()),
        ))
    });
    advance(&mut rt, 0.2);
    let live = rt.state.live.as_ref().unwrap();
    assert_eq!(live.drafts[1].text, "Newer unsent draft");
    assert_eq!(
        live.app.sessions[1].failed_prompt.as_deref(),
        Some("Other provider draft")
    );
    click(&mut rt, "live-session-0");
    assert_eq!(rt.state.live.as_ref().unwrap().drafts[0].text, "Next draft");
}

#[test]
fn live_navigation_restores_reading_position_and_approval_decisions_keep_their_owner() {
    use caelum_core::application::{Command, Event, Provider};
    let mut rt = live_runtime();
    rt.update(|s, _| {
        let app = &mut s.live.as_mut().unwrap().app;
        app.sessions[0].remote_id = Some("thread-1".into());
        app.sessions[0].conversation.history(Provider::Codex, &[serde_json::json!({
            "status":"completed", "items":[{"id":"long","type":"agentMessage","text":"A paragraph worth reading.\n\n".repeat(90)}]
        })], false);
    });
    advance(&mut rt, 0.5);
    let (x, y) = rt.find("live-transcript-0").unwrap();
    rt.wheel(x, y, 300.0);
    advance(&mut rt, 0.2);
    let position = element(&rt, "live-entry-0-codex-long").rect[1];
    click(&mut rt, "live-session-1");
    click(&mut rt, "live-session-0");
    advance(&mut rt, 0.5);
    assert!((element(&rt, "live-entry-0-codex-long").rect[1] - position).abs() < 1.0);
    assert_eq!(rt.scroll_at_bottom("live-transcript-0"), Some(false));
    rt.update(|s, tx| {
        s.live.as_mut().unwrap().app.apply(Event::Approvals(vec![
            caelum_protocol::ApprovalPrompt {
                provider: "codex".into(),
                project_path: "/project".into(),
                session_id: Some("thread-1".into()),
                request_id: "one".into(),
                summary: "Run command?".into(),
                description: "cargo test".into(),
            },
        ]));
        tx.scroll_to_bottom_instant("live-transcript-0");
    });
    advance(&mut rt, 0.3);
    click(&mut rt, "allow-once-one");
    let (_, command) = rt
        .state
        .live
        .as_mut()
        .unwrap()
        .app
        .commands()
        .pop()
        .unwrap();
    assert!(
        matches!(command, Command::Approve(prompt, caelum_protocol::ApprovalDecision::AllowOnce)
        if prompt.project_path == "/project" && prompt.session_id.as_deref() == Some("thread-1") && prompt.request_id == "one")
    );
}

#[test]
#[ignore = "requires a locally installed/authenticated provider and CAELUM_RUNE_SMOKE_PROJECT"]
fn real_provider_discovery_and_history_smoke() {
    use caelum_core::application::Provider;
    use std::{
        sync::Arc,
        time::{Duration, Instant},
    };
    let path = std::env::var("CAELUM_RUNE_SMOKE_PROJECT")
        .expect("set a project containing provider history");
    let provider = if std::env::var("CAELUM_RUNE_SMOKE_PROVIDER").as_deref() == Ok("codex") {
        Provider::Codex
    } else {
        Provider::OpenCode
    };
    let app = spec();
    let mut state = Workspace::live(Some(path));
    app.setup.unwrap()(&mut state, Arc::new(|| {}));
    let mut rt = Runtime::new(state, app.view, (1280.0, 1000.0), Env::new(true, 1.0));
    rt.poll = app.poll;
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut selected = None;
    loop {
        rt.frame(1.0 / 60.0);
        let live = rt.state.live.as_ref().unwrap();
        assert!(
            live.app.error.is_none(),
            "project failed: {:?}",
            live.app.error
        );
        if let Some(error) = live
            .app
            .notices
            .iter()
            .find(|(scope, _)| scope.provider == provider)
            .map(|(_, error)| error)
        {
            panic!("provider failed: {error}");
        }
        if let Some(index) = selected {
            let session: &caelum_core::application::Session = &live.app.sessions[index];
            assert!(
                session.error.is_none(),
                "history failed: {:?}",
                session.error
            );
            if session.loaded && !session.loading {
                println!(
                    "{}: {} sessions discovered; {} history entries rendered",
                    provider.as_str(),
                    live.app.sessions.len(),
                    session.conversation.entries.len()
                );
                break;
            }
        } else if let Some(index) = live
            .app
            .sessions
            .iter()
            .position(|s| s.scope.provider == provider && s.remote_id.is_some())
        {
            selected = Some(index);
            rt.update(|s, _| s.live.as_mut().unwrap().app.select(index));
        }
        assert!(
            Instant::now() < deadline,
            "provider discovery/history timed out"
        );
        std::thread::sleep(Duration::from_millis(16));
    }
}

#[test]
fn live_provider_and_model_controls_apply_to_first_send() {
    use caelum_core::application::{Command, Provider, Scope};
    use caelum_protocol::{ProviderChoice, ProviderChoices, ProviderModelChoice};
    let mut rt = live_runtime();
    rt.update(|s, _| {
        let app = &mut s.live.as_mut().unwrap().app;
        app.choices.insert(
            Scope {
                provider: Provider::OpenCode,
                project: "/project".into(),
            },
            ProviderChoices {
                models: ["first", "second"]
                    .into_iter()
                    .map(|id| ProviderModelChoice {
                        id: id.into(),
                        name: id.into(),
                        provider: None,
                        context_window: None,
                        efforts: vec![ProviderChoice {
                            id: "low".into(),
                            name: "Low".into(),
                        }],
                        default_effort: Some("low".into()),
                    })
                    .collect(),
                agents: vec![ProviderChoice {
                    id: "build".into(),
                    name: "Build".into(),
                }],
                default_model: Some("first".into()),
            },
        );
    });
    click(&mut rt, "inspector-toggle");
    advance(&mut rt, 0.5);
    click(&mut rt, "provider-trigger");
    click(&mut rt, "option1");
    assert_eq!(
        rt.state.live.as_ref().unwrap().app.sessions[0]
            .scope
            .provider,
        Provider::OpenCode
    );
    click(&mut rt, "model-trigger");
    click(&mut rt, "option1");
    click(&mut rt, "agent-trigger");
    click(&mut rt, "option0");
    click(&mut rt, "live-draft-0");
    rt.paste("Use my choices");
    key(&mut rt, Key::Enter, Mods::default());
    let (_, command) = rt
        .state
        .live
        .as_mut()
        .unwrap()
        .app
        .commands()
        .pop()
        .unwrap();
    assert!(
        matches!(command, Command::Start(scope, options) if scope.provider == Provider::OpenCode
        && options.model.as_deref() == Some("second") && options.effort.as_deref() == Some("low") && options.agent.as_deref() == Some("build"))
    );
}

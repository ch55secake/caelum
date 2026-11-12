use super::*;

#[test]
fn session_popover_roves_without_selecting_its_background_owner() {
    let mut rt = live_runtime_size((800.0, 620.0));
    rt.update(|s, _| {
        let l = s.live.as_mut().unwrap();
        l.app.sessions[1].remote_id = Some("background".into());
        l.app.sessions[1].loaded = true;
        l.app.sessions[0].conversation.working = true;
    });
    let (x, y) = rt.find("session-actions-1").unwrap();
    rt.pointer_move(x, y);
    advance(&mut rt, 0.2);
    click(&mut rt, "session-actions-1");
    assert_eq!(rt.state.live.as_ref().unwrap().app.selected, Some(0));
    let popup = element(&rt, "session-menu");
    assert!(
        popup
            .rect
            .iter()
            .zip(popup.visible)
            .all(|(a, b)| (*a - b).abs() < 1.0)
    );
    key(&mut rt, Key::Down, Mods::default());
    advance(&mut rt, 0.1);
    assert!(element(&rt, "archive-session").focused || element(&rt, "refresh-history").focused);
    key(&mut rt, Key::Escape, Mods::default());
    advance(&mut rt, 0.2);
    assert!(element(&rt, "session-actions-1").focused);
    assert!(!rt.state.live.as_ref().unwrap().app.sessions[0].stopping);
    click(&mut rt, "session-actions-1");
    click(&mut rt, "refresh-history");
    let commands = rt.state.live.as_mut().unwrap().app.commands();
    assert!(commands.iter().any(|(_, c)| matches!(c, caelum_core::application::Command::Resume(key) if key.id == "background")));
    assert_eq!(rt.state.live.as_ref().unwrap().app.selected, Some(0));
}

#[test]
fn archive_toast_pauses_for_presented_keyboard_focus_then_expires() {
    let mut rt = live_runtime_size((800.0, 620.0));
    rt.update(|s, _| s.live.as_mut().unwrap().archive(1, true));
    advance(&mut rt, 0.3);
    for _ in 0..40 {
        key(&mut rt, Key::Tab, Mods::default());
        advance(&mut rt, 0.05);
        if element(&rt, "undo-archive").focused {
            break;
        }
    }
    assert!(element(&rt, "undo-archive").focused);
    let now = caelum_core::application::presentation::now();
    rt.update(|s, _| {
        let l = s.live.as_mut().unwrap();
        assert_eq!(l.focus_key.as_deref(), Some("undo-archive"));
        assert!(!l.tick_archive(now + 30_000));
    });
    click(&mut rt, "live-draft-0");
    rt.update(|s, _| {
        let l = s.live.as_mut().unwrap();
        assert!(l.tick_archive(now + 39_000));
        assert!(l.app.sessions[1].archived);
    });
    advance(&mut rt, 0.2);
    assert!(
        !rt.elements()
            .iter()
            .any(|e| !e.exiting && e.key.as_deref() == Some("archive-toast"))
    );
}

#[test]
fn refresh_sessions_button_refreshes_every_open_session() {
    let mut rt = live_runtime_size((800.0, 620.0));
    rt.update(|s, _| {
        let app = &mut s.live.as_mut().unwrap().app;
        app.sessions[0].remote_id = Some("codex-session".into());
        app.sessions[1].remote_id = Some("opencode-session".into());
    });

    click(&mut rt, "refresh-sessions");

    let app = &rt.state.live.as_ref().unwrap().app;
    assert!(app.sessions[0].loading);
    assert!(app.sessions[1].loading);
}

#[test]
fn sessions_slash_command_opens_provider_history_and_resumes_a_selected_session() {
    let mut rt = live_runtime_size((800.0, 620.0));
    rt.update(|s, _| {
        let app = &mut s.live.as_mut().unwrap().app;
        app.sessions[0].remote_id = Some("old-codex-session".into());
        app.sessions[0].title = "Earlier Codex work".into();
        app.sessions[0].loaded = false;
    });
    fill(&mut rt, "live-draft-0", "/sessions");
    key(&mut rt, Key::Enter, Mods::default());
    advance(&mut rt, 0.2);

    assert_eq!(
        rt.state.live.as_ref().unwrap().panels.overlay,
        Some(crate::panels::Screen::Sessions)
    );
    assert!(rt.find("provider-session-0").is_some());
    let commands = rt.state.live.as_mut().unwrap().app.commands();
    assert!(commands.iter().any(|(_, command)| {
        matches!(
            command,
            caelum_core::application::Command::ListArchived(scope)
                if scope.project == "/project"
                    && scope.provider == caelum_core::application::Provider::Codex
        )
    }));

    click(&mut rt, "provider-session-0");
    assert_eq!(rt.state.live.as_ref().unwrap().app.selected, Some(0));
    assert_eq!(rt.state.live.as_ref().unwrap().panels.overlay, None);
    assert!(
        rt.state
            .live
            .as_mut()
            .unwrap()
            .app
            .commands()
            .iter()
            .any(|(_, command)| matches!(
                command,
                caelum_core::application::Command::Resume(key)
                    if key.id == "old-codex-session"
            ))
    );
}

#[test]
fn launcher_reuses_the_selected_project_draft_and_returns_keyboard_focus() {
    let mut rt = live_runtime();
    fill(&mut rt, "live-draft-0", "Keep my original draft");
    key(
        &mut rt,
        Key::Command('n'),
        Mods {
            cmd: true,
            shift: true,
            ..Default::default()
        },
    );
    advance(&mut rt, 0.3);
    click(&mut rt, "launch-provider-opencode");
    assert_eq!(
        rt.state.live.as_ref().unwrap().panels.launch_provider,
        caelum_core::application::Provider::OpenCode
    );
    fill(&mut rt, "launch-search", "project");
    click(&mut rt, &crate::code_view::source_key("/project", "launch"));
    assert_eq!(rt.state.live.as_ref().unwrap().app.selected, Some(1));
    rt.paste("OpenCode draft");
    advance(&mut rt, 0.1);
    assert_eq!(
        rt.state.live.as_ref().unwrap().drafts[1].text,
        "OpenCode draft"
    );
    key(
        &mut rt,
        Key::Command('n'),
        Mods {
            cmd: true,
            ..Default::default()
        },
    );
    assert_eq!(rt.state.live.as_ref().unwrap().app.sessions.len(), 2);
    assert_eq!(
        rt.state.live.as_ref().unwrap().drafts[0].text,
        "Keep my original draft"
    );
    assert_eq!(
        rt.state.live.as_ref().unwrap().drafts[1].text,
        "OpenCode draft"
    );
}

#[test]
fn closing_local_sessions_and_undo_keep_the_workspace_editable() {
    for dialog in [false, true] {
        let mut rt = live_runtime();
        click(&mut rt, "live-draft-0");
        rt.paste("Keep the archived draft café ☁️");
        click(&mut rt, "live-session-1");
        click(&mut rt, "live-draft-1");
        rt.paste("Independent background draft");
        click(&mut rt, "live-session-0");
        if dialog {
            click(&mut rt, "session-actions-0");
            click(&mut rt, "archive-session");
            advance(&mut rt, 0.4);
        } else {
            close_session(&mut rt, 0);
        }
        let live = rt.state.live.as_ref().unwrap();
        assert!(live.app.sessions[0].archived);
        assert_eq!(live.app.selected, Some(1));
        assert_eq!(live.app.sessions.len(), 2);
        assert!(live.panels.overlay.is_none());
        assert_eq!(live.drafts[1].text, "Independent background draft");
        element(&rt, "live-draft-1");
        close_session(&mut rt, 1);
        assert_eq!(rt.state.live.as_ref().unwrap().app.selected, Some(2));
        element(&rt, "live-draft-2");
        rt.paste("Replacement draft");
        rt.frame(1.0 / 60.0);
        assert_eq!(
            rt.state.live.as_ref().unwrap().drafts[2].text,
            "Replacement draft"
        );
        click(&mut rt, "undo-archive");
        let live = rt.state.live.as_ref().unwrap();
        assert_eq!(live.app.selected, Some(1));
        assert!(!live.app.sessions[1].archived);
        assert_eq!(live.drafts[1].text, "Independent background draft");
        assert_eq!(live.drafts[0].text, "Keep the archived draft café ☁️");
        assert_eq!(live.drafts[2].text, "Replacement draft");
        click(&mut rt, "live-session-1");
        click(&mut rt, "live-draft-1");
        rt.paste(" after undo");
        rt.frame(1.0 / 60.0);
        assert!(
            rt.state.live.as_ref().unwrap().drafts[1]
                .text
                .ends_with(" after undo")
        );

        click(&mut rt, "toggle-archived");
        advance(&mut rt, 0.4);
        click(&mut rt, "live-session-0");
        click(&mut rt, "session-actions-0");
        click(&mut rt, "archive-session");
        let live = rt.state.live.as_ref().unwrap();
        assert!(!live.panels.archived);
        assert!(!live.app.sessions[0].archived);
        assert_eq!(live.app.selected, Some(0));
        assert_eq!(live.drafts[0].text, "Keep the archived draft café ☁️");
    }
}

#[test]
fn closing_provider_sessions_handles_completion_failure_and_selection_changes() {
    use caelum_core::application::{Command, Event, Provider, Reply};
    for provider in Provider::ALL {
        let mut rt = live_runtime();
        rt.update(|s, _| {
            let session = &mut s.live.as_mut().unwrap().app.sessions[0];
            session.scope.provider = provider;
            session.remote_id = Some("remote-session".into());
            session.loaded = true;
        });
        click(&mut rt, "live-draft-0");
        rt.paste("Provider draft");
        close_session(&mut rt, 0);
        let live = rt.state.live.as_mut().unwrap();
        assert!(live.app.sessions[0].archiving);
        assert_eq!(live.app.selected, Some(0));
        let (request, command) = live.app.commands().pop().unwrap();
        assert!(matches!(command, Command::Archive(key, true)
            if key.scope.provider == provider && key.id == "remote-session"));
        rt.update(|s, _| {
            s.live
                .as_mut()
                .unwrap()
                .apply(Event::Completed(request, Err("Offline".into())));
        });
        let session = &rt.state.live.as_ref().unwrap().app.sessions[0];
        assert!(!session.archived && !session.archiving);
        assert_eq!(session.error.as_deref(), Some("Offline"));
        element(&rt, "live-draft-0");

        // Completing a background archive must not replace the user's new selection.
        close_session(&mut rt, 0);
        let request = rt
            .state
            .live
            .as_mut()
            .unwrap()
            .app
            .commands()
            .pop()
            .unwrap()
            .0;
        click(&mut rt, "live-session-1");
        click(&mut rt, "live-draft-1");
        rt.paste("Still writing here");
        rt.update(|s, _| {
            s.live
                .as_mut()
                .unwrap()
                .apply(Event::Completed(request, Ok(Reply::Done)));
        });
        assert_eq!(rt.state.live.as_ref().unwrap().app.selected, Some(1));
        assert_eq!(rt.state.live.as_ref().unwrap().app.sessions.len(), 2);
        assert_eq!(rt.state.live.as_ref().unwrap().app.last_archived, Some(0));
        advance(&mut rt, 0.4);
        click(&mut rt, "undo-archive");
        let (request, command) = rt
            .state
            .live
            .as_mut()
            .unwrap()
            .app
            .commands()
            .pop()
            .unwrap();
        assert!(matches!(command, Command::Archive(_, false)));
        rt.update(|s, _| {
            s.live
                .as_mut()
                .unwrap()
                .apply(Event::Completed(request, Ok(Reply::Done)));
        });
        advance(&mut rt, 0.4);
        assert_eq!(rt.state.live.as_ref().unwrap().app.selected, Some(1));
        assert_eq!(
            rt.state.live.as_ref().unwrap().drafts[1].text,
            "Still writing here"
        );
        click(&mut rt, "live-session-0");
        click(&mut rt, "live-draft-0");
        rt.paste(" after restore");
        rt.frame(1.0 / 60.0);
        assert_eq!(
            rt.state.live.as_ref().unwrap().drafts[0].text,
            "Provider draft after restore"
        );

        // Prefer the other session's draft instead of creating a blank replacement.
        click(&mut rt, "session-actions-0");
        click(&mut rt, "archive-session");
        let request = rt
            .state
            .live
            .as_mut()
            .unwrap()
            .app
            .commands()
            .pop()
            .unwrap()
            .0;
        rt.update(|s, _| {
            s.live
                .as_mut()
                .unwrap()
                .apply(Event::Completed(request, Ok(Reply::Done)));
        });
        advance(&mut rt, 0.4);
        let live = rt.state.live.as_ref().unwrap();
        assert_eq!(live.app.selected, Some(1));
        assert_eq!(live.app.sessions.len(), 2);
        assert_eq!(live.drafts[1].text, "Still writing here");

        // Restoring from the archive list updates both provider and workspace state.
        click(&mut rt, "toggle-archived");
        click(&mut rt, "live-session-0");
        click(&mut rt, "session-actions-0");
        click(&mut rt, "archive-session");
        let (request, command) = rt
            .state
            .live
            .as_mut()
            .unwrap()
            .app
            .commands()
            .pop()
            .unwrap();
        assert!(matches!(command, Command::Archive(_, false)));
        rt.update(|s, _| {
            s.live
                .as_mut()
                .unwrap()
                .apply(Event::Completed(request, Ok(Reply::Done)));
        });
        advance(&mut rt, 0.4);
        let live = rt.state.live.as_ref().unwrap();
        assert!(!live.app.sessions[0].archived);
        assert_eq!(live.app.selected, Some(0));
        assert!(!live.panels.archived);
    }
}

#[test]
fn deleting_an_archived_session_requires_confirmation() {
    use caelum_core::application::{Command, Event, Reply};

    let mut rt = live_runtime();
    rt.update(|s, _| {
        let live = s.live.as_mut().unwrap();
        live.app.sessions[0].remote_id = Some("remote-session".into());
        live.app.sessions[0].title =
            "A very long session title that must wrap inside the permanent deletion confirmation"
                .into();
        live.app.sessions[0].archived = true;
        live.panels.archived = true;
    });
    advance(&mut rt, 0.3);

    click(&mut rt, "session-actions-0");
    click(&mut rt, "delete-session");
    element(&rt, "confirm-delete-session");
    let dialog = element(&rt, "delete-session-dialog");
    let description = element(&rt, "delete-session-description");
    assert!(description.rect[3] > 20.0, "confirmation copy should wrap");
    assert!(description.rect[0] >= dialog.rect[0]);
    assert!(description.rect[0] + description.rect[2] <= dialog.rect[0] + dialog.rect[2]);
    click(&mut rt, "cancel-delete-session");
    let commands = rt.state.live.as_mut().unwrap().app.commands();
    assert!(
        commands
            .iter()
            .all(|(_, command)| !matches!(command, Command::DeleteSession(_)))
    );
    assert!(!rt.state.live.as_ref().unwrap().app.sessions[0].forgotten);

    close_session(&mut rt, 0);
    element(&rt, "confirm-delete-session");
    click(&mut rt, "confirm-delete-session");
    let (request, command) = rt
        .state
        .live
        .as_mut()
        .unwrap()
        .app
        .commands()
        .into_iter()
        .find(|(_, command)| matches!(command, Command::DeleteSession(_)))
        .unwrap();
    assert!(matches!(command, Command::DeleteSession(key) if key.id == "remote-session"));
    assert!(!rt.state.live.as_ref().unwrap().app.sessions[0].forgotten);

    rt.update(|s, _| {
        s.live
            .as_mut()
            .unwrap()
            .apply(Event::Completed(request, Ok(Reply::Done)));
    });
    advance(&mut rt, 0.4);
    let live = rt.state.live.as_ref().unwrap();
    assert!(live.app.sessions[0].forgotten);
    assert_ne!(live.app.selected, Some(0));
    assert!(!live.panels.archived);
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

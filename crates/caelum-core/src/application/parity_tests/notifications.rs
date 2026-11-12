use super::*;

fn open_workspace() -> Workspace {
    let mut app = workspace();
    app.projects.push(caelum_protocol::ProjectInfo {
        path: "/project".into(),
        name: "Project".into(),
    });
    app
}

fn notifications(app: &mut Workspace) -> Vec<Notification> {
    app.commands()
        .into_iter()
        .filter_map(|(_, command)| {
            if let Command::Notify(notification) = command {
                Some(notification)
            } else {
                None
            }
        })
        .collect()
}

#[test]
fn notification_effects_keep_the_exact_requesting_session() {
    let mut app = open_workspace();
    app.window_background = true;
    // Identical remote IDs in different providers must remain distinct.
    app.sessions[1].remote_id = Some("session-0".into());
    let target = app.sessions[1].key().unwrap();
    let question = caelum_protocol::QuestionPrompt {
        provider: "codex".into(),
        project_path: "/project".into(),
        session_id: "session-0".into(),
        request_id: "question".into(),
        title: "Question".into(),
        fields: vec![],
    };
    app.apply(Event::Questions(vec![question.clone()]));
    assert_eq!(notifications(&mut app)[0].target.as_ref(), Some(&target));
    app.apply(Event::Questions(vec![question]));
    assert!(notifications(&mut app).is_empty());
    app.apply(Event::Approvals(vec![caelum_protocol::ApprovalPrompt {
        provider: "codex".into(),
        project_path: "/project".into(),
        session_id: Some("session-0".into()),
        request_id: "approval".into(),
        summary: "Permission".into(),
        description: String::new(),
    }]));
    assert_eq!(notifications(&mut app)[0].target.as_ref(), Some(&target));
    app.apply(opencode(
        "session.text.delta",
        json!({"sessionID":"session-0","assistantMessageID":"answer","delta":"hello"}),
    ));
    app.apply(opencode(
        "session.execution.succeeded",
        json!({"sessionID":"session-0"}),
    ));
    assert_eq!(notifications(&mut app)[0].target, app.sessions[0].key());
}

#[test]
fn notification_activation_rejects_stale_sessions_and_preserves_drafts() {
    let mut app = open_workspace();
    app.sessions[0].draft = "first draft".into();
    app.sessions[1].draft = "second draft".into();
    app.sessions[1].remote_id = app.sessions[0].remote_id.clone();
    let target = app.sessions[1].key().unwrap();
    app.activate_notification(target.clone());
    assert_eq!(app.selected, Some(1));
    assert_eq!(app.sessions[0].draft, "first draft");
    assert_eq!(app.sessions[1].draft, "second draft");
    for state in ["archived", "forgotten", "closed", "missing"] {
        app.select(0);
        app.sessions[1].archived = state == "archived";
        app.sessions[1].forgotten = state == "forgotten";
        let mut stale = target.clone();
        if state == "closed" {
            stale.scope.project = "/closed-project".into();
        }
        if state == "missing" {
            stale.id = "deleted-session".into();
        }
        app.activate_notification(stale);
        assert_eq!(app.selected, Some(0), "{state}");
        assert_eq!(app.sessions.len(), 2);
    }
}

#[test]
fn cold_notification_click_waits_for_restore_and_newer_navigation_wins() {
    let original = open_workspace();
    let target = original.sessions[1].key().unwrap();
    for navigate in [false, true] {
        let mut app = Workspace::default();
        app.enable_store("/unused-notification-test/workspace.json".into());
        let load = app.commands()[0].0;
        app.activate_notification(target.clone());
        if navigate {
            app.open_project("/newer-project".into());
        }
        app.apply(Event::Completed(
            load,
            Ok(Reply::Snapshot(Some(Box::new(original.snapshot())))),
        ));
        assert_eq!(app.selected, Some(if navigate { 0 } else { 1 }));
    }
    let mut app = Workspace::default();
    app.enable_store("/unused-notification-test/workspace.json".into());
    let load = app.commands()[0].0;
    app.activate_notification(target);
    app.activate_notification(original.sessions[0].key().unwrap());
    app.apply(Event::Completed(
        load,
        Ok(Reply::Snapshot(Some(Box::new(original.snapshot())))),
    ));
    assert_eq!(app.selected, Some(0));
}

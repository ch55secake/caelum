use super::*;

#[test]
fn stale_opencode_archive_error_forgets_the_session_locally() {
    let mut app = Workspace::default();
    let project = "/project";
    let index = app.new_session(scope(Provider::OpenCode, project));
    app.sessions[index].remote_id = Some("ses_stale".into());
    app.sessions[index].draft = "Keep this draft".into();
    app.attach(
        index,
        Attachment::new("src/main.rs".into(), "snapshot".into(), None, false),
    );
    app.archive(index, true);
    let (request, command) = app.commands().pop().unwrap();
    assert!(matches!(command, Command::Archive(_, true)));

    app.apply(Event::Completed(
        request,
        Err("OpenCode session does not belong to this project".into()),
    ));

    assert!(app.sessions[index].forgotten);
    assert!(!app.sessions[index].archiving);
    assert_eq!(app.last_archived, None, "there is no provider Undo action");
    let selected = app.selected.unwrap();
    assert_ne!(selected, index);
    assert_eq!(app.sessions[selected].draft, "Keep this draft");
    assert_eq!(app.sessions[selected].attachments[0].content, "snapshot");
    assert!(app.sessions[selected]
        .error
        .as_deref()
        .is_some_and(|error| error.contains("not archived in OpenCode")));

    let snapshot = app.snapshot();
    assert_eq!(snapshot.sessions.len(), 1);
    assert_eq!(snapshot.selected, Some(0));
    assert_eq!(snapshot.sessions[0].draft, "Keep this draft");
    assert_eq!(snapshot.sessions[0].remote_id, None);
}

#[test]
fn archiving_selects_existing_context_before_a_blank_session() {
    let mut app = Workspace::default();
    app.projects.push(ProjectInfo {
        path: "/project".into(),
        name: "Project".into(),
    });
    let archived = app.new_session(scope(Provider::Codex, "/project"));
    let contextual = app.new_session(scope(Provider::Codex, "/project"));
    app.sessions[contextual].remote_id = Some("thread-with-context".into());
    let blank = app.new_session(scope(Provider::Codex, "/project"));
    app.select(archived);

    app.archive(archived, true);

    assert_eq!(app.selected, Some(contextual));
    assert_eq!(app.sessions.len(), 3, "do not create another empty session");
    assert!(!app.sessions[blank].archived);

    app.undo_archive();
    assert_eq!(app.selected, Some(archived));
    assert!(!app.sessions[archived].archived);
}

#[test]
fn deleting_archived_provider_sessions_removes_local_metadata_after_provider_success() {
    let mut app = Workspace::default();
    app.projects.push(ProjectInfo {
        path: "/project".into(),
        name: "Project".into(),
    });
    let index = app.new_session(scope(Provider::OpenCode, "/project"));
    app.sessions[index].remote_id = Some("ses_archived".into());
    app.archive(index, true);
    let archive = app.commands().pop().unwrap().0;
    app.apply(Event::Completed(archive, Ok(Reply::Done)));
    assert!(app.sessions[index].archived);

    assert!(app.delete_archived_session(index));
    assert!(!app.sessions[index].forgotten, "wait for provider deletion");
    let (request, command) = app.commands().pop().unwrap();
    assert!(matches!(command, Command::DeleteSession(key) if key.id == "ses_archived"));
    app.apply(Event::Completed(
        request,
        Err("provider does not support deletion".into()),
    ));
    assert!(app.sessions[index].archived);
    assert!(!app.sessions[index].forgotten);
    assert!(!app.sessions[index].deleting);

    assert!(app.delete_archived_session(index));
    let (request, command) = app.commands().pop().unwrap();
    assert!(matches!(command, Command::DeleteSession(key) if key.id == "ses_archived"));
    app.apply(Event::Completed(request, Ok(Reply::Done)));

    assert!(app.sessions[index].forgotten);
    assert!(app
        .snapshot()
        .sessions
        .iter()
        .all(|session| { session.remote_id.as_deref() != Some("ses_archived") }));
}

#[test]
fn provider_deleted_events_forget_sessions_and_keep_the_selection_usable() {
    let mut app = Workspace::default();
    app.projects.push(ProjectInfo {
        path: "/project".into(),
        name: "Project".into(),
    });
    let parent = app.new_session(scope(Provider::OpenCode, "/project"));
    app.sessions[parent].remote_id = Some("ses_parent".into());
    app.sessions[parent].archived = true;
    let child = app.new_session(scope(Provider::OpenCode, "/project"));
    app.sessions[child].remote_id = Some("ses_child".into());
    app.sessions[child].archived = true;
    let active = app.new_session(scope(Provider::OpenCode, "/project"));
    app.sessions[active].remote_id = Some("ses_active".into());
    app.select(parent);

    for id in ["ses_parent", "ses_child"] {
        app.apply(Event::OpenCode(OpenCodeEvent {
            project_path: "/project".into(),
            event_type: "session.deleted".into(),
            data: json!({"sessionID": id}),
            id: None,
            created: None,
            location: None,
        }));
    }

    assert!(app.sessions[parent].forgotten);
    assert!(app.sessions[child].forgotten);
    assert_eq!(app.selected, Some(active));
    assert!(app.sessions[active].can_send());
}

#[test]
fn discovery_started_before_archive_completion_preserves_the_new_state() {
    for provider in Provider::ALL {
        for archived in [false, true] {
            for during in [false, true] {
                let mut workspace = Workspace::default();
                let scope = scope(provider, "/a");
                let index = workspace.new_session(scope.clone());
                workspace.sessions[index].remote_id = Some("changed".into());
                workspace.sessions[index].archived = !archived;
                let other = workspace.new_session(scope.clone());
                workspace.sessions[other].remote_id = Some("other".into());
                workspace.sessions[other].archived = archived;
                let mut list = None;
                if !during {
                    list = Some(discover_session_list(&mut workspace, &scope, !archived));
                }
                workspace.archive(index, archived);
                let archive = workspace.commands().pop().unwrap().0;
                if during {
                    list = Some(discover_session_list(&mut workspace, &scope, !archived));
                }
                workspace.apply(Event::Completed(archive, Ok(Reply::Done)));
                assert_eq!(workspace.sessions[index].archived, archived);

                workspace.apply(Event::Completed(
                    list.unwrap(),
                    Ok(Reply::Sessions(vec![
                        summary(scope.clone(), "changed"),
                        summary(scope.clone(), "other"),
                        summary(scope.clone(), "discovered"),
                    ])),
                ));
                assert_eq!(workspace.sessions[index].archived, archived);
                assert_eq!(workspace.sessions[other].archived, !archived);
                assert_eq!(
                    workspace.sessions.last().unwrap().remote_id.as_deref(),
                    Some("discovered")
                );
                assert_eq!(workspace.sessions.last().unwrap().archived, !archived);

                // A fresh read can still reconcile a later change made outside Caelum.
                let fresh = discover_session_list(&mut workspace, &scope, !archived);
                workspace.apply(Event::Completed(
                    fresh,
                    Ok(Reply::Sessions(vec![summary(scope, "changed")])),
                ));
                assert_eq!(workspace.sessions[index].archived, !archived);
            }
        }
    }
}

#[test]
fn discovery_deferred_during_creation_keeps_its_original_archive_boundary() {
    for provider in Provider::ALL {
        let mut workspace = Workspace::default();
        let scope = scope(provider, "/a");
        let existing = workspace.new_session(scope.clone());
        workspace.sessions[existing].remote_id = Some("existing".into());
        let created = workspace.new_session(scope.clone());
        workspace.send(created, "Hello".into());
        let create = workspace.commands().pop().unwrap().0;
        let list = discover_session_list(&mut workspace, &scope, false);
        workspace.apply(Event::Completed(
            list,
            Ok(Reply::Sessions(vec![
                summary(scope.clone(), "existing"),
                summary(scope.clone(), "created"),
            ])),
        ));
        workspace.archive(existing, true);
        let archive = workspace.commands().pop().unwrap().0;
        workspace.apply(Event::Completed(archive, Ok(Reply::Done)));
        workspace.apply(Event::Completed(
            create,
            Ok(Reply::Session(summary(scope, "created"))),
        ));
        assert!(workspace.sessions[existing].archived);
        assert_eq!(
            workspace.sessions[created].remote_id.as_deref(),
            Some("created")
        );
        assert_eq!(workspace.sessions.len(), 2);
    }
}

#[test]
fn failed_archive_changes_do_not_block_discovery_reconciliation() {
    for archived in [false, true] {
        let mut workspace = Workspace::default();
        let scope = scope(Provider::OpenCode, "/a");
        let index = workspace.new_session(scope.clone());
        workspace.sessions[index].remote_id = Some("session".into());
        workspace.sessions[index].archived = !archived;
        let list = discover_session_list(&mut workspace, &scope, archived);
        workspace.archive(index, archived);
        let archive = workspace.commands().pop().unwrap().0;
        workspace.apply(Event::Completed(archive, Err("Connection lost".into())));
        workspace.apply(Event::Completed(
            list,
            Ok(Reply::Sessions(vec![summary(scope, "session")])),
        ));
        assert_eq!(workspace.sessions[index].archived, archived);
    }
}

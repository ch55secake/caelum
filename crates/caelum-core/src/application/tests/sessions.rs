use super::*;
use caelum_protocol::ProviderChoices;

#[test]
fn refresh_open_sessions_refreshes_every_open_active_session_only() {
    let mut app = Workspace::default();
    app.projects.extend([
        ProjectInfo {
            path: "/a".into(),
            name: "a".into(),
        },
        ProjectInfo {
            path: "/b".into(),
            name: "b".into(),
        },
    ]);
    let first = app.new_session(scope(Provider::Codex, "/a"));
    app.sessions[first].remote_id = Some("first".into());
    let second = app.new_session(scope(Provider::OpenCode, "/a"));
    app.sessions[second].remote_id = Some("second".into());
    let other_project = app.new_session(scope(Provider::Codex, "/b"));
    app.sessions[other_project].remote_id = Some("other-project".into());
    let archived = app.new_session(scope(Provider::Codex, "/a"));
    app.sessions[archived].remote_id = Some("archived".into());
    app.sessions[archived].archived = true;
    let closed_project = app.new_session(scope(Provider::Codex, "/closed"));
    app.sessions[closed_project].remote_id = Some("closed-project".into());

    app.refresh_open_sessions();

    let mut resumed: Vec<_> = app
        .commands()
        .into_iter()
        .filter_map(|(_, command)| match command {
            Command::Resume(key) => Some(key.id),
            _ => None,
        })
        .collect();
    resumed.sort();
    assert_eq!(resumed, ["first", "other-project", "second"]);
    assert!(app.sessions[first].loading);
    assert!(app.sessions[second].loading);
    assert!(app.sessions[other_project].loading);
    assert!(!app.sessions[archived].loading);
    assert!(!app.sessions[closed_project].loading);
}

#[test]
fn refresh_open_sessions_reloads_catalogues_for_each_open_scope_once() {
    let mut app = Workspace::default();
    for path in ["/a", "/b", "/archived", "/forgotten"] {
        app.projects.push(ProjectInfo {
            path: path.into(),
            name: path.into(),
        });
    }
    app.project_roots.insert("/linked".into(), "/b".into());
    let first = app.new_session(scope(Provider::Codex, "/a"));
    app.sessions[first].remote_id = Some("first".into());
    let duplicate = app.new_session(scope(Provider::Codex, "/a"));
    app.sessions[duplicate].remote_id = Some("duplicate".into());
    let busy = app.new_session(scope(Provider::OpenCode, "/b"));
    app.sessions[busy].remote_id = Some("busy".into());
    app.sessions[busy].submitting = true;
    app.new_session(scope(Provider::Codex, "/b"));
    app.new_session(scope(Provider::OpenCode, "/linked"));
    let archived = app.new_session(scope(Provider::OpenCode, "/archived"));
    app.sessions[archived].archived = true;
    let forgotten = app.new_session(scope(Provider::OpenCode, "/forgotten"));
    app.sessions[forgotten].forgotten = true;
    app.new_session(scope(Provider::OpenCode, "/closed"));
    app.select(first);
    app.sessions[first].draft = "Keep this draft".into();
    app.sessions[first].options.model = Some("saved-model".into());
    app.choices.insert(
        app.sessions[first].scope.clone(),
        ProviderChoices {
            models: vec![],
            agents: vec![],
            default_model: None,
        },
    );

    // The sidebar refreshes the active project before all open sessions.
    app.refresh_choices_for(scope(Provider::Codex, "/a"));
    app.refresh_project();
    app.refresh_open_sessions();
    app.refresh_open_sessions();

    let mut catalogues: Vec<_> = app
        .commands()
        .into_iter()
        .filter_map(|(_, command)| match command {
            Command::Choices(scope) => Some((scope.project, scope.provider.as_str())),
            _ => None,
        })
        .collect();
    catalogues.sort();
    assert_eq!(
        catalogues,
        [
            ("/a".into(), "codex"),
            ("/a".into(), "opencode"),
            ("/b".into(), "codex"),
            ("/b".into(), "opencode"),
            ("/linked".into(), "opencode"),
        ]
    );
    assert_eq!(app.selected, Some(first));
    assert_eq!(app.sessions[first].draft, "Keep this draft");
    assert_eq!(
        app.sessions[first].options.model.as_deref(),
        Some("saved-model")
    );
}

#[test]
fn disconnect_recovery_is_bounded_and_resuming_restored_queues_loads_history_first() {
    let mut app = Workspace::default();
    let index = started(&mut app, scope(Provider::Codex, "/a"), "s");
    app.send(index, "queued".into());
    app.apply(codex("/a", "s", "codex/processExited", json!({})));
    let (resume, _) = app.commands().pop().unwrap();
    app.apply(Event::Completed(resume, Err("still offline".into())));
    app.apply(codex("/a", "s", "codex/processExited", json!({})));
    assert!(app.commands().is_empty());
    app.resume_queue(index);
    assert!(matches!(
        app.commands().as_slice(),
        [(_, Command::Resume(_))]
    ));
    assert_eq!(app.sessions[index].queue.len(), 1);
}

#[test]
fn discovery_during_creation_adopts_the_stable_local_session_once() {
    let mut workspace = Workspace::default();
    let origin = scope(Provider::Codex, "/a");
    let index = workspace.new_session(origin.clone());
    workspace.refresh_project();
    let list = workspace.commands().into_iter().find(|(_, command)| matches!(command, Command::List(scope) if scope.provider == Provider::Codex)).unwrap().0;
    workspace.send(index, "Hello".into());
    let create = workspace.commands().pop().unwrap().0;
    workspace.apply(Event::Completed(
        list,
        Ok(Reply::Sessions(vec![summary(origin.clone(), "created")])),
    ));
    assert_eq!(workspace.sessions.len(), 1);
    workspace.apply(Event::Completed(
        create,
        Ok(Reply::Session(summary(origin, "created"))),
    ));
    assert_eq!(workspace.sessions.len(), 1);
    assert_eq!(
        workspace.sessions[index].remote_id.as_deref(),
        Some("created")
    );
    assert!(workspace.sessions[index].working());
}

#[test]
fn provider_session_discovery_includes_archived_history() {
    let mut workspace = Workspace::default();
    let scope = scope(Provider::OpenCode, "/a");
    workspace.discover_sessions(scope.clone());
    let commands = workspace.commands();
    let active = commands
        .iter()
        .find(|(_, command)| matches!(command, Command::List(current) if current == &scope))
        .unwrap()
        .0;
    let archived = commands
        .iter()
        .find(|(_, command)| matches!(command, Command::ListArchived(current) if current == &scope))
        .unwrap()
        .0;

    workspace.apply(Event::Completed(
        active,
        Ok(Reply::Sessions(vec![summary(scope.clone(), "active")])),
    ));
    workspace.apply(Event::Completed(
        archived,
        Ok(Reply::Sessions(vec![summary(scope, "archived")])),
    ));

    assert_eq!(workspace.sessions.len(), 2);
    assert!(!workspace.sessions[0].archived);
    assert!(workspace.sessions[1].archived);
}

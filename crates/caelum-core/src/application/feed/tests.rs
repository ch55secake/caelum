use crate::application::*;
use serde_json::json;

fn workspace() -> Workspace {
    let mut app = Workspace::default();
    for (path, name) in [
        ("/alpha", "Alpha"),
        ("/beta", "Beta"),
        ("/linked", "Linked"),
    ] {
        app.projects.push(caelum_protocol::ProjectInfo {
            path: path.into(),
            name: name.into(),
        });
        let i = app.new_session(Scope {
            project: path.into(),
            provider: Provider::Codex,
        });
        app.sessions[i].last_activity = Some(1000 + i as u64);
        app.sessions[i].remote_id = Some("shared-id".into());
        app.sessions[i].title = format!("Session {name}");
    }
    app.project_roots.insert("/linked".into(), "/alpha".into());
    app.select(0);
    app.commands();
    app
}

#[test]
fn scope_filters_include_linked_checkouts_but_exclude_closed_archived_and_forgotten_sessions() {
    let mut app = workspace();
    assert_eq!(app.session_feed(None, "", false), [2, 1, 0]);
    app.preferences.session_scope = SessionScope::CurrentProject;
    assert_eq!(app.session_feed(None, "", false), [2, 0]);
    app.active_project = None;
    assert!(app.session_feed(None, "", false).is_empty());
    app.select(0);
    assert!(app.session_feed(None, "Beta", false).is_empty());
    assert_eq!(app.session_feed(Some("/beta"), "codex", false), [1]);
    app.sessions[2].archived = true;
    assert_eq!(app.session_feed(None, "", true), [2]);
    app.sessions[2].forgotten = true;
    assert!(app.session_feed(None, "", true).is_empty());
    app.preferences.session_scope = SessionScope::AllProjects;
    app.close_project("/beta");
    assert_eq!(app.session_feed(None, "", false), [0]);
}

#[test]
fn search_matches_project_names_branches_and_unicode_worktree_labels() {
    let mut app = workspace();
    app.projects[0].name = "Café".into();
    app.sessions[2].details = Some(caelum_protocol::SessionDetails {
        branch: Some("fix/scroll".into()),
        branches: vec![],
        branch_is_current: false,
        changes: None,
    });
    assert_eq!(app.session_feed(None, "CAFÉ", false), [2, 0]);
    assert_eq!(app.session_feed(None, "fix/scroll", false), [2]);
    app.sessions[0].last_activity = None;
    app.sessions[1].last_activity = Some(1002);
    assert_eq!(app.session_feed(None, "", false), [1, 2, 0]);
}

#[test]
fn recency_and_scope_roundtrip_and_old_snapshots_keep_unknown_recency() {
    let mut original = workspace();
    original.preferences.session_scope = SessionScope::CurrentProject;
    let value = serde_json::to_value(original.snapshot()).unwrap();
    let mut app = Workspace::default();
    app.enable_store("/unused-feed-test/workspace.json".into());
    let load = app.commands()[0].0;
    app.apply(Event::Completed(
        load,
        Ok(Reply::Snapshot(Some(Box::new(
            serde_json::from_value(value.clone()).unwrap(),
        )))),
    ));
    assert_eq!(app.preferences.session_scope, SessionScope::CurrentProject);
    assert_eq!(app.session_feed(None, "", false), [2, 0]);
    let mut legacy = value;
    legacy["preferences"]
        .as_object_mut()
        .unwrap()
        .remove("sessionScope");
    for session in legacy["sessions"].as_array_mut().unwrap() {
        session.as_object_mut().unwrap().remove("last_activity");
    }
    let legacy: Snapshot = serde_json::from_value(legacy).unwrap();
    assert_eq!(legacy.preferences.session_scope, SessionScope::AllProjects);
    assert!(legacy.sessions.iter().all(|s| s.last_activity.is_none()));
}

#[test]
fn selection_refresh_and_streamed_tokens_do_not_change_recency_but_turn_boundaries_do() {
    let mut app = workspace();
    app.select(1);
    assert_eq!(app.sessions[0].last_activity, Some(1000));
    assert!(app.send(0, "Start a turn".into()));
    let started = app.sessions[0].last_activity;
    assert!(started > Some(1002));
    let event = |method: &str, params| {
        Event::Codex(caelum_protocol::CodexEvent {
            project_path: "/alpha".into(),
            method: method.into(),
            params,
            request_id: None,
        })
    };
    for text in ["one", "two", "three"] {
        app.apply(event(
            "item/agentMessage/delta",
            json!({"threadId":"shared-id","itemId":"answer","delta":text}),
        ));
        assert_eq!(app.sessions[0].last_activity, started);
        assert_eq!(app.session_feed(None, "", false), [0, 2, 1]);
    }
    let send = app
        .commands()
        .into_iter()
        .find(|(_, c)| matches!(c, Command::Send(..)))
        .unwrap()
        .0;
    app.apply(Event::Completed(send, Ok(Reply::Done)));
    app.apply(event(
        "turn/completed",
        json!({"threadId":"shared-id", "turn":{"id":"turn"}}),
    ));
    assert!(app.sessions[0].last_activity >= started);
    // The completion refresh receives an older summary.
    let resume = app
        .commands()
        .into_iter()
        .find(|(_, c)| matches!(c, Command::Resume(..)))
        .unwrap()
        .0;
    let key = app.sessions[0].key().unwrap();
    let summary = Summary {
        key,
        updated_at: Some(42),
        title: String::new(),
        working: false,
        options: Options::default(),
    };
    let completed = app.sessions[0].last_activity;
    app.apply(Event::Completed(resume, Ok(Reply::Session(summary))));
    assert_eq!(app.sessions[0].last_activity, completed);
}

#[test]
fn discovery_retains_provider_recency_and_external_opencode_turns_bump_only_at_boundaries() {
    let mut app = Workspace::default();
    app.projects.push(caelum_protocol::ProjectInfo {
        path: "/alpha".into(),
        name: "Alpha".into(),
    });
    app.discover_open_sessions();
    let commands = app.commands();
    let at = 1_700_000_000_000;
    for (request, command) in commands {
        if let Command::List(scope) = command {
            let updated_at = Some(
                at + if scope.provider == Provider::Codex {
                    0
                } else {
                    1000
                },
            );
            app.apply(Event::Completed(
                request,
                Ok(Reply::Sessions(vec![Summary {
                    key: SessionKey {
                        scope,
                        id: "same-id".into(),
                    },
                    updated_at,
                    title: "Discovered".into(),
                    working: false,
                    options: Options::default(),
                }])),
            ));
        }
    }
    let index = app
        .sessions
        .iter()
        .position(|s| s.scope.provider == Provider::OpenCode)
        .unwrap();
    assert_eq!(app.sessions[index].last_activity, Some(at + 1000));
    let event = |kind: &str, created, delta| {
        Event::OpenCode(caelum_protocol::OpenCodeEvent {
            project_path: "/alpha".into(),
            event_type: kind.into(),
            data: json!({"sessionID":"same-id", "assistantMessageID":"answer", "delta":delta}),
            id: None,
            created: Some(created),
            location: None,
        })
    };
    app.apply(event("session.text.delta", at + 60_000, "First"));
    assert_eq!(app.sessions[index].last_activity, Some(at + 60_000));
    app.apply(event("session.text.delta", at + 120_000, " token"));
    assert_eq!(app.sessions[index].last_activity, Some(at + 60_000));
    app.apply(event("session.execution.succeeded", at + 180_000, ""));
    assert_eq!(app.sessions[index].last_activity, Some(at + 180_000));
    app.apply(event("session.execution.succeeded", at + 240_000, ""));
    assert_eq!(app.sessions[index].last_activity, Some(at + 180_000));
}

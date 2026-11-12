use super::*;

fn connected(app: &mut Workspace, provider: Provider, project: &str, id: &str) -> usize {
    let index = app.new_session(scope(provider, project));
    app.sessions[index].remote_id = Some(id.into());
    app.sessions[index].title = "Original title".into();
    index
}

fn event(
    project: &str,
    session: &str,
    id: &str,
    method: &str,
    mut data: serde_json::Value,
) -> Event {
    data["sessionID"] = session.into();
    Event::OpenCode(OpenCodeEvent {
        project_path: project.into(),
        event_type: method.into(),
        id: Some(id.into()),
        created: None,
        data,
        location: None,
    })
}

fn succeeded(app: &mut Workspace, index: usize, turn: usize) {
    let key = app.sessions[index].key().unwrap();
    app.apply(event(
        &key.scope.project,
        &key.id,
        &format!("success-{turn}"),
        "session.execution.succeeded",
        json!({}),
    ));
}

fn requests(app: &mut Workspace) -> Vec<(u64, SessionKey)> {
    let mut titles = Vec::new();
    loop {
        let commands = app.commands();
        if commands.is_empty() {
            return titles;
        }
        for (request, command) in commands {
            match command {
                Command::RegenerateTitle(key) => titles.push((request, key)),
                Command::Save(..) => app.apply(Event::Completed(request, Ok(Reply::Done))),
                _ => {}
            }
        }
    }
}

#[test]
fn explicit_refreshes_reload_history_without_requesting_titles_or_resetting_the_turn_cadence() {
    for all_sessions in [false, true] {
        let mut app = Workspace::default();
        app.projects.push(ProjectInfo {
            path: "/a".into(),
            name: "Project".into(),
        });
        let index = connected(&mut app, Provider::OpenCode, "/a", "one");
        succeeded(&mut app, index, 1);
        succeeded(&mut app, index, 2);
        assert!(requests(&mut app).is_empty());
        app.sessions[index].draft = "Unsent draft".into();
        let refresh = |app: &mut Workspace| {
            if all_sessions {
                app.refresh_open_sessions();
            } else {
                app.refresh_session(index);
            }
        };
        refresh(&mut app);
        let commands = app.commands();
        assert!(commands
            .iter()
            .any(|(_, command)| matches!(command, Command::Resume(key) if key.id == "one")));
        assert!(!commands
            .iter()
            .any(|(_, command)| matches!(command, Command::RegenerateTitle(_))));
        refresh(&mut app);
        assert!(requests(&mut app).is_empty());
        assert_eq!(app.sessions[index].title, "Original title");
        assert_eq!(app.sessions[index].draft, "Unsent draft");

        succeeded(&mut app, index, 3);
        let pending = requests(&mut app);
        assert_eq!(pending.len(), 1);
        app.apply(Event::Completed(pending[0].0, Ok(Reply::Done)));
        refresh(&mut app);
        assert!(requests(&mut app).is_empty());
        for turn in 4..=5 {
            succeeded(&mut app, index, turn);
            assert!(requests(&mut app).is_empty());
        }
        succeeded(&mut app, index, 6);
        assert_eq!(requests(&mut app).len(), 1);
    }
}

#[test]
fn titles_regenerate_every_three_successes_and_apply_live_renames() {
    let mut app = Workspace::default();
    let index = connected(&mut app, Provider::OpenCode, "/a", "one");
    for turn in 1..=6 {
        app.apply(event(
            "/a",
            "one",
            &format!("start-{turn}"),
            "session.execution.started",
            json!({}),
        ));
        succeeded(&mut app, index, turn);
        let pending = requests(&mut app);
        if turn % 3 != 0 {
            assert!(pending.is_empty());
            continue;
        }
        assert_eq!(pending.len(), 1);
        let (request, key) = &pending[0];
        assert_eq!(key, &app.sessions[index].key().unwrap());
        let title = format!("Updated title {turn}");
        app.apply(event(
            "/a",
            "one",
            &format!("rename-{turn}"),
            "session.renamed",
            json!({"title": title}),
        ));
        app.apply(Event::Completed(*request, Ok(Reply::Done)));
        assert_eq!(app.sessions[index].title, title);
        assert_eq!(app.snapshot().sessions[index].title, title);
        assert!(requests(&mut app).is_empty());
    }
}

#[test]
fn titles_count_success_events_once_even_before_admission_or_after_idle() {
    let mut app = Workspace::default();
    let index = connected(&mut app, Provider::OpenCode, "/a", "one");
    app.sessions[index].submitting = true;
    succeeded(&mut app, index, 1);
    succeeded(&mut app, index, 1);
    for (i, method) in [
        "session.execution.failed",
        "session.execution.interrupted",
        "session.step.ended",
        "session.tool.success",
        "session.shell.ended",
        "session.idle",
    ]
    .into_iter()
    .enumerate()
    {
        app.apply(event("/a", "one", &format!("other-{i}"), method, json!({})));
    }
    assert!(!app.sessions[index].conversation.working);
    succeeded(&mut app, index, 2);
    assert!(requests(&mut app).is_empty());
    succeeded(&mut app, index, 3);
    assert_eq!(requests(&mut app).len(), 1);
}

#[test]
fn titles_keep_independent_cadences_and_original_project_provider_session_ownership() {
    let mut app = Workspace::default();
    let first = connected(&mut app, Provider::OpenCode, "/a", "one");
    let same_project = connected(&mut app, Provider::OpenCode, "/a", "two");
    let same_id = connected(&mut app, Provider::OpenCode, "/b", "one");
    let other_provider = connected(&mut app, Provider::Codex, "/a", "one");
    for turn in 1..=3 {
        app.apply(codex("/a", "one", "turn/completed", json!({})));
        succeeded(&mut app, first, turn);
        if turn == 1 {
            succeeded(&mut app, same_project, turn);
            succeeded(&mut app, same_id, turn);
        }
    }
    assert_eq!(app.selected, Some(other_provider));
    let pending = requests(&mut app);
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].1, app.sessions[first].key().unwrap());
    app.apply(event(
        "/a",
        "one",
        "renamed",
        "session.renamed",
        json!({"title":"New direction"}),
    ));
    app.apply(Event::Completed(pending[0].0, Ok(Reply::Done)));
    assert_eq!(app.sessions[first].title, "New direction");
    for index in [same_project, same_id, other_provider] {
        assert_eq!(app.sessions[index].title, "Original title");
    }
    for turn in 2..=3 {
        succeeded(&mut app, same_id, turn);
    }
    let pending = requests(&mut app);
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].1, app.sessions[same_id].key().unwrap());
}

#[test]
fn titles_coalesce_turns_while_one_request_is_in_flight() {
    let mut app = Workspace::default();
    let index = connected(&mut app, Provider::OpenCode, "/a", "one");
    for turn in 1..=3 {
        succeeded(&mut app, index, turn);
    }
    let first = requests(&mut app)[0].0;
    for turn in 4..=12 {
        succeeded(&mut app, index, turn);
        assert!(requests(&mut app).is_empty());
    }
    app.apply(Event::Completed(first, Ok(Reply::Done)));
    let pending = requests(&mut app);
    assert_eq!(pending.len(), 1);
    let second = pending[0].0;
    app.apply(Event::Completed(first, Ok(Reply::Done)));
    assert!(requests(&mut app).is_empty());
    app.apply(Event::Completed(second, Ok(Reply::Done)));
    assert!(requests(&mut app).is_empty());
    for turn in 13..=14 {
        succeeded(&mut app, index, turn);
    }
    assert!(requests(&mut app).is_empty());
    succeeded(&mut app, index, 15);
    assert_eq!(requests(&mut app).len(), 1);
}

#[test]
fn title_failures_preserve_chat_and_wait_another_interval_before_retrying() {
    let mut app = Workspace::default();
    let index = connected(&mut app, Provider::OpenCode, "/a", "one");
    for turn in 1..=3 {
        succeeded(&mut app, index, turn);
    }
    let request = requests(&mut app)[0].0;
    assert!(app.send(index, "Continue while naming is in flight".into()));
    assert!(app.send(index, "Queued work".into()));
    app.sessions[index].draft = "Unsent draft".into();
    let before = app.snapshot();
    app.apply(Event::Completed(
        request,
        Err("title model unavailable".into()),
    ));
    assert_eq!(app.snapshot(), before);
    assert!(app.sessions[index].error.is_none());
    assert!(!app.sessions[index].queue_paused);
    assert!(app.sessions[index].working());
    assert!(requests(&mut app).is_empty());
    for turn in 4..=5 {
        succeeded(&mut app, index, turn);
    }
    assert!(requests(&mut app).is_empty());
    succeeded(&mut app, index, 6);
    assert_eq!(requests(&mut app).len(), 1);
}

#[test]
fn title_followups_do_not_target_archived_forgotten_disconnected_or_replaced_sessions() {
    for state in [
        "archived",
        "archiving",
        "forgotten",
        "deleting",
        "disconnected",
        "replaced",
    ] {
        let mut app = Workspace::default();
        let index = connected(&mut app, Provider::OpenCode, "/a", "one");
        for turn in 1..=3 {
            succeeded(&mut app, index, turn);
        }
        let request = requests(&mut app)[0].0;
        for turn in 4..=6 {
            succeeded(&mut app, index, turn);
        }
        assert!(requests(&mut app).is_empty());
        let session = &mut app.sessions[index];
        match state {
            "archived" => session.archived = true,
            "archiving" => session.archiving = true,
            "forgotten" => session.forgotten = true,
            "deleting" => session.deleting = true,
            "disconnected" => session.disconnected = true,
            "replaced" => session.remote_id = Some("replacement".into()),
            _ => unreachable!(),
        }
        app.apply(Event::Completed(request, Ok(Reply::Done)));
        assert!(requests(&mut app).is_empty(), "{state}");
    }
}

#[test]
fn live_title_renames_survive_stale_resume_snapshots() {
    for provider in Provider::ALL {
        let mut app = Workspace::default();
        let index = connected(&mut app, provider, "/a", "one");
        app.refresh(index);
        let resume = app
            .commands()
            .into_iter()
            .find(|(_, c)| matches!(c, Command::Resume(_)))
            .unwrap()
            .0;
        let renamed = match provider {
            Provider::OpenCode => event(
                "/a",
                "one",
                "rename",
                "session.renamed",
                json!({"title":"Fresh title"}),
            ),
            Provider::Codex => codex(
                "/a",
                "one",
                "thread/name/updated",
                json!({"threadName":"Fresh title"}),
            ),
        };
        app.apply(renamed);
        app.apply(Event::Completed(
            resume,
            Ok(Reply::Session(summary(scope(provider, "/a"), "one"))),
        ));
        assert_eq!(app.sessions[index].title, "Fresh title");
        let history = app
            .commands()
            .into_iter()
            .find(|(_, c)| matches!(c, Command::History(..)))
            .unwrap()
            .0;
        app.apply(Event::Completed(history, Ok(page(vec![], false))));
        app.refresh(index);
        let resume = app
            .commands()
            .into_iter()
            .find(|(_, c)| matches!(c, Command::Resume(_)))
            .unwrap()
            .0;
        let mut latest = summary(scope(provider, "/a"), "one");
        latest.title = "Later title".into();
        app.apply(Event::Completed(resume, Ok(Reply::Session(latest))));
        assert_eq!(app.sessions[index].title, "Later title");
    }
}

#[test]
fn title_cadence_survives_restart_and_old_snapshots_default_to_zero() {
    let mut app = Workspace::default();
    let index = connected(&mut app, Provider::OpenCode, "/a", "one");
    succeeded(&mut app, index, 1);
    succeeded(&mut app, index, 2);
    let mut saved = serde_json::to_value(app.snapshot()).unwrap();
    // Snapshots from manual title refreshes may still contain the old cooldown.
    saved["sessions"][index]["title_refreshed_at"] = json!(1_000);
    let snapshot: Snapshot = serde_json::from_value(saved.clone()).unwrap();
    let mut restored = Workspace::default();
    restored.enable_store("/unused-in-reducer-test".into());
    let load = restored.commands().pop().unwrap().0;
    restored.apply(Event::Completed(
        load,
        Ok(Reply::Snapshot(Some(Box::new(snapshot)))),
    ));
    restored.refresh_session(index);
    assert!(requests(&mut restored).is_empty());
    succeeded(&mut restored, index, 3);
    let save = restored
        .commands()
        .into_iter()
        .find(|(_, c)| matches!(c, Command::Save(..)))
        .unwrap()
        .0;
    restored.apply(Event::Completed(save, Ok(Reply::Done)));
    assert_eq!(requests(&mut restored).len(), 1);

    saved["sessions"][index]
        .as_object_mut()
        .unwrap()
        .remove("title_refresh_turns");
    let legacy: Snapshot = serde_json::from_value(saved).unwrap();
    assert_eq!(legacy.sessions[index].title_refresh_turns, 0);
}

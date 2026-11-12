use super::{State, age};
use caelum_core::application::{
    Command, Event, Provider, Reply, Scope, SessionScope, Task, TaskReply, Workspace,
};

#[test]
fn relative_time_has_exact_deadlines_and_handles_unknown_or_future_timestamps() {
    assert_eq!(age(None, 1000), (String::new(), None));
    assert_eq!(age(Some(1000), 500), ("now".into(), Some(61_000)));
    assert_eq!(age(Some(1000), 61_000), ("1m".into(), Some(121_000)));
    assert_eq!(age(Some(1000), 3_601_000), ("1h".into(), Some(7_201_000)));
    assert_eq!(
        age(Some(1000), 86_401_000),
        ("1d".into(), Some(172_801_000))
    );
}

#[test]
fn feed_discovers_all_open_scopes_once_and_forgets_closed_projects() {
    let mut app = Workspace::default();
    let mut state = State::default();
    for path in ["/first", "/second"] {
        app.projects.push(caelum_protocol::ProjectInfo {
            path: path.into(),
            name: path.into(),
        });
    }
    state.discover(&mut app);
    let commands = app.commands();
    assert_eq!(
        commands
            .iter()
            .filter(|(_, c)| matches!(c, Command::List(_)))
            .count(),
        4
    );
    assert_eq!(
        commands
            .iter()
            .filter(|(_, c)| matches!(c, Command::ListArchived(_)))
            .count(),
        4
    );
    for (request, _) in commands {
        app.apply(Event::Completed(request, Ok(Reply::Sessions(vec![]))));
    }
    state.discover(&mut app);
    assert!(app.commands().is_empty());
    app.close_project("/second");
    state.discover(&mut app);
    assert!(state.discovered.iter().all(|s| s.project == "/first"));
}

#[test]
fn recency_polling_changes_only_visible_labels_and_hidden_feeds_stop_scheduling() {
    let mut app = Workspace::default();
    app.projects.push(caelum_protocol::ProjectInfo {
        path: "/first".into(),
        name: "First".into(),
    });
    let index = app.new_session(Scope {
        project: "/first".into(),
        provider: Provider::Codex,
    });
    let mut state = State::default();
    let mut tx = rune::Tx::default();
    state.poll(&mut app, "", false, &mut tx, false);
    let now = caelum_core::application::presentation::now();
    app.sessions[index].last_activity = Some(now - 90_000);
    state.now = now - 60_000;
    state.visible = vec![index];
    assert!(state.poll(&mut app, "", false, &mut tx, true));
    assert!(!state.poll(&mut app, "", false, &mut tx, true));
    assert!(state.deadline.is_some());
    state.hidden();
    assert!(!state.poll(&mut app, "", false, &mut tx, true));
    assert!(state.deadline.is_none());
}

#[test]
fn pull_requests_refresh_on_scope_and_navigation_changes_but_not_search_or_idle_frames() {
    let mut app = Workspace::default();
    for path in ["/first", "/second"] {
        app.projects.push(caelum_protocol::ProjectInfo {
            path: path.into(),
            name: path.into(),
        });
        app.new_session(Scope {
            project: path.into(),
            provider: Provider::Codex,
        });
        app.services.checkouts.entry(path.into()).or_default().git =
            Some(caelum_protocol::GitRepositoryStatus {
                project_path: path.into(),
                repository_root: Some(path.into()),
                branch: Some("feature".into()),
                changes: vec![],
            });
    }
    app.select(0);
    let mut state = State::default();
    let mut tx = rune::Tx::default();
    state.poll(&mut app, "", false, &mut tx, false);
    assert_eq!(finish_refreshes(&mut app), ["/first", "/second"]);
    for (query, archived) in [("", false), ("search", false), ("", true)] {
        state.poll(&mut app, query, archived, &mut tx, false);
        assert!(app.commands().is_empty());
    }
    state.project = Some("/first".into());
    state.poll(&mut app, "", false, &mut tx, false);
    assert_eq!(finish_refreshes(&mut app), ["/first"]);
    state.project = None;
    app.preferences.session_scope = SessionScope::CurrentProject;
    state.poll(&mut app, "", false, &mut tx, false);
    assert_eq!(finish_refreshes(&mut app), ["/first"]);
    app.select(1);
    state.poll(&mut app, "", false, &mut tx, false);
    assert_eq!(finish_refreshes(&mut app), ["/second"]);
    app.preferences.session_scope = SessionScope::AllProjects;
    state.poll(&mut app, "", false, &mut tx, false);
    assert_eq!(finish_refreshes(&mut app), ["/first", "/second"]);
    app.select(0);
    state.poll(&mut app, "", false, &mut tx, false);
    assert_eq!(finish_refreshes(&mut app), ["/first"]);
    state.project = Some("/second".into());
    state.poll(&mut app, "", false, &mut tx, false);
    assert_eq!(finish_refreshes(&mut app), ["/second"]);
    app.close_project("/second");
    state.poll(&mut app, "", false, &mut tx, false);
    assert!(state.project.is_none());
    assert_eq!(finish_refreshes(&mut app), ["/first"]);
}

fn finish_refreshes(app: &mut Workspace) -> Vec<String> {
    let mut projects = Vec::new();
    for (request, command) in app.commands() {
        let reply = match command {
            Command::Task(Task::Git { project }) => {
                projects.push(project.clone());
                TaskReply::Git(
                    app.services.checkouts[&project].git.clone().unwrap(),
                    vec![],
                )
            }
            Command::Task(Task::Discover { .. }) => TaskReply::Discovered(None),
            _ => panic!("unexpected refresh command"),
        };
        app.apply(Event::Completed(request, Ok(Reply::Task(reply))));
    }
    projects
}

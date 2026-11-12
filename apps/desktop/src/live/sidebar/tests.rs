use super::{State, age};
use caelum_core::application::{Command, Event, Provider, Reply, Scope, Workspace};

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
    state.poll(&app, "", false, &mut tx, false);
    let now = caelum_core::application::presentation::now();
    app.sessions[index].last_activity = Some(now - 90_000);
    state.now = now - 60_000;
    state.visible = vec![index];
    assert!(state.poll(&app, "", false, &mut tx, true));
    assert!(!state.poll(&app, "", false, &mut tx, true));
    assert!(state.deadline.is_some());
    state.hidden();
    assert!(!state.poll(&app, "", false, &mut tx, true));
    assert!(state.deadline.is_none());
}

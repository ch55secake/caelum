use super::*;
use crate::application::{Event, Snapshot};
use caelum_protocol::ProjectInfo;

fn workspace() -> Workspace {
    let mut app = Workspace::default();
    for path in ["/project", "/worktree"] {
        app.projects.push(ProjectInfo {
            name: path.into(),
            path: path.into(),
        });
    }
    app.new_session(Scope {
        project: "/project".into(),
        provider: Provider::Codex,
    });
    app.sessions[0].draft = "Existing conversation draft".into();
    app.home_select_project("/worktree".into());
    app.home_set_provider(Provider::OpenCode);
    app.home.draft.options.model = Some("home-model".into());
    app.home.draft.text = "Start something new".into();
    app.home.draft.attachments.push(Attachment::new(
        "notes.rs".into(),
        "Context".into(),
        None,
        false,
    ));
    app.commands();
    app
}

#[test]
fn home_submission_owns_a_fresh_session_with_exact_scope_and_options() {
    let mut app = workspace();
    let index = app.send_home().unwrap();
    assert_eq!(index, 1);
    assert_eq!(app.selected, Some(index));
    assert_eq!(app.sessions[0].draft, "Existing conversation draft");
    assert_eq!(
        app.sessions[index].scope,
        Scope {
            project: "/worktree".into(),
            provider: Provider::OpenCode
        }
    );
    assert_eq!(
        app.sessions[index].options.model.as_deref(),
        Some("home-model")
    );
    assert!(app.home.draft.text.is_empty());
    assert!(app.home.draft.attachments.is_empty());
    assert!(app
        .commands()
        .iter()
        .any(|(_, c)| matches!(c, Command::Start(scope, _) if scope.project == "/worktree")));
    assert!(app.send_home().is_none());
    assert_eq!(app.sessions.len(), 2);
}

#[test]
fn missing_target_or_invalid_submission_keeps_home_and_existing_session() {
    let mut app = workspace();
    let draft = app.home.draft.clone();
    app.projects.clear();
    assert!(app.send_home().is_none());
    assert_eq!(app.home.draft, draft);
    assert_eq!(app.sessions.len(), 1);
    app.projects.push(ProjectInfo {
        name: "Worktree".into(),
        path: "/worktree".into(),
    });
    app.home.draft.text = "/unknown".into();
    assert!(app.send_home().is_none());
    assert_eq!(app.sessions.len(), 1);
    assert_eq!(app.selected, Some(0));
    assert_eq!(app.home.draft.text, "/unknown");
    assert_eq!(app.home.draft.attachments, draft.attachments);
}

#[test]
fn project_opening_is_scoped_and_never_creates_or_selects_a_session() {
    let mut app = workspace();
    app.home_open_project("/old".into());
    let first = app
        .commands()
        .into_iter()
        .find(|(_, c)| matches!(c, Command::Open(_)))
        .unwrap()
        .0;
    app.home_open_project("/new".into());
    let second = app
        .commands()
        .into_iter()
        .find(|(_, c)| matches!(c, Command::Open(_)))
        .unwrap()
        .0;
    app.apply(Event::Completed(
        second,
        Ok(Reply::Project(ProjectInfo {
            name: "New".into(),
            path: "/new".into(),
        })),
    ));
    app.apply(Event::Completed(
        first,
        Ok(Reply::Project(ProjectInfo {
            name: "Old".into(),
            path: "/old".into(),
        })),
    ));
    assert_eq!(app.home.draft.project.as_deref(), Some("/new"));
    assert_eq!(app.selected, Some(0));
    assert_eq!(app.sessions.len(), 1);
    assert_eq!(app.home.draft.text, "Start something new");
    assert!(!app.home.opening());
}

#[test]
fn stale_attachment_does_not_change_a_new_target_or_its_pending_count() {
    let mut app = workspace();
    app.home_attach_image("Old.png".into(), vec![]);
    let first = app
        .commands()
        .into_iter()
        .find(|(_, c)| matches!(c, Command::Task(Task::EncodeImage { .. })))
        .unwrap()
        .0;
    assert!(app.send_home().is_none());
    app.home_select_project("/project".into());
    app.home_attach_image("New.png".into(), vec![]);
    let second = app
        .commands()
        .into_iter()
        .find(|(_, c)| matches!(c, Command::Task(Task::EncodeImage { .. })))
        .unwrap()
        .0;
    app.apply(Event::Completed(
        first,
        Ok(Reply::Task(TaskReply::Image("old-data".into()))),
    ));
    assert_eq!(app.home.attaching, 1);
    let image = "new-data".repeat(20_000);
    app.apply(Event::Completed(
        second,
        Ok(Reply::Task(TaskReply::Image(image.clone()))),
    ));
    assert_eq!(app.home.attaching, 0);
    assert_eq!(app.home.draft.attachments.last().unwrap().content, image);
    assert!(!app
        .home
        .draft
        .attachments
        .iter()
        .any(|a| a.content == "old-data"));
}

#[test]
fn home_round_trips_and_older_snapshots_default_to_an_empty_home() {
    let app = workspace();
    let snapshot = app.snapshot();
    let json = serde_json::to_value(&snapshot).unwrap();
    let restored: Snapshot = serde_json::from_value(json.clone()).unwrap();
    assert_eq!(restored.home, app.home.draft);
    let mut old = json;
    old.as_object_mut().unwrap().remove("home");
    assert_eq!(
        serde_json::from_value::<Snapshot>(old).unwrap().home,
        HomeDraft::default()
    );
    let mut fresh = Workspace::default();
    fresh.enable_store("/unused-in-home-test".into());
    let id = fresh.commands()[0].0;
    fresh.apply(Event::Completed(
        id,
        Ok(Reply::Snapshot(Some(Box::new(restored)))),
    ));
    assert_eq!(fresh.home.draft, snapshot.home);
    fresh.home.draft.text = "Typing during launch".into();
    fresh.home.restore(HomeDraft::default());
    assert_eq!(fresh.home.draft.text, "Typing during launch");
}

#[test]
fn returning_home_invalidates_an_older_project_navigation() {
    let mut app = workspace();
    app.open_project("/late".into());
    let id = app
        .commands()
        .into_iter()
        .find(|(_, c)| matches!(c, Command::Open(_)))
        .unwrap()
        .0;
    app.leave_conversation();
    app.apply(Event::Completed(
        id,
        Ok(Reply::Project(ProjectInfo {
            name: "Late".into(),
            path: "/late".into(),
        })),
    ));
    assert_eq!(app.selected, None);
    assert_eq!(app.sessions.len(), 1);
}

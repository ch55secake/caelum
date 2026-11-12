use super::*;
use crate::application::{Attachment, Command, Event, Provider, Reply, Scope};
use serde_json::{json, Value};

fn workspace() -> Workspace {
    let mut app = Workspace::default();
    for project in ["/a", "/b", "/c"] {
        app.new_session(Scope {
            project: project.into(),
            provider: Provider::Codex,
        });
    }
    app.sessions[0].attachments.push(Attachment::new(
        "src/context.rs".into(),
        "saved context".into(),
        None,
        false,
    ));
    app
}

#[test]
fn every_persisted_field_participates_in_change_detection() {
    let app = workspace();
    let original = app.snapshot();
    assert!(app.matches_snapshot(&original));
    let check = |section: Option<&str>, key: &str, value: Value| {
        let mut data = serde_json::to_value(&original).unwrap();
        let target = match section {
            Some("sessions") => &mut data["sessions"][0],
            Some(section) => &mut data[section],
            None => &mut data,
        };
        target[key] = value;
        let changed: Snapshot = serde_json::from_value(data).unwrap();
        assert_ne!(changed, original, "{section:?}/{key}");
        assert!(!app.matches_snapshot(&changed), "{section:?}/{key}");
    };
    for (key, value) in [
        ("last_activity", json!(12)),
        ("scope", json!({"project":"/other","provider":"opencode"})),
        ("remote_id", json!("remote")),
        ("title", json!("New title")),
        ("title_refresh_turns", json!(2)),
        ("options", json!({"model":"new-model"})),
        ("draft", json!("edited draft")),
        ("attachments", json!([])),
        ("failed_prompt", json!("retry me")),
        ("pending_submission", json!({"text":"pending","options":{}})),
        ("queue", json!([{"text":"queued","options":{}}])),
        (
            "inbox",
            json!([{"id":"queued","text":"pending","state":"pending","created":1,"pending":false}]),
        ),
        ("archived", json!(true)),
        ("reading", json!(12.5)),
        ("unread", json!(true)),
    ] {
        check(Some("sessions"), key, value);
    }
    for (key, value) in [
        ("files", json!({"/a":{"tabs":["src/a.rs"]}})),
        ("theme", json!("dark")),
        ("themeFamily", json!("nord")),
        ("accentColor", json!("teal")),
        ("sessionScope", json!("currentProject")),
        ("defaultProvider", json!("opencode")),
        ("worktreeDirectory", json!("checkouts")),
        ("worktreeExpanded", json!({"/a":true})),
        ("providers", json!({"codex":{"model":"new-model"}})),
        ("favoriteModels", json!({"codex":["new-model"]})),
        ("notifications", json!(false)),
        ("notificationMode", json!("always")),
        ("soundVolume", json!(75)),
        ("backgroundShells", json!(false)),
        ("inputNeeded", json!(false)),
    ] {
        check(Some("preferences"), key, value);
    }
    for (key, value) in [
        ("version", json!(2)),
        ("projects", json!([{"name":"Project","path":"/a"}])),
        ("sessions", json!([])),
        ("selected", json!(0)),
        ("remembered", json!({"/a":1})),
        ("remembered_checkouts", json!({"/a":1})),
        ("managed_worktrees", json!(["/a/tree"])),
        ("project_roots", json!({"/a/tree":"/a"})),
    ] {
        check(None, key, value);
    }
}

#[test]
fn projection_handles_forgotten_indices_and_live_file_overrides() {
    let mut app = workspace();
    app.sessions[1].forgotten = true;
    app.remembered.insert("/forgotten".into(), 1);
    app.remembered.insert("/invalid".into(), usize::MAX);
    app.remembered_checkouts.insert("/forgotten".into(), 1);
    app.remembered_checkouts
        .insert("/invalid".into(), usize::MAX);
    for path in ["", "/a", "/saved-only"] {
        let files = app.preferences.files.entry(path.into()).or_default();
        files.tabs = vec!["remembered.rs".into()];
        files.selected = Some("remembered.rs".into());
        files.scroll.insert("remembered.rs".into(), [2.0, 42.0]);
        files.line.insert("remembered.rs".into(), 5);
        files.expanded.insert("src".into());
    }
    for path in ["", "/a", "/checkout-only"] {
        let checkout = app.services.checkouts.entry(path.into()).or_default();
        checkout.tabs = vec!["live.rs".into()];
        checkout.selected_file = Some("live.rs".into());
    }
    for selected in [None, Some(0), Some(1), Some(2), Some(500)] {
        app.selected = selected;
        let snapshot = app.snapshot();
        assert!(app.matches_snapshot(&snapshot));
        let mut changed = snapshot.clone();
        changed.preferences.files.get_mut("/a").unwrap().selected = None;
        assert!(!app.matches_snapshot(&changed));
    }
    let snapshot = app.snapshot();
    app.preferences.files.get_mut("/a").unwrap().tabs.clear();
    app.services.checkouts.get_mut("").unwrap().tabs.clear();
    app.sessions[1].draft = "forgotten edits".into();
    assert_eq!(app.snapshot(), snapshot);
    assert!(app.matches_snapshot(&snapshot));
    for field in ["scroll", "line", "expanded"] {
        let mut data = serde_json::to_value(&snapshot).unwrap();
        data["preferences"]["files"]["/a"][field] = if field == "expanded" {
            json!([])
        } else {
            json!({})
        };
        let changed = serde_json::from_value(data).unwrap();
        assert!(!app.matches_snapshot(&changed), "{field}");
    }
    app.services.checkouts.remove("/checkout-only");
    assert!(!app.matches_snapshot(&snapshot));
}

#[test]
fn direct_edits_save_again_but_transcript_updates_do_not() {
    let mut app = workspace();
    app.enable_store("/unused-in-reducer-test".into());
    let load = app
        .commands()
        .into_iter()
        .find(|(_, c)| matches!(c, Command::Load(_)))
        .unwrap()
        .0;
    app.apply(Event::Completed(load, Ok(Reply::Snapshot(None))));
    let save = app
        .commands()
        .into_iter()
        .find(|(_, c)| matches!(c, Command::Save(..)))
        .unwrap()
        .0;
    app.apply(Event::Completed(save, Ok(Reply::Done)));
    // Warm command queues before exercising otherwise idle polling.
    app.commands();
    app.sessions[0]
        .conversation
        .optimistic("message".into(), "transcript only".into());
    assert!(app.commands().is_empty());
    app.sessions[0].attachments[0].content = "edited context".into();
    app.sessions[0].draft = "later typing".into();
    app.preferences.theme_family = "nord".into();
    let commands = app.commands();
    let [(save, Command::Save(_, snapshot))] = commands.as_slice() else {
        panic!("expected a new save")
    };
    assert_eq!(snapshot.sessions[0].draft, "later typing");
    assert_eq!(
        snapshot.sessions[0].attachments[0].content,
        "edited context"
    );
    app.sessions[0].draft = "typing during save".into();
    assert!(app.commands().is_empty());
    app.apply(Event::Completed(*save, Ok(Reply::Done)));
    assert!(app.commands().iter().any(
        |(_, c)| matches!(c, Command::Save(_, s) if s.sessions[0].draft == "typing during save")
    ));
}

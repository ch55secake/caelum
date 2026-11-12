use super::*;

#[test]
fn durable_submission_waits_for_its_snapshot_but_not_for_later_typing() {
    let mut app = Workspace::default();
    let index = app.new_session(scope(Provider::Codex, "/a"));
    app.enable_store("/unused-in-reducer-test".into());
    let (load, _) = app.commands().pop().unwrap();
    app.apply(Event::Completed(load, Ok(Reply::Snapshot(None))));
    app.sessions[index].draft = "durable prompt".into();
    assert!(app.send(index, "durable prompt".into()));
    let (save, command) = app.commands().pop().unwrap();
    let Command::Save(_, snapshot) = command else {
        panic!("save must precede provider admission")
    };
    assert_eq!(
        snapshot.sessions[index]
            .pending_submission
            .as_ref()
            .map(|p| p.text.as_str()),
        Some("durable prompt")
    );
    app.sessions[index].draft = "newer draft".into();
    assert!(app.commands().is_empty());
    app.apply(Event::Completed(save, Ok(Reply::Done)));
    let commands = app.commands();
    assert_eq!(
        commands
            .iter()
            .filter(|(_, c)| matches!(c, Command::Start(..)))
            .count(),
        1
    );
    assert!(commands.iter().any(
        |(_, c)| matches!(c, Command::Save(_, s) if s.sessions[index].draft == "newer draft")
    ));
}

#[test]
fn failed_disk_write_blocks_admission_and_retry_keeps_the_original_prompt() {
    let mut app = Workspace::default();
    let index = app.new_session(scope(Provider::Codex, "/a"));
    app.enable_store("/unused-in-reducer-test".into());
    let (load, _) = app.commands().pop().unwrap();
    app.apply(Event::Completed(load, Ok(Reply::Snapshot(None))));
    app.send(index, "keep through disk failure".into());
    let (save, _) = app.commands().pop().unwrap();
    app.apply(Event::Completed(save, Err("disk full".into())));
    assert!(app.commands().is_empty());
    assert_eq!(
        app.snapshot().sessions[index]
            .pending_submission
            .as_ref()
            .map(|p| p.text.as_str()),
        Some("keep through disk failure")
    );
    app.retry_save();
    let (save, _) = app.commands().pop().unwrap();
    app.apply(Event::Completed(save, Ok(Reply::Done)));
    assert!(matches!(
        app.commands().as_slice(),
        [(_, Command::Start(..))]
    ));
}

#[test]
fn workspace_roundtrip_keeps_drafts_and_unknown_versions_are_not_imported() {
    let directory = std::env::temp_dir().join(format!(
        "caelum-snapshot-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let store = Store(directory.join("workspace.json"));
    let mut app = Workspace::default();
    app.preferences.theme_family = "nord".into();
    app.preferences.accent_color = "teal".into();
    let index = app.new_session(scope(Provider::OpenCode, "/project with spaces"));
    app.sessions[index].draft = "café ☁️\nnext line".into();
    app.attach(
        index,
        Attachment::new("a.rs".into(), "内容".into(), Some((2, 2)), false),
    );
    store.save(&app.snapshot()).unwrap();
    assert_eq!(store.load().unwrap(), Some(app.snapshot()));
    for mode in ["light", "dark", "system"] {
        for black in [false, true] {
            let mut old = serde_json::to_value(app.snapshot()).unwrap();
            old["preferences"]["theme"] = mode.into();
            old["preferences"]["blackBackgrounds"] = black.into();
            std::fs::write(&store.0, serde_json::to_vec(&old).unwrap()).unwrap();
            let migrated = store.load().unwrap().unwrap();
            assert_eq!(migrated.preferences.theme, mode);
            assert_eq!(
                migrated.preferences.theme_family,
                if black { "vercel" } else { "nord" }
            );
            assert_eq!(
                migrated.preferences.accent_color,
                if black { "theme" } else { "teal" }
            );
            assert_eq!(migrated.sessions, app.snapshot().sessions);
            store.save(&migrated).unwrap();
            assert_eq!(store.load().unwrap(), Some(migrated.clone()));
            assert!(serde_json::to_value(migrated).unwrap()["preferences"]
                .get("blackBackgrounds")
                .is_none());
        }
    }
    let mut future = app.snapshot();
    future.version = 2;
    store.save(&future).unwrap();
    assert!(store.load().is_err());
    assert_eq!(
        serde_json::from_slice::<Snapshot>(&std::fs::read(&store.0).unwrap())
            .unwrap()
            .version,
        2
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn restart_preserves_unsent_and_unacknowledged_prompts_and_pauses_all_queues() {
    let mut original = Workspace::default();
    let index = original.new_session(scope(Provider::Codex, "/a"));
    original.sessions[index].failed_prompt = Some("previous failed prompt".into());
    original.send(index, "pending admission".into());
    original.send(index, "queued after admission".into());
    original.sessions[index].draft = "newer draft".into();
    let mut restored = Workspace::default();
    restored.enable_store("/unused-in-reducer-test".into());
    let (load, _) = restored.commands().pop().unwrap();
    restored.apply(Event::Completed(
        load,
        Ok(Reply::Snapshot(Some(Box::new(original.snapshot())))),
    ));
    assert_eq!(restored.sessions[index].draft, "newer draft");
    assert_eq!(
        restored.sessions[index].failed_prompt.as_deref(),
        Some("previous failed prompt")
    );
    assert_eq!(
        restored.sessions[index]
            .queue
            .iter()
            .map(|p| p.text.as_str())
            .collect::<Vec<_>>(),
        ["pending admission", "queued after admission"]
    );
    assert!(restored.sessions[index].queue_paused);
    assert!(!restored
        .commands()
        .iter()
        .any(|(_, c)| matches!(c, Command::Start(..) | Command::Send(..))));
}

#[test]
fn saving_errors_never_block_cancellation_or_permission_replies() {
    let mut app = Workspace::default();
    let index = started(&mut app, scope(Provider::Codex, "/a"), "s");
    app.enable_store("/unused-in-reducer-test".into());
    let (load, _) = app.commands().pop().unwrap();
    app.apply(Event::Completed(load, Ok(Reply::Snapshot(None))));
    let (save, _) = app.commands().pop().unwrap();
    app.apply(Event::Completed(save, Err("disk full".into())));
    app.cancel(index);
    app.approve(
        caelum_protocol::ApprovalPrompt {
            provider: "codex".into(),
            project_path: "/a".into(),
            session_id: Some("s".into()),
            request_id: "approval".into(),
            summary: "Test".into(),
            description: String::new(),
        },
        caelum_protocol::ApprovalDecision::Deny,
    );
    let commands = app.commands();
    assert!(commands
        .iter()
        .any(|(_, c)| matches!(c, Command::Cancel(_))));
    assert!(commands
        .iter()
        .any(|(_, c)| matches!(c, Command::Approve(..))));
}

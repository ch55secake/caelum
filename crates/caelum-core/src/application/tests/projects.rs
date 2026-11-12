use super::*;

#[test]
fn closing_project_groups_keeps_sessions_work_and_drafts_through_reopen_and_restore() {
    let mut app = Workspace::default();
    for path in ["/main", "/linked", "/other"] {
        app.projects.push(ProjectInfo {
            name: path.into(),
            path: path.into(),
        });
        let index = app.new_session(scope(Provider::Codex, path));
        app.sessions[index].draft = format!("Draft for {path}");
    }
    app.project_roots.insert("/linked".into(), "/main".into());
    app.sessions[0].remote_id = Some("thread-main".into());
    app.sessions[1].remote_id = Some("thread-linked".into());
    app.sessions[1].conversation.working = true;
    app.sessions[1].reading = 512.0;
    app.attach(
        1,
        Attachment::new("src/main.rs".into(), "snapshot".into(), None, false),
    );
    app.sessions[1].queue.push(QueuedPrompt {
        text: "Next turn".into(),
        options: Default::default(),
    });
    app.select(1);
    app.close_project("/main");
    assert_eq!(
        app.projects
            .iter()
            .map(|p| p.path.as_str())
            .collect::<Vec<_>>(),
        ["/other"]
    );
    assert_eq!(app.selected, Some(2));
    assert_eq!(app.sessions.len(), 3);
    assert!(app.sessions[1].working());
    assert!(!app.sessions[1].archived);
    assert_eq!(app.sessions[1].queue[0].text, "Next turn");
    assert!(
        app.commands().is_empty(),
        "close must not cancel, archive or remove anything"
    );
    app.apply(codex(
        "/linked",
        "thread-linked",
        "item/agentMessage/delta",
        json!({"itemId":"reply", "delta":"Continues while closed"}),
    ));
    assert!(app.sessions[1]
        .conversation
        .entries
        .iter()
        .any(|e| e.text.contains("Continues while closed")));
    app.open_project("/main".into());
    let open = app
        .commands()
        .into_iter()
        .find(|(_, c)| matches!(c, Command::Open(_)))
        .unwrap()
        .0;
    app.apply(Event::Completed(
        open,
        Ok(Reply::Project(ProjectInfo {
            path: "/main".into(),
            name: "Main".into(),
        })),
    ));
    assert_eq!(app.selected, Some(1));
    assert_eq!(app.sessions[1].draft, "Draft for /linked");
    assert_eq!(app.sessions[1].attachments[0].content, "snapshot");
    assert_eq!(app.sessions[1].reading, 512.0);
    app.close_project("/main");
    app.close_project("/other");
    assert!(app.projects.is_empty());
    assert_eq!(app.selected, None);
    assert_eq!(app.active_project, None);
    let snapshot: Snapshot =
        serde_json::from_value(serde_json::to_value(app.snapshot()).unwrap()).unwrap();
    let mut restored = Workspace::default();
    restored.enable_store("unused-project-close.json".into());
    let load = restored.commands().pop().unwrap().0;
    restored.apply(Event::Completed(
        load,
        Ok(Reply::Snapshot(Some(Box::new(snapshot)))),
    ));
    assert!(restored.projects.is_empty());
    assert!(restored.selected.is_none());
    assert!(!restored
        .commands()
        .iter()
        .any(|(_, c)| matches!(c, Command::Details(_) | Command::Resume(_))));
    restored.open_project("/main".into());
    let mut commands = restored.commands();
    if let Some((save, _)) = commands
        .iter()
        .find(|(_, c)| matches!(c, Command::Save(..)))
    {
        restored.apply(Event::Completed(*save, Ok(Reply::Done)));
        commands.extend(restored.commands());
    }
    let open = commands
        .into_iter()
        .find(|(_, c)| matches!(c, Command::Open(_)))
        .unwrap()
        .0;
    restored.apply(Event::Completed(
        open,
        Ok(Reply::Project(ProjectInfo {
            path: "/main".into(),
            name: "Main".into(),
        })),
    ));
    assert_eq!(restored.selected, Some(1));
    assert_eq!(restored.sessions[1].draft, "Draft for /linked");
    assert_eq!(restored.sessions[1].attachments[0].content, "snapshot");
    assert_eq!(restored.sessions[1].queue[0].text, "Next turn");
    let mut commands = restored.commands();
    if let Some((save, _)) = commands
        .iter()
        .find(|(_, c)| matches!(c, Command::Save(..)))
    {
        restored.apply(Event::Completed(*save, Ok(Reply::Done)));
        commands.extend(restored.commands());
    }
    assert_eq!(
        commands
            .iter()
            .filter(|(_, c)| matches!(c, Command::Details(_)))
            .count(),
        2
    );
    assert!(commands
        .iter()
        .any(|(_, c)| matches!(c, Command::Resume(key) if key.id == "thread-linked")));
}

#[test]
fn closed_projects_reject_late_open_and_worktree_discovery_without_changing_navigation() {
    let mut app = Workspace::default();
    for path in ["/main", "/other"] {
        app.projects.push(ProjectInfo {
            path: path.into(),
            name: path.into(),
        });
        app.new_session(scope(Provider::Codex, path));
    }
    app.task(Task::Worktrees {
        project: "/main".into(),
        directory: String::new(),
    });
    app.launch_worktree(
        Task::CreateWorktree {
            project: "/main".into(),
            branch: "new-branch".into(),
            base: "main".into(),
            directory: String::new(),
        },
        Provider::Codex,
    );
    app.open_project("/main".into());
    let commands = app.commands();
    let open = commands
        .iter()
        .find(|(_, c)| matches!(c, Command::Open(_)))
        .unwrap()
        .0;
    let listing = commands
        .iter()
        .find(|(_, c)| matches!(c, Command::Task(Task::Worktrees { .. })))
        .unwrap()
        .0;
    let create = commands
        .iter()
        .find(|(_, c)| matches!(c, Command::Task(Task::CreateWorktree { .. })))
        .unwrap()
        .0;
    app.close_project("/main");
    assert_eq!(
        app.selected,
        Some(1),
        "closing a background project retains selection"
    );
    app.apply(Event::Completed(
        open,
        Ok(Reply::Project(ProjectInfo {
            path: "/main".into(),
            name: "Main".into(),
        })),
    ));
    app.apply(Event::Completed(listing, Ok(Reply::Task(TaskReply::Worktrees(Some(serde_json::from_value(json!({
        "project":{"path":"/main","name":"Main"},"storageDirectory":"/worktrees","branches":["main"],"defaultBase":"main",
        "worktrees":[{"path":"/main","project":{"path":"/main","name":"Main"},"branch":"main","main":true,"locked":null,"prunable":null,"available":true},
        {"path":"/late-linked","project":{"path":"/late-linked","name":"Linked"},"branch":"feature","main":false,"locked":null,"prunable":null,"available":true}]
    })).unwrap()))))));
    app.apply(Event::Completed(create, Ok(Reply::Task(TaskReply::Worktree(serde_json::from_value(json!({
        "path":"/created-linked","project":{"path":"/created-linked","name":"Created"},"branch":"new-branch","main":false,"locked":null,"prunable":null,"available":true
    })).unwrap())))));
    assert_eq!(app.projects.len(), 1);
    assert_eq!(app.projects[0].path, "/other");
    assert!(!app.opening);
    assert_eq!(app.active_project.as_deref(), Some("/other"));
    assert_eq!(app.selected, Some(1));
    assert!(!app
        .commands()
        .iter()
        .any(|(_, c)| matches!(c, Command::Open(_) | Command::List(_))));

    // Creation may already have handed off to Open when the project is closed.
    app.open_project("/main".into());
    let open = app
        .commands()
        .into_iter()
        .find(|(_, c)| matches!(c, Command::Open(_)))
        .unwrap()
        .0;
    app.apply(Event::Completed(
        open,
        Ok(Reply::Project(ProjectInfo {
            path: "/main".into(),
            name: "Main".into(),
        })),
    ));
    app.launch_worktree(
        Task::CreateWorktree {
            project: "/main".into(),
            branch: "another".into(),
            base: "main".into(),
            directory: String::new(),
        },
        Provider::Codex,
    );
    let create = app
        .commands()
        .into_iter()
        .find(|(_, c)| matches!(c, Command::Task(Task::CreateWorktree { .. })))
        .unwrap()
        .0;
    app.apply(Event::Completed(create, Ok(Reply::Task(TaskReply::Worktree(serde_json::from_value(json!({
        "path":"/opening-linked","project":{"path":"/opening-linked","name":"Opening"},"branch":"another","main":false,"locked":null,"prunable":null,"available":true
    })).unwrap())))));
    let open = app
        .commands()
        .into_iter()
        .find(|(_, c)| matches!(c, Command::Open(_)))
        .unwrap()
        .0;
    app.close_project("/main");
    app.apply(Event::Completed(
        open,
        Ok(Reply::Project(ProjectInfo {
            path: "/opening-linked".into(),
            name: "Opening".into(),
        })),
    ));
    assert!(!app.project_is_open("/main"));
    assert!(!app.project_is_open("/opening-linked"));
    assert_eq!(app.projects.len(), 1);
    assert_eq!(app.selected, Some(1));

    // A pending background Undo cannot select a project after it was closed.
    app.projects.push(ProjectInfo {
        path: "/main".into(),
        name: "Main".into(),
    });
    app.sessions[0].remote_id = Some("archived".into());
    app.archive(0, true);
    let archive = app
        .commands()
        .into_iter()
        .find(|(_, c)| matches!(c, Command::Archive(_, true)))
        .unwrap()
        .0;
    app.apply(Event::Completed(archive, Ok(Reply::Done)));
    app.undo_archive();
    let restore = app
        .commands()
        .into_iter()
        .find(|(_, c)| matches!(c, Command::Archive(_, false)))
        .unwrap()
        .0;
    app.close_project("/main");
    app.apply(Event::Completed(restore, Ok(Reply::Done)));
    assert_eq!(app.selected, Some(1));
    assert!(!app.sessions[0].archived);
    assert!(!app.project_is_open("/main"));
}

#[test]
fn latest_project_selection_wins_when_validations_complete_out_of_order() {
    let mut workspace = Workspace::default();
    workspace.open_project("/a".into());
    let (a, _) = workspace.commands().pop().unwrap();
    workspace.open_project("/b".into());
    let (b, _) = workspace.commands().pop().unwrap();
    for (request, path) in [(b, "/b"), (a, "/a")] {
        workspace.apply(Event::Completed(
            request,
            Ok(Reply::Project(ProjectInfo {
                name: path.into(),
                path: path.into(),
            })),
        ));
    }
    assert_eq!(workspace.active_project.as_deref(), Some("/b"));
    assert_eq!(workspace.projects.len(), 2);
}

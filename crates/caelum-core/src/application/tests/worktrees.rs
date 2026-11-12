use super::*;

#[test]
fn managed_worktrees_exclude_external_checkouts_and_survive_storage_changes() {
    let mut app = Workspace::default();
    app.projects.push(ProjectInfo {
        path: "/main".into(),
        name: "Main".into(),
    });
    let selected = app.new_session(scope(Provider::Codex, "/main"));
    app.sessions[selected].draft = "Keep the project draft".into();
    let tree = |path: &str, main: bool| {
        json!({
            "path":path,"project":{"path":path,"name":"Main"},"branch":"same-branch",
            "main":main,"locked":null,"prunable":null,"available":true
        })
    };
    let listing = |storage: &str| {
        serde_json::from_value(json!({
            "project":{"path":"/main","name":"Main"},"storageDirectory":storage,
            "branches":["main","unused-branch"],"defaultBase":"main",
            "worktrees":[tree("/main", true), tree("/storage/repo/first", false),
                tree("/custom/repo/second", false), tree("/scratch/external", false)]
        }))
        .unwrap()
    };
    let refresh = |app: &mut Workspace, storage: &str| {
        app.task(Task::Worktrees {
            project: "/main".into(),
            directory: storage.into(),
        });
        let request = app
            .commands()
            .into_iter()
            .find(|(_, c)| matches!(c, Command::Task(Task::Worktrees { .. })))
            .unwrap()
            .0;
        app.apply(Event::Completed(
            request,
            Ok(Reply::Task(TaskReply::Worktrees(Some(listing(storage))))),
        ));
    };
    refresh(&mut app, "/storage/repo");
    assert_eq!(
        app.managed_worktrees("/main")
            .map(|t| t.path.as_str())
            .collect::<Vec<_>>(),
        ["/storage/repo/first"]
    );
    assert_eq!(app.group_path("/storage/repo/first"), "/main");
    assert!(!app
        .projects
        .iter()
        .any(|p| p.path == "/scratch/external" || p.path == "/custom/repo/second"));
    assert!(!app.commands().iter().any(|(_, command)| matches!(command, Command::List(scope) if scope.project == "/scratch/external")));
    assert_eq!(
        app.managed_worktrees("/storage/repo/first").count(),
        0,
        "only the parent owns the list"
    );

    app.task(Task::CreateWorktree {
        project: "/main".into(),
        branch: "work/second".into(),
        base: "main".into(),
        directory: "/custom".into(),
    });
    let create = app
        .commands()
        .into_iter()
        .find(|(_, c)| matches!(c, Command::Task(Task::CreateWorktree { .. })))
        .unwrap()
        .0;
    app.apply(Event::Completed(
        create,
        Ok(Reply::Task(TaskReply::Worktree(
            serde_json::from_value(tree("/custom/repo/second", false)).unwrap(),
        ))),
    ));
    app.preferences.worktree_directory = "/new-location".into();
    let snapshot = serde_json::from_value(serde_json::to_value(app.snapshot()).unwrap()).unwrap();
    let mut restored = Workspace::default();
    restored.enable_store("unused-managed-worktrees.json".into());
    let load = restored.commands().pop().unwrap().0;
    restored.apply(Event::Completed(
        load,
        Ok(Reply::Snapshot(Some(Box::new(snapshot)))),
    ));
    refresh(&mut restored, "/new-location/repo");
    assert_eq!(
        restored
            .managed_worktrees("/main")
            .map(|t| t.path.as_str())
            .collect::<Vec<_>>(),
        ["/storage/repo/first", "/custom/repo/second"]
    );
    assert_eq!(restored.group_path("/custom/repo/second"), "/main");
    assert_eq!(restored.selected, Some(selected));
    assert_eq!(restored.sessions[selected].draft, "Keep the project draft");
    assert!(!restored
        .projects
        .iter()
        .any(|p| p.path == "/scratch/external"));
    assert!(restored
        .services
        .worktrees("/main")
        .unwrap()
        .branches
        .contains(&"unused-branch".into()));

    // An explicitly opened checkout can own its own Caelum worktrees even when
    // another open project shares the same Git repository and branch catalogue.
    let drain = |app: &mut Workspace| {
        let mut outgoing = Vec::new();
        loop {
            let commands = app.commands();
            if commands.is_empty() {
                break;
            }
            for (id, command) in commands {
                if matches!(command, Command::Save(..)) {
                    app.apply(Event::Completed(id, Ok(Reply::Done)));
                } else {
                    outgoing.push((id, command));
                }
            }
        }
        outgoing
    };
    restored.projects.push(ProjectInfo {
        path: "/scratch/external".into(),
        name: "External project".into(),
    });
    restored.task(Task::CreateWorktree {
        project: "/scratch/external".into(),
        branch: "work/third".into(),
        base: "main".into(),
        directory: "/custom".into(),
    });
    let create = drain(&mut restored)
        .into_iter()
        .find(|(_, c)| matches!(c, Command::Task(Task::CreateWorktree { .. })))
        .unwrap()
        .0;
    let third: caelum_protocol::GitWorktree =
        serde_json::from_value(tree("/custom/repo/third", false)).unwrap();
    restored.apply(Event::Completed(
        create,
        Ok(Reply::Task(TaskReply::Worktree(third.clone()))),
    ));
    let refresh = drain(&mut restored)
        .into_iter()
        .find(|(_, c)| matches!(c, Command::Task(Task::Worktrees { .. })))
        .unwrap()
        .0;
    let mut shared: caelum_protocol::GitWorktrees = listing("/custom/repo");
    shared.worktrees.push(third);
    restored.apply(Event::Completed(
        refresh,
        Ok(Reply::Task(TaskReply::Worktrees(Some(shared)))),
    ));
    assert_eq!(
        restored.group_path("/custom/repo/third"),
        "/scratch/external"
    );
    assert_eq!(
        restored
            .managed_worktrees("/scratch/external")
            .map(|t| t.path.as_str())
            .collect::<Vec<_>>(),
        ["/custom/repo/third"]
    );
    assert_eq!(restored.managed_worktrees("/main").count(), 2);
}

#[test]
fn switching_worktrees_restores_each_checkout_session_without_redirecting_main() {
    let mut app = Workspace::default();
    for path in ["/main", "/linked"] {
        app.projects.push(ProjectInfo {
            path: path.into(),
            name: path.into(),
        });
    }
    app.project_roots.insert("/linked".into(), "/main".into());
    let main = app.new_session(scope(Provider::Codex, "/main"));
    app.sessions[main].draft = "Main draft café".into();
    app.sessions[main].reading = 220.0;
    let other_main = app.new_session(scope(Provider::OpenCode, "/main"));
    app.select(main);
    let linked = app.new_session(scope(Provider::Codex, "/linked"));
    app.sessions[linked].draft = "Linked draft".into();
    app.sessions[linked].conversation.working = true;
    app.sessions[linked].queue.push(QueuedPrompt {
        text: "Next linked turn".into(),
        options: Default::default(),
    });
    app.attach(
        linked,
        Attachment::new("src/main.rs".into(), "snapshot".into(), None, false),
    );

    assert!(app.select_checkout("/main"));
    assert_eq!(app.selected, Some(main));
    assert_eq!(app.sessions[main].reading, 220.0);
    assert!(app.select_checkout("/linked"));
    assert_eq!(app.selected, Some(linked));
    assert!(app.sessions[linked].working());
    assert!(
        app.commands().is_empty(),
        "navigation must not send or cancel work"
    );
    app.select_project("/main");
    assert_eq!(
        app.selected,
        Some(linked),
        "the project card still recalls its active worktree"
    );
    app.preferences
        .worktree_expanded
        .insert("/main".into(), false);
    let snapshot: Snapshot =
        serde_json::from_value(serde_json::to_value(app.snapshot()).unwrap()).unwrap();
    let mut restored = Workspace::default();
    restored.enable_store("unused-checkout-restore.json".into());
    let load = restored.commands().pop().unwrap().0;
    restored.apply(Event::Completed(
        load,
        Ok(Reply::Snapshot(Some(Box::new(snapshot)))),
    ));
    assert!(restored.select_checkout("/main"));
    assert_eq!(restored.selected, Some(main));
    assert_eq!(restored.sessions[main].draft, "Main draft café");
    assert_eq!(
        restored.preferences.worktree_expanded.get("/main"),
        Some(&false)
    );
    assert!(restored.select_checkout("/linked"));
    assert_eq!(restored.sessions[linked].draft, "Linked draft");
    assert_eq!(restored.sessions[linked].queue[0].text, "Next linked turn");
    assert_eq!(restored.sessions[linked].attachments[0].content, "snapshot");
    restored.sessions[main].archived = true;
    assert!(restored.select_checkout("/main"));
    assert_eq!(restored.selected, Some(other_main));
    assert!(!restored.select_checkout("/unknown"));
    assert_eq!(restored.selected, Some(other_main));
}

#[test]
fn worktree_catalog_uses_the_latest_checkout_read_without_stealing_selection() {
    let mut app = Workspace::default();
    for path in ["/main", "/linked"] {
        app.projects.push(ProjectInfo {
            path: path.into(),
            name: path.into(),
        });
    }
    app.project_roots.insert("/linked".into(), "/main".into());
    let owner = app.new_session(scope(Provider::Codex, "/main"));
    let mut requests = Vec::new();
    for path in ["/main", "/linked"] {
        app.task(Task::Worktrees {
            project: path.into(),
            directory: String::new(),
        });
        requests.push(app.commands().pop().unwrap().0);
    }
    let listing = |source: &str, branch: &str, available: bool| {
        serde_json::from_value(json!({
        "project":{"path":source,"name":source},"storageDirectory":"/worktrees","branches":["main",branch],"defaultBase":"main",
        "worktrees":[
            {"path":"/main","project":{"path":"/main","name":"Main"},"branch":"main","main":true,"locked":null,"prunable":null,"available":true},
            {"path":"/linked","project":{"path":"/linked","name":"Linked"},"branch":branch,"main":false,"locked":null,"prunable":null,"available":available}
        ]
    })).unwrap()
    };
    app.apply(Event::Completed(
        requests[1],
        Ok(Reply::Task(TaskReply::Worktrees(Some(listing(
            "/linked",
            "new-branch",
            false,
        ))))),
    ));
    app.apply(Event::Completed(
        requests[0],
        Ok(Reply::Task(TaskReply::Worktrees(Some(listing(
            "/main",
            "old-branch",
            true,
        ))))),
    ));
    for path in ["/main", "/linked"] {
        assert_eq!(
            app.services.worktrees(path).unwrap().worktrees[1]
                .branch
                .as_deref(),
            Some("new-branch")
        );
    }
    assert!(
        !app.select_checkout("/linked"),
        "a missing checkout cannot be selected"
    );
    assert_eq!(app.selected, Some(owner));
    app.task(Task::Worktrees {
        project: "/main".into(),
        directory: String::new(),
    });
    let retry = app
        .commands()
        .into_iter()
        .find(|(_, c)| matches!(c, Command::Task(Task::Worktrees { .. })))
        .unwrap()
        .0;
    app.apply(Event::Completed(retry, Err("Git unavailable".into())));
    assert_eq!(
        app.services.worktrees("/main").unwrap().worktrees[1]
            .branch
            .as_deref(),
        Some("new-branch"),
        "a failed refresh keeps the last successful listing"
    );
    app.task(Task::Worktrees {
        project: "/main".into(),
        directory: String::new(),
    });
    let refresh = app
        .commands()
        .into_iter()
        .find(|(_, c)| matches!(c, Command::Task(Task::Worktrees { .. })))
        .unwrap()
        .0;
    let mut removed: caelum_protocol::GitWorktrees = listing("/main", "removed", true);
    removed.worktrees.retain(|tree| tree.main);
    app.apply(Event::Completed(
        refresh,
        Ok(Reply::Task(TaskReply::Worktrees(Some(removed)))),
    ));
    assert!(
        !app.select_checkout("/linked"),
        "a stale row cannot select a removed worktree"
    );
    assert_eq!(app.selected, Some(owner));
}

#[test]
fn removing_the_active_linked_checkout_returns_to_the_surviving_project() {
    let mut app = Workspace::default();
    for path in ["/main", "/linked"] {
        app.projects.push(ProjectInfo {
            name: path.into(),
            path: path.into(),
        });
    }
    app.project_roots.insert("/linked".into(), "/main".into());
    let index = app.new_session(scope(Provider::Codex, "/linked"));
    app.task(Task::RemoveWorktree {
        project: "/linked".into(),
        path: "/linked".into(),
    });
    let (id, _) = app.commands().pop().unwrap();
    app.apply(Event::Completed(
        id,
        Ok(Reply::Task(TaskReply::Worktree(
            caelum_protocol::GitWorktree {
                path: "/linked".into(),
                project: ProjectInfo {
                    name: "Linked".into(),
                    path: "/linked".into(),
                },
                label: None,
                branch: Some("feature".into()),
                main: false,
                locked: None,
                prunable: None,
                available: true,
            },
        ))),
    ));
    assert!(app.sessions[index].archived);
    assert_eq!(app.active_project.as_deref(), Some("/main"));
    assert_eq!(app.sessions[app.selected.unwrap()].scope.project, "/main");
    assert!(!app.projects.iter().any(|p| p.path == "/linked"));
    assert!(
        matches!(app.commands().as_slice(), [(_, Command::Task(Task::Worktrees { project, .. }))] if project == "/main")
    );
}

#[test]
fn removing_a_worktree_selects_existing_context_instead_of_creating_a_draft() {
    let mut app = Workspace::default();
    for path in ["/main", "/linked"] {
        app.projects.push(ProjectInfo {
            name: path.into(),
            path: path.into(),
        });
    }
    app.project_roots.insert("/linked".into(), "/main".into());
    let main = app.new_session(scope(Provider::Codex, "/main"));
    app.sessions[main].remote_id = Some("thread-main".into());
    app.sessions[main].draft = "Keep the main session context".into();
    let linked = app.new_session(scope(Provider::Codex, "/linked"));
    app.sessions[linked].remote_id = Some("thread-linked".into());
    app.select(linked);
    let list = discover_session_list(&mut app, &scope(Provider::Codex, "/linked"), false);

    app.task(Task::RemoveWorktree {
        project: "/linked".into(),
        path: "/linked".into(),
    });
    let request = app.commands().pop().unwrap().0;
    app.apply(Event::Completed(
        request,
        Ok(Reply::Task(TaskReply::Worktree(
            caelum_protocol::GitWorktree {
                path: "/linked".into(),
                project: ProjectInfo {
                    name: "Linked".into(),
                    path: "/linked".into(),
                },
                label: None,
                branch: Some("feature".into()),
                main: false,
                locked: None,
                prunable: None,
                available: true,
            },
        ))),
    ));

    assert!(app.sessions[linked].archived);
    assert_eq!(app.sessions.len(), 2);
    assert_eq!(app.active_project.as_deref(), Some("/main"));
    assert_eq!(app.selected, Some(main));
    assert_eq!(app.sessions[main].draft, "Keep the main session context");
    app.apply(Event::Completed(
        list,
        Ok(Reply::Sessions(vec![summary(
            scope(Provider::Codex, "/linked"),
            "thread-linked",
        )])),
    ));
    assert!(app.sessions[linked].archived);
}

#[test]
fn discovering_worktrees_loads_their_provider_choices() {
    let mut app = Workspace::default();
    app.projects.push(ProjectInfo {
        name: "Main".into(),
        path: "/main".into(),
    });
    app.new_session(scope(Provider::Codex, "/main"));
    app.task(Task::Worktrees {
        project: "/main".into(),
        directory: String::new(),
    });
    let listing = app
        .commands()
        .into_iter()
        .find(|(_, command)| matches!(command, Command::Task(Task::Worktrees { .. })))
        .unwrap()
        .0;

    app.apply(Event::Completed(
        listing,
        Ok(Reply::Task(TaskReply::Worktrees(Some(
            serde_json::from_value(json!({
                "project":{"path":"/main","name":"Main"},
                "storageDirectory":"/worktrees",
                "branches":["main","feature"],
                "defaultBase":"main",
                "worktrees":[
                    {"path":"/main","project":{"path":"/main","name":"Main"},"branch":"main","main":true,"locked":null,"prunable":null,"available":true},
                    {"path":"/worktrees/linked","project":{"path":"/worktrees/linked","name":"Linked"},"branch":"feature","main":false,"locked":null,"prunable":null,"available":true},
                    {"path":"/external","project":{"path":"/external","name":"External"},"branch":"external","main":false,"locked":null,"prunable":null,"available":true}
                ]
            }))
            .unwrap(),
        )))),
    ));

    let commands = app.commands();
    for provider in Provider::ALL {
        let scope = scope(provider, "/worktrees/linked");
        assert!(commands.iter().any(|(_, command)| {
            matches!(command, Command::Choices(requested) if requested == &scope)
        }));
    }
    assert!(!commands.iter().any(|(_, command)| {
        matches!(command, Command::Choices(scope) | Command::Commands(scope) | Command::List(scope)
            if scope.project == "/external")
    }));
}

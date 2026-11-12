use super::*;

#[test]
fn launcher_worktree_handoff_uses_the_chosen_provider_and_preserves_newer_navigation() {
    for navigate in [false, true] {
        let mut app = workspace();
        app.sessions[0].draft = "Keep main draft".into();
        app.launch_worktree(
            Task::CreateWorktree {
                project: "/project".into(),
                branch: "feature".into(),
                base: "main".into(),
                directory: String::new(),
            },
            Provider::OpenCode,
        );
        let (request, _) = app.commands().pop().unwrap();
        if navigate {
            app.select(1);
        }
        app.apply(Event::Completed(
            request,
            Ok(Reply::Task(TaskReply::Worktree(
                caelum_protocol::GitWorktree {
                    path: "/linked".into(),
                    project: caelum_protocol::ProjectInfo {
                        path: "/linked".into(),
                        name: "Linked".into(),
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
        let commands = app.commands();
        if navigate {
            assert!(!commands.iter().any(|(_, c)| matches!(c, Command::Open(_))));
            assert_eq!(app.selected, Some(1));
        } else {
            let (open, _) = commands
                .into_iter()
                .find(|(_, c)| matches!(c, Command::Open(_)))
                .unwrap();
            app.apply(Event::Completed(
                open,
                Ok(Reply::Project(caelum_protocol::ProjectInfo {
                    path: "/linked".into(),
                    name: "Linked".into(),
                })),
            ));
            assert_eq!(
                app.sessions[app.selected.unwrap()].scope,
                Scope {
                    project: "/linked".into(),
                    provider: Provider::OpenCode
                }
            );
        }
        assert_eq!(app.sessions[0].draft, "Keep main draft");
    }
}

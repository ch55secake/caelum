use super::*;
use serde_json::json;

fn workspace() -> Workspace {
    let mut app = Workspace::default();
    for provider in [Provider::OpenCode, Provider::Codex] {
        let i = app.new_session(Scope {
            project: "/project".into(),
            provider,
        });
        app.sessions[i].remote_id = Some(format!("session-{i}"));
        app.sessions[i].loaded = true;
    }
    app.select(0);
    app.commands();
    app
}

fn opencode(method: &str, data: serde_json::Value) -> Event {
    Event::OpenCode(caelum_protocol::OpenCodeEvent {
        project_path: "/project".into(),
        event_type: method.into(),
        data,
        id: None,
        created: None,
        location: None,
    })
}

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

#[test]
fn provider_inbox_retries_keep_identity_and_late_snapshots_cannot_resurrect_delivery() {
    let mut app = workspace();
    app.sessions[0].conversation.working = true;
    app.refresh_inbox(0);
    let (snapshot, _) = app.commands().pop().unwrap();
    assert!(app.send(0, "Queued café".into()));
    let (admission, command) = app.commands().pop().unwrap();
    let Command::Enqueue(_, id, text) = command else {
        panic!("provider queue")
    };
    assert_eq!(text, "Queued café");
    assert!(app.sessions[0].conversation.entries.is_empty());
    app.apply(Event::Completed(snapshot, Ok(Reply::Inbox(vec![]))));
    assert_eq!(app.sessions[0].inbox.items.len(), 1);
    app.apply(Event::Completed(admission, Err("lost response".into())));
    app.retry_queued(0, &id);
    let (retry, command) = app.commands().pop().unwrap();
    assert!(matches!(command,Command::Enqueue(_,ref same,_) if same == &id));
    app.apply(opencode(
        "session.inbox.delivered",
        json!({"sessionID":"session-0","inboxID":id}),
    ));
    app.apply(Event::Completed(retry, Ok(Reply::Done)));
    app.apply(opencode("session.inbox.enqueued",json!({"sessionID":"session-0","inboxID":id,"item":{"type":"user","payload":{"text":"Queued café"}}})));
    assert!(app.sessions[0].inbox.items.is_empty());
    assert_eq!(app.sessions[0].conversation.entries.len(), 1);
    app.sessions[0].conversation.history(
        Provider::OpenCode,
        &[json!({"id":id,"type":"user","text":"Queued café"})],
        false,
    );
    assert_eq!(app.sessions[0].conversation.entries.len(), 1);

    app.refresh_inbox(0);
    let (snapshot, _) = app.commands().pop().unwrap();
    app.apply(Event::Completed(
        snapshot,
        Ok(Reply::Inbox(vec![
            json!({"id":"msg_external","type":"user","payload":{"text":"Restored queue"}}),
        ])),
    ));
    app.remove_queued(0, "msg_external");
    let (remove, command) = app.commands().pop().unwrap();
    assert!(matches!(command,Command::RemoveQueued(_,ref id) if id == "msg_external"));
    app.select(1);
    app.apply(Event::Completed(
        remove,
        Err("Cancellation response lost".into()),
    ));
    app.apply(opencode("session.inbox.enqueued",json!({"sessionID":"session-0","inboxID":"msg_external","item":{"type":"user","payload":{"text":"Restored queue"}}})));
    app.retry_queued(0, "msg_external");
    let (remove, command) = app.commands().pop().unwrap();
    assert!(matches!(command, Command::RemoveQueued(_, ref id) if id == "msg_external"));
    app.apply(Event::Completed(remove, Ok(Reply::Done)));
    assert!(app.sessions[0].inbox.items.is_empty());
    assert_eq!(app.selected, Some(1));
}

#[test]
fn local_prompts_wait_for_provider_inbox_reconciliation() {
    let mut app = workspace();
    app.refresh_inbox(0);
    let (snapshot, _) = app.commands().pop().unwrap();
    assert!(app.send(0, "After the remote queue".into()));
    app.resume_queue(0);
    assert!(app.commands().is_empty());
    let image = Attachment::image("café.png".into(), b"\x89PNG\r\n\x1a\n").unwrap();
    app.apply(Event::Completed(snapshot, Ok(Reply::Inbox(vec![json!({"id":"msg_external","type":"user","payload":{"text":"First","files":[{"uri":image.content,"name":image.path}]}})]))));
    let (text, attachments) = split_context(&app.sessions[0].inbox.items[0].text);
    assert_eq!(text, "First");
    assert_eq!(attachments[0].content, image.content);
    assert_eq!(attachments[0].path, "café.png");
    assert!(app.commands().is_empty());
    app.remove_queued(0, "msg_external");
    let (remove, _) = app.commands().pop().unwrap();
    app.apply(Event::Completed(remove, Ok(Reply::Done)));
    assert!(
        matches!(app.commands().pop().unwrap().1, Command::Send(_, ref text, _) if text == "After the remote queue")
    );
}

#[test]
fn failed_local_queue_retries_its_original_options_before_later_prompts() {
    let mut app = workspace();
    app.sessions[1].conversation.working = true;
    app.sessions[1].options.model = Some("queued-model".into());
    app.send(1, "First".into());
    app.send(1, "Second".into());
    app.sessions[1].options.model = Some("newer-model".into());
    app.options_changed(1);
    app.sessions[1].conversation.working = false;
    app.resume_queue(1);
    let (request, _) = app.commands().pop().unwrap();
    app.apply(Event::Completed(request, Err("offline".into())));
    assert_eq!(app.sessions[1].queue.len(), 2);
    assert!(app.sessions[1].queue_paused);
    app.resume_queue(1);
    let (_, command) = app.commands().pop().unwrap();
    assert!(
        matches!(command,Command::Send(_,ref text,ref options) if text == "First" && options.model.as_deref() == Some("queued-model"))
    );
    assert_eq!(
        app.sessions[1].options.model.as_deref(),
        Some("newer-model")
    );
}

#[test]
fn unread_and_notifications_follow_visibility_and_do_not_replay_completed_activity() {
    let mut app = workspace();
    app.preferences.sound_volume = 0;
    app.apply(opencode(
        "session.text.delta",
        json!({"sessionID":"session-0","assistantMessageID":"answer","delta":"hello"}),
    ));
    assert!(app.acknowledge());
    assert!(!app.sessions[0].unread);
    app.conversation_hidden = true;
    let mut ended = opencode(
        "session.text.ended",
        json!({"sessionID":"session-0","assistantMessageID":"answer","text":"hello world"}),
    );
    if let Event::OpenCode(event) = &mut ended {
        event.id = Some("evt-ended".into());
    }
    app.apply(ended);
    assert!(!app.acknowledge());
    assert!(app.sessions[0].unread);
    app.apply(opencode(
        "session.execution.succeeded",
        json!({"sessionID":"session-0"}),
    ));
    let commands = app.commands();
    assert_eq!(
        commands
            .iter()
            .filter(|(_, c)| matches!(c, Command::Notify(..)))
            .count(),
        1
    );
    app.apply(opencode(
        "session.execution.succeeded",
        json!({"sessionID":"session-0"}),
    ));
    assert!(!app
        .commands()
        .iter()
        .any(|(_, c)| matches!(c, Command::Notify(..))));
    app.sessions[0].loading = false;
    app.conversation_hidden = false;
    app.sessions[0].reading = 300.0;
    assert!(!app.acknowledge());
    app.sessions[0].reading = -1.0;
    assert!(app.acknowledge());
    let mut duplicate = opencode(
        "session.text.ended",
        json!({"sessionID":"session-0","assistantMessageID":"answer","text":"hello world"}),
    );
    if let Event::OpenCode(event) = &mut duplicate {
        event.id = Some("evt-ended".into());
    }
    app.apply(duplicate);
    assert!(!app.sessions[0].unread);
    app.window_background = true;
    app.apply(opencode(
        "session.text.delta",
        json!({"sessionID":"session-0","assistantMessageID":"next","delta":"new"}),
    ));
    assert!(!app.acknowledge());
    assert!(app.snapshot().sessions[0].unread);
    let prompt = caelum_protocol::ApprovalPrompt {
        provider: "codex".into(),
        project_path: "/project".into(),
        session_id: Some("session-1".into()),
        request_id: "request".into(),
        summary: "Permission".into(),
        description: String::new(),
    };
    app.apply(Event::Approvals(vec![prompt.clone()]));
    app.apply(Event::Approvals(vec![prompt]));
    assert_eq!(
        app.commands()
            .iter()
            .filter(|(_, c)| matches!(c, Command::Notify(..)))
            .count(),
        1
    );
}
fn file() -> Reply {
    Reply::Task(TaskReply::File(caelum_protocol::ProjectFileContent {
        path: "src/café.rs".into(),
        content: "fn café() {}\n".into(),
        size_bytes: 14,
        truncated: false,
    }))
}

#[test]
fn caret_completion_preserves_unicode_suffix_and_dismissal_unlocks_the_draft() {
    let mut app = workspace();
    let draft = "Before ☁️ @ca after";
    let caret = draft.find(" after").unwrap();
    let token = Completion::at(draft, caret, caret).unwrap();
    assert!(Completion::at(draft, 0, caret).is_none());
    for cancel in [false, true] {
        app.sessions[0].draft = draft.into();
        app.complete_mention(
            0,
            token.clone(),
            "src/café.rs".into(),
            "/project".into(),
            "@src/café.rs".into(),
        );
        let (id, _) = app.commands().pop().unwrap();
        if cancel {
            app.cancel_mentions(0);
            assert_eq!(app.sessions[0].attaching, 0);
            app.sessions[0].draft = "Newer draft".into();
        }
        app.apply(Event::Completed(id, Ok(file())));
        assert_eq!(
            app.sessions[0].draft,
            if cancel {
                "Newer draft"
            } else {
                "Before ☁️ @src/café.rs after"
            }
        );
        assert_eq!(app.sessions[0].attachments.len(), 1);
    }
    app.sessions[0].draft = draft.into();
    app.complete_mention(
        0,
        token,
        "src/café.rs".into(),
        "/project".into(),
        "@src/café.rs".into(),
    );
    let (id, _) = app.commands().pop().unwrap();
    app.select(1);
    app.select(0);
    app.apply(Event::Completed(id, Ok(file())));
    assert_eq!(
        app.sessions[0].draft, draft,
        "returning to the original session does not revive a dismissed read"
    );
}

#[test]
fn side_questions_keep_main_context_and_reject_a_dismissed_generation() {
    let mut app = workspace();
    app.sessions[0].draft = "Main draft".into();
    app.attach(
        0,
        Attachment::new("main.rs".into(), "main".into(), None, false),
    );
    app.sessions[0].side_attachments.push(Attachment::new(
        "side.rs".into(),
        "side".into(),
        None,
        false,
    ));
    assert!(app.ask(0, "Why?".into()));
    let (id, command) = app.commands().pop().unwrap();
    let Command::Generate(_, prompt) = command else {
        panic!("side request")
    };
    assert!(prompt.contains("side.rs"));
    assert!(!prompt.contains("main.rs"));
    app.dismiss_side_question(0);
    assert!(app.ask(0, "New question".into()));
    let (new, _) = app.commands().pop().unwrap();
    app.apply(Event::Completed(
        id,
        Ok(Reply::Generated("Old answer".into())),
    ));
    assert!(app.sessions[0].generating);
    app.apply(Event::Completed(new, Err("offline".into())));
    assert_eq!(app.sessions[0].side_error.as_deref(), Some("offline"));
    assert_eq!(app.sessions[0].draft, "Main draft");
    assert_eq!(app.sessions[0].attachments.len(), 1);
}

#[test]
fn provider_controls_run_during_execution_without_clearing_stop_on_failure() {
    let mut app = workspace();
    app.provider_commands.insert(
        app.sessions[0].scope.clone(),
        vec![caelum_protocol::OpenCodeCommandInfo {
            name: "goal".into(),
            description: None,
        }],
    );
    app.sessions[0].conversation.working = true;
    assert!(app.send(0, "/goal pause".into()));
    assert!(app.sessions[0].queue.is_empty());
    let (id, command) = app.commands().pop().unwrap();
    assert!(matches!(command, Command::Send(..)));
    assert!(!app.send(0, "/goal pause".into()));
    app.apply(Event::Completed(id, Err("Rejected control".into())));
    assert!(app.sessions[0].working());
    assert!(!app.sessions[0].commanding);
    assert_eq!(
        app.sessions[0].failed_prompt.as_deref(),
        Some("/goal pause")
    );
    app.cancel(0);
    assert!(matches!(
        app.commands().pop().unwrap().1,
        Command::Cancel(_)
    ));
}

#[test]
fn context_serialization_keeps_file_snapshots_images_and_command_arguments_separate() {
    let image = Attachment::image("screenshot.png".into(), b"\x89PNG\r\n\x1a\n").unwrap();
    let file = Attachment::new("café.rs".into(), "fn café() {}".into(), Some((4, 4)), false);
    let prompt = super::composer::with_context("/review please", &[file.clone(), image.clone()]);
    assert_eq!(
        split_context(&prompt),
        ("/review please", vec![file, image.clone()])
    );
    let files = super::composer::command_files(&prompt);
    assert_eq!(files.len(), 2);
    assert_eq!(files[1].uri, image.content);
    assert!(files[0].description.contains("Lines 4–4"));
    assert_eq!(super::composer::command_text(&prompt), "/review please");
    let (text, images) = super::provider_prompt(&prompt);
    assert!(!text.contains("caelum-image-context"));
    assert!(text.contains("fn café"));
    assert_eq!(images, vec![image]);
}

#[test]
fn rendered_media_hides_reminders_but_preserves_literal_code_and_unicode() {
    let input="Keep café  \n<system-reminder>secret ![hidden](secret.png)</system-reminder>\n![visible](image.png)\n```md\n![literal](code.png)\n<system-reminder>literal</system-reminder>\n```\n`![inline](inline.png)`\n";
    let shown = presentation::visible_text(input);
    assert!(shown.starts_with("Keep café  \n"));
    assert!(!shown.contains("secret"));
    assert!(shown.contains("![literal](code.png)"));
    assert!(shown.contains("<system-reminder>literal</system-reminder>"));
    assert!(shown.contains("`![inline](inline.png)`"));
    let media = presentation::markdown_media(input);
    assert_eq!(media.len(), 1);
    assert_eq!(media[0].source, "image.png");
    assert_eq!(
        presentation::visible_text("Answer<system-reminder>unfinished"),
        "Answer"
    );
}

#[test]
fn tool_timing_is_frozen_when_complete_and_usage_remains_attributed_to_its_model() {
    let mut c = Conversation::default();
    c.apply(Provider::Codex,"item/started",&json!({"_created":1000,"item":{"id":"run","type":"commandExecution","command":"cargo test"}}));
    c.apply(Provider::Codex,"item/completed",&json!({"_created":3500,"item":{"id":"run","type":"commandExecution","status":"completed","exitCode":1}}));
    let tool = presentation::Tool::from_entry(&c.entries[0], 999999);
    assert_eq!(tool.timing, "2.5s");
    assert_eq!(tool.outcome, presentation::Outcome::Failure);
    c.apply(Provider::OpenCode,"session.step.started",&json!({"assistantMessageID":"answer","_created":100,"model":{"id":"model","providerID":"vendor"}}));
    c.apply(Provider::OpenCode,"session.step.ended",&json!({"assistantMessageID":"answer","_created":200,"tokens":{"input":100,"output":10,"cache":{"read":40,"write":5}}}));
    assert_eq!(c.used_tokens, Some(155));
    assert_eq!(c.usage_model.as_deref(), Some("vendor/model"));
    c.history(Provider::OpenCode,&[json!({"id":"old","type":"assistant","time":{"created":1},"tokens":{"input":1,"output":1,"cache":{"read":0,"write":0}}})],true);
    assert_eq!(c.used_tokens, Some(155));
}

#[test]
fn refreshing_statistics_preserves_the_same_filter_cache_and_rejects_old_ranges() {
    let mut app = workspace();
    let task = Task::Stats {
        project: "/project".into(),
        filter: caelum_protocol::OpenCodeStatsFilter {
            from: Some(100),
            ..Default::default()
        },
    };
    app.task(task.clone());
    let (old, _) = app.commands().pop().unwrap();
    let other = Task::Stats {
        project: "/project".into(),
        filter: caelum_protocol::OpenCodeStatsFilter {
            from: Some(200),
            ..Default::default()
        },
    };
    app.task(other.clone());
    let (new, _) = app.commands().pop().unwrap();
    app.apply(Event::Completed(old, Err("old failure".into())));
    assert!(app.services.busy(&other));
    assert!(app.services.error(&other).is_none());
    app.apply(Event::Completed(new, Err("current failure".into())));
    assert_eq!(app.services.error(&other), Some("current failure"));
}

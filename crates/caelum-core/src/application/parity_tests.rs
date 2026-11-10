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
fn review_summaries_distinguish_unknown_checks_and_unmergeable_states() {
    use super::presentation::{CheckSummary, MergeReadiness, Outcome};
    let checks: Vec<caelum_protocol::GitHubStatusCheck> = serde_json::from_value(json!([
        {"__typename":"StatusContext", "context":"CI", "state":"SUCCESS"},
        {"__typename":"CheckRun", "name":"Optional", "status":"COMPLETED", "conclusion":"SKIPPED"}
    ]))
    .unwrap();
    assert_eq!(CheckSummary::new(None).outcome, Outcome::Running);
    assert_eq!(CheckSummary::new(Some(&[])).outcome, Outcome::Neutral);
    assert_eq!(
        CheckSummary::new(Some(&checks[..1])).outcome,
        Outcome::Success
    );
    assert_ne!(CheckSummary::new(Some(&checks)).outcome, Outcome::Success);
    let mut details: caelum_protocol::GitHubPullRequestDetails = serde_json::from_value(json!({"number":1,"title":"Review","url":"https://github.com/example/repo/pull/1","isDraft":false,"body":"","state":"OPEN","author":null,"baseRefName":"main","headRefName":"feature","headRefOid":"head","mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","additions":1,"deletions":0,"changedFiles":1,"reviewDecision":null,"statusCheckRollup":[]})).unwrap();
    assert!(MergeReadiness::new(&details).can_merge);
    for state in ["BLOCKED", "DIRTY", "UNKNOWN", "FUTURE_STATE"] {
        details.merge_state_status = state.into();
        assert!(!MergeReadiness::new(&details).can_merge, "{state}");
    }
    details.merge_state_status = "BEHIND".into();
    assert!(MergeReadiness::new(&details).behind);
    details.state = "MERGED".into();
    assert!(!MergeReadiness::new(&details).behind);
    assert!(!MergeReadiness::new(&details).can_merge);
    details.state = "OPEN".into();
    details.merge_state_status = "CLEAN".into();
    details.mergeable = "UNKNOWN".into();
    assert!(!MergeReadiness::new(&details).can_merge);
    details.mergeable = "MERGEABLE".into();
    details.pull_request.is_draft = true;
    assert!(!MergeReadiness::new(&details).can_merge);
}

#[test]
fn archive_notice_expires_only_after_unengaged_time_and_recovers_after_undo_failure() {
    let mut app = workspace();
    app.archive(0, true);
    let request = app.commands().pop().unwrap().0;
    assert_eq!(app.last_archived, None, "notice waits for provider success");
    app.apply(Event::Completed(request, Ok(Reply::Done)));
    let now = presentation::now();
    app.tick_archive_notice(now + 4_000, false);
    app.tick_archive_notice(now + 24_000, true);
    assert_eq!(app.last_archived, Some(0));
    app.undo_archive();
    let undo = app
        .commands()
        .into_iter()
        .find(|(_, c)| matches!(c, Command::Archive(_, false)))
        .unwrap()
        .0;
    app.tick_archive_notice(now + 60_000, false);
    assert_eq!(
        app.last_archived,
        Some(0),
        "restoration keeps its notice alive"
    );
    app.apply(Event::Completed(undo, Err("Offline, try again".into())));
    let failed = presentation::now();
    app.tick_archive_notice(failed + 7_000, false);
    assert_eq!(app.last_archived, Some(0));
    assert!(app.tick_archive_notice(failed + 8_001, false));
    assert_eq!(app.last_archived, None);
    assert!(
        app.sessions[0].archived,
        "expiration does not restore the session"
    );
}

#[test]
fn late_archive_and_restore_callbacks_cannot_replace_a_newer_notice() {
    let mut app = workspace();
    app.archive(0, true);
    let first = app.commands().pop().unwrap().0;
    app.archive(1, true);
    let second = app.commands().pop().unwrap().0;
    app.apply(Event::Completed(second, Ok(Reply::Done)));
    app.apply(Event::Completed(first, Ok(Reply::Done)));
    assert_eq!(app.last_archived, Some(1));
    app.undo_archive();
    let undo = app
        .commands()
        .into_iter()
        .find(|(_, c)| matches!(c, Command::Archive(_, false)))
        .unwrap()
        .0;
    let draft = app.selected.unwrap();
    app.archive(draft, true);
    assert_eq!(app.last_archived, Some(draft));
    app.apply(Event::Completed(undo, Ok(Reply::Done)));
    assert_eq!(app.last_archived, Some(draft));
    app.dismiss_archive(1);
    assert_eq!(app.last_archived, Some(draft));
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
fn attachment_reads_and_failures_follow_the_request_owner_and_retry() {
    let mut app = workspace();
    app.sessions[0].draft = "Keep this draft".into();
    app.sessions[0].error = Some("Unrelated provider error".into());
    app.attach_file(0, "src/café.rs".into());
    app.attach_image(0, "Pasted image.png".into(), vec![1, 2]);
    let reads = app.attachment_reads(0);
    assert_eq!(
        reads.iter().map(|(_, name)| *name).collect::<Vec<_>>(),
        ["src/café.rs", "Pasted image.png"]
    );
    let ids: Vec<_> = reads.iter().map(|(id, _)| *id).collect();
    assert_eq!(app.sessions[0].attaching, 2);
    assert!(!app.sessions[0].can_submit());
    app.commands();
    app.select(1);
    app.apply(Event::Completed(ids[1], Err("Unsupported image".into())));
    app.apply(Event::Completed(ids[0], Ok(file())));
    assert!(app.attachment_reads(0).is_empty());
    assert!(app.attachment_reads(1).is_empty());
    assert_eq!(app.sessions[0].attaching, 0);
    assert!(app.sessions[0].can_submit());
    assert_eq!(app.sessions[0].attachments[0].content, "fn café() {}\n");
    assert_eq!(
        app.sessions[0].attachment_errors,
        [("Pasted image.png".into(), "Unsupported image".into())]
    );
    assert!(app.sessions[1].attachment_errors.is_empty());
    assert!(app.sessions[1].attachments.is_empty());
    assert_eq!(app.sessions[0].draft, "Keep this draft");
    assert_eq!(
        app.sessions[0].error.as_deref(),
        Some("Unrelated provider error")
    );

    app.attach_image(0, "Pasted image.png".into(), vec![3]);
    assert!(app.sessions[0].attachment_errors.is_empty());
    let id = app.attachment_reads(0)[0].0;
    app.apply(Event::Completed(
        id,
        Ok(Reply::Task(TaskReply::Image(
            "data:image/png;base64,AQ==".into(),
        ))),
    ));
    assert_eq!(app.sessions[0].attachments.len(), 2);
    assert_eq!(app.sessions[0].attaching, 0);
    assert!(app.attachment_reads(0).is_empty());

    app.attach_file(0, "wrong-response.txt".into());
    let id = app.attachment_reads(0)[0].0;
    app.apply(Event::Completed(id, Ok(Reply::Done)));
    assert_eq!(app.sessions[0].attaching, 0);
    assert!(app.attachment_reads(0).is_empty());
    assert_eq!(
        app.sessions[0].attachment_errors[0].1,
        "Unexpected attachment response"
    );
}

#[test]
fn mention_status_clears_on_cancel_and_stale_failure_does_not_touch_new_context() {
    for stale in [false, true] {
        let mut app = workspace();
        app.sessions[0].draft = "Review @ca".into();
        let token = Completion::at(&app.sessions[0].draft, 10, 10).unwrap();
        app.complete_mention(
            0,
            token,
            "src/café.rs".into(),
            "/project".into(),
            "@src/café.rs".into(),
        );
        let id = app.attachment_reads(0)[0].0;
        app.commands();
        if stale {
            app.cancel_mentions(0);
            assert!(app.attachment_reads(0).is_empty());
            app.sessions[0].draft = "New draft".into();
        }
        app.apply(Event::Completed(id, Err("File was removed".into())));
        assert!(app.attachment_reads(0).is_empty());
        assert_eq!(app.sessions[0].attaching, 0);
        assert_eq!(app.sessions[0].attachment_errors.is_empty(), stale);
        assert_eq!(
            app.sessions[0].draft,
            if stale { "New draft" } else { "Review @ca" }
        );
        assert!(app.sessions[0].error.is_none());
    }
    let mut app = workspace();
    app.sessions[0].side_draft = "@ca".into();
    app.complete_side_mention(
        0,
        Completion::at("@ca", 3, 3).unwrap(),
        "src/café.rs".into(),
        "/project".into(),
        "@src/café.rs".into(),
    );
    assert!(app.attachment_reads(0).is_empty());
    let id = app.commands().pop().unwrap().0;
    app.apply(Event::Completed(id, Err("Side context failure".into())));
    assert!(app.sessions[0].attachment_errors.is_empty());
    assert_eq!(
        app.sessions[0].side_error.as_deref(),
        Some("Side context failure")
    );
}

#[test]
fn attachment_limit_reports_paste_failure_without_losing_captured_context() {
    let mut app = workspace();
    for i in 0..16 {
        assert!(app.attach(
            0,
            Attachment::new(format!("{i}.rs"), "captured".into(), None, false)
        ));
    }
    app.attach_image(0, "Pasted image.png".into(), vec![1]);
    assert_eq!(app.sessions[0].attachments.len(), 16);
    assert!(app.attachment_reads(0).is_empty());
    assert_eq!(app.sessions[0].attaching, 0);
    assert_eq!(app.sessions[0].attachment_errors[0].0, "Pasted image.png");
    assert!(app.attach(
        0,
        Attachment::new("0.rs".into(), "replacement".into(), None, false)
    ));
    assert_eq!(app.sessions[0].attachments.len(), 16);
    assert_eq!(
        app.sessions[0].attachments.last().unwrap().content,
        "replacement"
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
fn reminder_fragments_never_enter_presented_rows_or_copied_text() {
    let mut c = Conversation::default();
    for ch in "<system-reminder>secret ![private](private.png)</system-reminder>".chars() {
        c.apply(
            Provider::Codex,
            "item/agentMessage/delta",
            &json!({"itemId":"answer","delta":ch.to_string()}),
        );
        let entry = &c.entries[0];
        assert_eq!(presentation::entry_text(entry), "");
        assert!(!presentation::entry_visible(entry));
        assert!(presentation::markdown_media(&entry.text).is_empty());
    }
    c.apply(
        Provider::Codex,
        "item/agentMessage/delta",
        &json!({"itemId":"answer","delta":"Visible café"}),
    );
    assert_eq!(presentation::entry_text(&c.entries[0]), "Visible café");
    assert!(
        c.entries[0].text.contains("secret"),
        "presentation must retain provider records"
    );
    c.history(Provider::OpenCode, &[
        json!({"id":"synthetic","type":"synthetic","text":"<system-reminder>secret status</system-reminder>"}),
        json!({"id":"user","type":"user","text":"```xml\n<system-reminder>literal</system-reminder>\n```\n    <system-reminder>indented code</system-reminder>\n`<system-reminder>inline</system-reminder>`"}),
    ], false);
    let literal = c
        .entries
        .iter()
        .find(|e| e.kind == EntryKind::User)
        .unwrap();
    assert!(presentation::entry_text(literal).contains("indented code"));
    assert!(presentation::entry_text(literal).contains("literal"));
    assert!(presentation::entry_text(literal).contains("inline"));
    assert!(c
        .entries
        .iter()
        .filter(|e| e.kind == EntryKind::Status)
        .all(|e| !presentation::entry_visible(e)));
}

#[test]
fn session_activity_survives_gaps_and_obeys_waiting_retry_and_terminal_states() {
    use presentation::{activity, ActivityKind as A};
    let mut app = workspace();
    assert!(app.send(1, "Work on this".into()));
    assert_eq!(activity(&app.sessions[1], false).unwrap().kind, A::Working);
    app.sessions[1].submitting = false;
    app.sessions[1].conversation.apply(
        Provider::Codex,
        "item/reasoning/summaryTextDelta",
        &json!({"itemId":"thought","delta":"Think"}),
    );
    assert_eq!(activity(&app.sessions[1], false).unwrap().kind, A::Thinking);
    app.sessions[1].conversation.apply(
        Provider::Codex,
        "item/completed",
        &json!({"item":{"id":"thought","type":"reasoning","summary":[{"text":"Think"}]}}),
    );
    assert_eq!(activity(&app.sessions[1], false).unwrap().kind, A::Working);
    app.sessions[1]
        .conversation
        .apply(Provider::OpenCode, "session.retry.scheduled", &json!({}));
    assert_eq!(activity(&app.sessions[1], false).unwrap().kind, A::Retrying);
    app.sessions[1].conversation.apply(
        Provider::Codex,
        "item/started",
        &json!({"item":{"id":"tool","type":"commandExecution","command":"cargo test"}}),
    );
    assert_eq!(activity(&app.sessions[1], false).unwrap().kind, A::Tool);
    assert_eq!(activity(&app.sessions[1], true).unwrap().kind, A::Input);
    app.cancel(1);
    assert_eq!(activity(&app.sessions[1], false).unwrap().kind, A::Stopping);
    app.sessions[1].stopping = false;
    app.sessions[1].conversation.settle();
    assert!(activity(&app.sessions[1], false).is_none());
    app.sessions[1].shells.push(
        serde_json::from_value(json!({"id":"shell","command":"cargo test","startedAt":1})).unwrap(),
    );
    assert_eq!(
        activity(&app.sessions[1], false).unwrap().kind,
        A::Background
    );
    app.sessions[1].disconnected = true;
    assert!(activity(&app.sessions[1], false).is_none());
    assert!(activity(&app.sessions[0], false).is_none());
}

#[test]
fn addressing_review_feedback_submits_to_the_target_and_preserves_its_draft() {
    let mut app = workspace();
    let url = "https://github.com/example/project/pull/1";
    let r = app
        .services
        .reviews
        .entry(("/project".into(), url.into()))
        .or_default();
    r.details = Some(serde_json::from_value(json!({"number":1,"title":"Review","url":url,"isDraft":false,"body":"","state":"OPEN","author":null,"baseRefName":"main","headRefName":"feature","headRefOid":"abc","mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","additions":1,"deletions":0,"changedFiles":1,"reviewDecision":null,"statusCheckRollup":[]})).unwrap());
    r.discussion.push(serde_json::from_value(json!({"id":"comment","body":"Preserve the newer draft.","url":url,"author":null,"createdAt":"2026-11-07","state":null,"diffHunk":null})).unwrap());
    r.selected_comments.insert("comment".into());
    app.sessions[1].draft = "Independent draft".into();
    app.sessions[1].attachments.push(Attachment::new(
        "draft.rs".into(),
        "not feedback".into(),
        None,
        false,
    ));
    assert!(!app.address_review(1, "/other", url));
    assert!(app.address_review(1, "/project", url));
    let requests = app.commands();
    assert!(requests.iter().any(|(_, c)| matches!(c, Command::Send(key, prompt, _) if key.id == "session-1" && prompt.contains("Preserve the newer draft.") && !prompt.contains("not feedback"))));
    assert_eq!(app.selected, Some(0));
    assert_eq!(app.sessions[1].draft, "Independent draft");
    assert_eq!(app.sessions[1].attachments.len(), 1);
    assert!(!app.address_review(1, "/project", url));
}

#[test]
fn compaction_without_a_summary_stays_visible_for_both_providers() {
    use presentation::{entry_visible, outcome, Outcome};
    let mut codex = Conversation::default();
    codex.apply(
        Provider::Codex,
        "item/started",
        &json!({"item":{"id":"compact","type":"contextCompaction"},"_created":100}),
    );
    assert!(entry_visible(&codex.entries[0]));
    assert!(!codex.entries[0].complete);
    codex.apply(
        Provider::Codex,
        "item/completed",
        &json!({"item":{"id":"compact","type":"contextCompaction"},"_created":200}),
    );
    codex.history(
        Provider::Codex,
        &[json!({"status":"completed","items":[{"id":"compact","type":"contextCompaction"}]})],
        false,
    );
    assert_eq!(codex.entries.len(), 1);
    assert!(entry_visible(&codex.entries[0]));
    assert_eq!(
        outcome(&codex.entries[0].status, codex.entries[0].complete),
        Outcome::Success
    );

    let mut native = Conversation::default();
    native.used_tokens = Some(1000);
    native.apply(
        Provider::OpenCode,
        "session.compaction.started",
        &json!({"inputID":"compact","_created":100}),
    );
    assert!(entry_visible(&native.entries[0]));
    native.apply(
        Provider::OpenCode,
        "session.compaction.ended",
        &json!({"inputID":"compact","_created":200}),
    );
    native.history(Provider::OpenCode, &[json!({"id":"compact","type":"compaction","status":"completed","summary":"","time":{"created":100}})], false);
    assert_eq!(native.entries.len(), 1);
    assert!(entry_visible(&native.entries[0]));
    assert_eq!(
        outcome(&native.entries[0].status, native.entries[0].complete),
        Outcome::Success
    );
    assert_eq!(native.used_tokens, None);
}

#[test]
fn compaction_failures_and_summaries_survive_refresh_and_late_events() {
    use presentation::{entry_visible, Outcome, Tool};
    let mut c = Conversation::default();
    c.apply(
        Provider::OpenCode,
        "session.compaction.started",
        &json!({"inputID":"first","_created":100}),
    );
    c.apply(
        Provider::OpenCode,
        "session.compaction.delta",
        &json!({"inputID":"first","text":"## Context\n","_created":110}),
    );
    c.apply(
        Provider::OpenCode,
        "session.compaction.delta",
        &json!({"inputID":"first","text":"Preserve drafts.","_created":120}),
    );
    c.apply(Provider::OpenCode, "session.compaction.failed", &json!({"inputID":"first","error":{"message":"Summary service unavailable"},"_created":130}));
    let revision = c.activity;
    c.history(
        Provider::OpenCode,
        &[json!({"id":"first","type":"compaction","status":"failed","time":{"created":100}})],
        false,
    );
    assert_eq!(c.entries.len(), 1);
    assert!(entry_visible(&c.entries[0]));
    assert_eq!(c.entries[0].text, "## Context\nPreserve drafts.");
    assert_eq!(c.entries[0].error, "Summary service unavailable");
    assert_eq!(
        Tool::from_entry(&c.entries[0], 1000).outcome,
        Outcome::Failure
    );
    c.apply(
        Provider::OpenCode,
        "session.compaction.started",
        &json!({"inputID":"first","_created":100}),
    );
    c.apply(
        Provider::OpenCode,
        "session.compaction.delta",
        &json!({"inputID":"first","text":"stale","_created":140}),
    );
    assert_eq!(c.activity, revision);
    c.apply(
        Provider::OpenCode,
        "session.compaction.started",
        &json!({"inputID":"second","_created":200}),
    );
    c.apply(
        Provider::OpenCode,
        "session.compaction.failed",
        &json!({"inputID":"first","error":"late failure","_created":210}),
    );
    c.apply(
        Provider::OpenCode,
        "session.compaction.delta",
        &json!({"text":"stale","_created":150}),
    );
    assert!(!c.entries[1].complete);
    assert!(c.entries[1].text.is_empty());
    c.apply(
        Provider::OpenCode,
        "session.compaction.delta",
        &json!({"inputID":"second","text":"Fresh summary","_created":220}),
    );
    c.apply(
        Provider::OpenCode,
        "session.compaction.ended",
        &json!({"inputID":"second","_created":230}),
    );
    c.history(Provider::OpenCode, &[json!({"id":"second","type":"compaction","status":"completed","summary":"Fresh summary","time":{"created":200}})], false);
    assert_eq!(c.entries.len(), 2);
    assert_eq!(c.entries[1].text, "Fresh summary");
    assert_eq!(
        Tool::from_entry(&c.entries[1], 1000).outcome,
        Outcome::Success
    );
}

#[test]
fn compaction_interruption_does_not_claim_success() {
    use presentation::{Outcome, Tool};
    let mut c = Conversation::default();
    c.apply(
        Provider::OpenCode,
        "session.compaction.started",
        &json!({"inputID":"compact","_created":100}),
    );
    c.settle();
    assert_eq!(
        Tool::from_entry(&c.entries[0], 1000).outcome,
        Outcome::Interrupted
    );
    c.history(
        Provider::OpenCode,
        &[json!({"id":"compact","type":"compaction","status":"running","time":{"created":100}})],
        false,
    );
    assert_eq!(
        Tool::from_entry(&c.entries[0], 1000).outcome,
        Outcome::Interrupted
    );
    let mut restored = Conversation::default();
    restored.history(
        Provider::OpenCode,
        &[json!({"id":"compact","type":"compaction","status":"cancelled"})],
        false,
    );
    assert_eq!(
        Tool::from_entry(&restored.entries[0], 1000).outcome,
        Outcome::Interrupted
    );
    restored.history(Provider::OpenCode, &[json!({"id":"failed","type":"compaction","status":"failed","error":{"message":"Could not compact context"}})], false);
    let failed = Tool::from_entry(&restored.entries[1], 1000);
    assert_eq!(failed.outcome, Outcome::Failure);
    assert_eq!(failed.error, "Could not compact context");
    for status in ["running", "pending", "in_progress"] {
        let mut restored = Conversation::default();
        restored.history(
            Provider::OpenCode,
            &[json!({"id":"active","type":"compaction","status":status})],
            false,
        );
        assert_eq!(
            Tool::from_entry(&restored.entries[0], 1000).outcome,
            Outcome::Running
        );
        restored.settle();
        assert_eq!(
            Tool::from_entry(&restored.entries[0], 1000).outcome,
            Outcome::Interrupted
        );
    }
}

#[test]
fn command_success_omits_exit_zero_but_failure_keeps_its_code() {
    use presentation::{Outcome, Tool};
    for metadata_key in ["exit", "exitCode"] {
        for code in [0, 101] {
            let mut c = Conversation::default();
            c.history(Provider::OpenCode, &[json!({"id":"message","type":"assistant","content":[{"id":"run","type":"tool","name":"bash","state":{"status":"completed","input":{"command":"cargo test","timeout":120000},"metadata":{metadata_key:code},"content":[{"type":"text","text":"test output"}]}}]})], false);
            let tool = Tool::from_entry(&c.entries[0], 0);
            assert_eq!(tool.summary, "cargo test");
            assert_eq!(tool.output, "test output");
            assert_eq!(
                tool.outcome,
                if code == 0 {
                    Outcome::Success
                } else {
                    Outcome::Failure
                }
            );
            assert_eq!(
                tool.details,
                if code == 0 { vec![] } else { vec!["Exit 101"] }
            );
        }
    }
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
fn shell_tool_feed_hides_internal_ids_but_keeps_command_and_output() {
    let id = "sh_internal_fixture";
    let markup = format!(r#"<shell id="{id}" state="completed" command="cargo test">done</shell>"#);
    let mut conversation = Conversation::default();
    conversation.history(
        Provider::OpenCode,
        &[json!({
            "id":"message",
            "type":"assistant",
            "content":[{
                "id":"tool",
                "type":"tool",
                "name":"shell",
                "state":{
                    "status":"completed",
                    "input":markup.clone(),
                    "content":[{"type":"text","text":markup}]
                }
            }]
        })],
        false,
    );
    let tool = presentation::Tool::from_entry(&conversation.entries[0], 0);
    assert!(!tool.summary.contains(id));
    assert!(!tool.input.contains(id));
    assert!(!tool.output.contains(id));
    assert!(tool.summary.contains("cargo test"));
    assert!(tool.output.contains("state=\"completed\""));
    assert!(tool.output.contains("done"));
}

#[test]
fn tool_summary_omits_unrecognized_json_arguments() {
    let mut conversation = Conversation::default();
    conversation.history(
        Provider::OpenCode,
        &[json!({
            "id":"message",
            "type":"assistant",
            "content":[{
                "id":"tool",
                "type":"tool",
                "name":"custom_tool",
                "state":{
                    "status":"completed",
                    "input":{"options":{"enabled":true}},
                    "content":[]
                }
            }]
        })],
        false,
    );
    let tool = presentation::Tool::from_entry(&conversation.entries[0], 0);
    assert!(tool.summary.is_empty());
    assert!(tool.input.contains("enabled"));
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

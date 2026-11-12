use super::*;

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
    let prompt = crate::application::composer::with_context(
        "/review please",
        &[file.clone(), image.clone()],
    );
    assert_eq!(
        split_context(&prompt),
        ("/review please", vec![file, image.clone()])
    );
    let files = crate::application::composer::command_files(&prompt);
    assert_eq!(files.len(), 2);
    assert_eq!(files[1].uri, image.content);
    assert!(files[0].description.contains("Lines 4–4"));
    assert_eq!(
        crate::application::composer::command_text(&prompt),
        "/review please"
    );
    let (text, images) = crate::application::provider_prompt(&prompt);
    assert!(!text.contains("caelum-image-context"));
    assert!(text.contains("fn café"));
    assert_eq!(images, vec![image]);
}

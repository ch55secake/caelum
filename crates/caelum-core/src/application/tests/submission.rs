use super::*;

#[test]
fn queues_wait_for_reconciled_idle_and_stop_requires_explicit_resume() {
    let mut app = Workspace::default();
    let sc = scope(Provider::Codex, "/a");
    let index = started(&mut app, sc.clone(), "thread");
    app.send(index, "queued".into());
    assert!(app.commands().is_empty());
    assert_eq!(app.sessions[index].queue.len(), 1);
    app.cancel(index);
    let (cancel, _) = app.commands().pop().unwrap();
    app.apply(Event::Completed(cancel, Ok(Reply::Done)));
    let (resume, _) = app.commands().pop().unwrap();
    app.apply(Event::Completed(
        resume,
        Ok(Reply::Session(summary(sc, "thread"))),
    ));
    let (history, _) = app.commands().pop().unwrap();
    app.apply(Event::Completed(history, Ok(page(vec![], false))));
    assert!(app.commands().is_empty());
    assert_eq!(app.sessions[index].queue.len(), 1);
    app.resume_queue(index);
    assert!(
        matches!(app.commands().as_slice(), [(_, Command::Send(_, prompt, _))] if prompt == "queued")
    );
}

#[test]
fn queued_settings_and_edits_during_creation_keep_their_own_options() {
    let mut app = Workspace::default();
    let sc = scope(Provider::Codex, "/a");
    let index = app.new_session(sc.clone());
    app.sessions[index].options.model = Some("first".into());
    app.send(index, "first prompt".into());
    let (start, _) = app.commands().pop().unwrap();
    app.sessions[index].options.model = Some("second".into());
    app.options_changed(index);
    app.send(index, "queued prompt".into());
    app.sessions[index].options.model = Some("third".into());
    app.options_changed(index);
    app.apply(Event::Completed(
        start,
        Ok(Reply::Session(summary(sc.clone(), "s"))),
    ));
    let (send, command) = app.commands().pop().unwrap();
    assert!(
        matches!(command, Command::Send(_, _, options) if options.model.as_deref() == Some("first"))
    );
    app.apply(Event::Completed(send, Ok(Reply::Done)));
    app.apply(codex(
        "/a",
        "s",
        "turn/completed",
        json!({"turn":{"status":"completed"}}),
    ));
    let (resume, _) = app.commands().pop().unwrap();
    let mut old = summary(sc, "s");
    old.options.model = Some("first".into());
    app.apply(Event::Completed(resume, Ok(Reply::Session(old))));
    let (history, _) = app.commands().pop().unwrap();
    app.apply(Event::Completed(history, Ok(page(vec![], false))));
    assert!(
        matches!(app.commands().as_slice(), [(_, Command::Send(_, text, options))] if text == "queued prompt" && options.model.as_deref() == Some("second"))
    );
    assert_eq!(app.sessions[index].options.model.as_deref(), Some("third"));
}

#[test]
fn delayed_creation_and_send_never_steal_selection_or_cross_scopes() {
    let mut workspace = Workspace::default();
    let a = scope(Provider::Codex, "/a");
    let first = workspace.new_session(a.clone());
    workspace.send(first, "first prompt".into());
    let (create, _) = workspace.commands().pop().unwrap();
    let second = workspace.new_session(scope(Provider::OpenCode, "/b"));
    workspace.apply(Event::Completed(
        create,
        Ok(Reply::Session(summary(a, "same-id"))),
    ));
    assert_eq!(workspace.selected, Some(second));
    let (send, command) = workspace.commands().pop().unwrap();
    assert!(matches!(command, Command::Send(key, _, _) if key.scope.project == "/a"));
    workspace.apply(codex(
        "/b",
        "same-id",
        "item/agentMessage/delta",
        json!({"itemId":"answer", "delta":"wrong"}),
    ));
    workspace.apply(Event::OpenCode(OpenCodeEvent {
        project_path: "/a".into(),
        event_type: "session.text.delta".into(),
        data: json!({"sessionID":"same-id","assistantMessageID":"answer","delta":"wrong provider"}),
        id: None,
        created: None,
        location: None,
    }));
    workspace.apply(codex(
        "/a",
        "same-id",
        "item/agentMessage/delta",
        json!({"itemId":"answer", "delta":"right"}),
    ));
    workspace.apply(codex(
        "/a",
        "same-id",
        "turn/completed",
        json!({"turn":{"id":"turn","status":"completed"}}),
    ));
    workspace.apply(Event::Completed(send, Ok(Reply::Done)));
    assert!(
        !workspace.sessions[first].working(),
        "late admission cannot resurrect a completed turn"
    );
    assert_eq!(
        workspace.sessions[first].conversation.entries[1].text,
        "right"
    );
    assert!(workspace.sessions[second].conversation.entries.is_empty());
    assert_eq!(workspace.selected, Some(second));
}

#[test]
fn cancelling_during_creation_keeps_the_prompt_and_never_submits_it() {
    let mut workspace = Workspace::default();
    let scope = scope(Provider::Codex, "/a");
    let index = workspace.new_session(scope.clone());
    workspace.send(index, "keep me".into());
    let (request, _) = workspace.commands().pop().unwrap();
    workspace.cancel(index);
    workspace.apply(Event::Completed(
        request,
        Ok(Reply::Session(summary(scope, "new"))),
    ));
    assert!(workspace.commands().is_empty());
    assert!(!workspace.sessions[index].working());
    assert_eq!(
        workspace.sessions[index].failed_prompt.as_deref(),
        Some("keep me")
    );
    assert!(workspace.sessions[index].conversation.entries.is_empty());
}

#[test]
fn cancellation_waits_for_admission_and_failures_remain_retryable() {
    let mut workspace = Workspace::default();
    let origin = scope(Provider::OpenCode, "/a");
    let index = workspace.new_session(origin.clone());
    workspace.send(index, "recover me".into());
    let (create, _) = workspace.commands().pop().unwrap();
    workspace.apply(Event::Completed(
        create,
        Ok(Reply::Session(summary(origin, "ses_a"))),
    ));
    let (send, _) = workspace.commands().pop().unwrap();
    workspace.cancel(index);
    assert!(workspace.commands().is_empty());
    workspace.apply(Event::Completed(send, Ok(Reply::Done)));
    let (cancel, command) = workspace.commands().pop().unwrap();
    assert!(matches!(command, Command::Cancel(_)));
    workspace.apply(Event::Completed(cancel, Err("cancel unavailable".into())));
    assert!(workspace.sessions[index].working());
    assert!(!workspace.sessions[index].stopping);

    let other = workspace.new_session(scope(Provider::Codex, "/b"));
    workspace.send(other, "failed creation".into());
    let (request, _) = workspace.commands().pop().unwrap();
    workspace.apply(Event::Completed(request, Err("offline".into())));
    assert_eq!(
        workspace.sessions[other].failed_prompt.as_deref(),
        Some("failed creation")
    );
    assert!(workspace.sessions[other].can_send());
}

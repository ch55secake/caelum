use super::*;

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

use super::*;

#[test]
fn question_conditions_types_and_custom_answers_are_validated_in_field_order() {
    let prompt = caelum_protocol::QuestionPrompt {
        provider: "codex".into(),
        project_path: "/a".into(),
        session_id: "s".into(),
        request_id: "q".into(),
        title: "Choose".into(),
        fields: vec![
            json!({"key":"enabled","type":"boolean","required":true}),
            json!({"key":"count","type":"integer","minimum":1,"maximum":4,"required":true,"when":[{"key":"enabled","op":"eq","value":true}]}),
            json!({"key":"names","type":"multiselect","custom":true,"required":true,"minItems":2}),
        ],
    };
    let draft = json!({"enabled":false,"count":"invalid stale value","names":["A","B","A"]})
        .as_object()
        .unwrap()
        .clone();
    let answer = forms::validate(&prompt, &draft);
    assert!(answer.errors.is_empty());
    assert!(!answer.values.contains_key("count"));
    assert_eq!(answer.values["names"], json!(["A", "B"]));
    let answer = forms::validate(
        &prompt,
        json!({"enabled":true,"count":"2.3","names":[]})
            .as_object()
            .unwrap(),
    );
    assert!(answer.errors.contains_key("count") && answer.errors.contains_key("names"));
}

#[test]
fn attachment_completion_retains_the_original_session_after_navigation() {
    let mut app = Workspace::default();
    let first = app.new_session(scope(Provider::Codex, "/a"));
    app.attach_file(first, "src/main.rs".into());
    let (read, _) = app.commands().pop().unwrap();
    let second = app.new_session(scope(Provider::OpenCode, "/b"));
    app.apply(Event::Completed(
        read,
        Ok(Reply::Task(TaskReply::File(
            caelum_protocol::ProjectFileContent {
                path: "src/main.rs".into(),
                content: "fn main() {}".into(),
                size_bytes: 12,
                truncated: false,
            },
        ))),
    ));
    assert_eq!(app.sessions[first].attachments.len(), 1);
    assert!(app.sessions[second].attachments.is_empty());
    assert_eq!(app.selected, Some(second));
}

#[test]
fn native_compaction_and_shells_reconcile_without_resurrecting_execution() {
    let mut conversation = Conversation::default();
    conversation.apply(
        Provider::OpenCode,
        "session.compaction.started",
        &json!({"_event_id":"evt_compact","_created":10}),
    );
    conversation.apply(
        Provider::OpenCode,
        "session.compaction.delta",
        &json!({"text":"Summary", "_created":11}),
    );
    conversation.apply(
        Provider::OpenCode,
        "session.compaction.ended",
        &json!({"text":"Summary", "_created":12}),
    );
    conversation.history(Provider::OpenCode, &[json!({"id":"msg_compact","type":"compaction","summary":"Summary","status":"completed","time":{"created":10}})], false);
    assert_eq!(conversation.entries.len(), 1);
    conversation.apply(
        Provider::OpenCode,
        "session.shell.started",
        &json!({"shell":{"id":"background","status":"running","command":"test"}}),
    );
    conversation.settle();
    assert!(!conversation.entries[1].complete);
    conversation.apply(Provider::OpenCode, "session.shell.ended", &json!({"shell":{"id":"background","status":"exited","exit":1,"command":"test"},"output":{"output":"failure"}}));
    assert!(!conversation.working);
    assert!(conversation.entries[1].complete);
    assert!(conversation.entries[1].status.contains("failed"));
}

#[test]
fn shell_events_win_over_old_lists_and_stop_stays_with_its_owner() {
    let mut app = Workspace::default();
    let scope = scope(Provider::OpenCode, "/a");
    let index = app.new_session(scope.clone());
    app.sessions[index].remote_id = Some("ses_a".into());
    app.refresh(index);
    let resume = app
        .commands()
        .into_iter()
        .find(|(_, c)| matches!(c, Command::Resume(_)))
        .unwrap()
        .0;
    app.apply(Event::Completed(
        resume,
        Ok(Reply::Session(summary(scope, "ses_a"))),
    ));
    let list = app
        .commands()
        .into_iter()
        .find(|(_, c)| matches!(c, Command::Shells(_)))
        .unwrap()
        .0;
    app.apply(Event::OpenCode(OpenCodeEvent {
        project_path:"/a".into(), event_type:"session.shell.started".into(),
        data:json!({"sessionID":"ses_a","shell":{"id":"sh_one","status":"running","command":"cargo test","time":{"started":1000}}}),
        id:None,created:None,location:None,
    }));
    app.apply(Event::Completed(list, Ok(Reply::Shells(vec![]))));
    assert_eq!(
        app.sessions[index].shells.len(),
        1,
        "a stale list must not hide a newly started shell"
    );
    app.commands();
    let output = Task::Shell {
        project: "/a".into(),
        session: "ses_a".into(),
        id: "sh_one".into(),
        stop: false,
    };
    app.task(output.clone());
    let read = app.commands().pop().unwrap().0;
    app.apply(Event::Completed(
        read,
        Ok(Reply::Task(TaskReply::Shell("retained output".into()))),
    ));
    app.task(output);
    let late_read = app.commands().pop().unwrap().0;
    app.task(Task::Shell {
        project: "/a".into(),
        session: "ses_a".into(),
        id: "sh_one".into(),
        stop: true,
    });
    let stop = app.commands().pop().unwrap().0;
    let other = app.new_session(crate::application::Scope {
        provider: Provider::OpenCode,
        project: "/b".into(),
    });
    app.sessions[other].draft = "Independent draft".into();
    app.apply(Event::Completed(
        stop,
        // Stopping succeeds even if the final output read failed.
        Ok(Reply::Task(TaskReply::Shell(String::new()))),
    ));
    assert!(app.sessions[index].shells.is_empty());
    let entry = app.sessions[index].conversation.entries.last().unwrap();
    assert!(entry.complete);
    assert_eq!(entry.output, "retained output");
    assert_eq!(entry.started_at, Some(1000));
    assert!(app.services.checkouts["/a"].shell_output.is_empty());
    app.apply(Event::Completed(
        late_read,
        Ok(Reply::Task(TaskReply::Shell("stale output".into()))),
    ));
    assert!(
        app.services.checkouts["/a"].shell_output.is_empty(),
        "a late poll must not resurrect a stopped shell's cache"
    );
    assert_eq!(app.selected, Some(other));
    assert_eq!(app.sessions[other].draft, "Independent draft");
}

#[test]
fn failed_approval_reply_stays_with_its_origin_after_navigation() {
    let mut workspace = Workspace::default();
    let first = started(&mut workspace, scope(Provider::Codex, "/a"), "one");
    workspace.approve(
        caelum_protocol::ApprovalPrompt {
            provider: "codex".into(),
            project_path: "/a".into(),
            session_id: Some("one".into()),
            request_id: "request".into(),
            summary: "Run".into(),
            description: "Command".into(),
        },
        caelum_protocol::ApprovalDecision::AllowOnce,
    );
    let request = workspace.commands().pop().unwrap().0;
    let second = workspace.new_session(scope(Provider::Codex, "/b"));
    workspace.apply(Event::Completed(request, Err("Approval expired".into())));
    assert_eq!(
        workspace.sessions[first].error.as_deref(),
        Some("Approval expired")
    );
    assert!(workspace.sessions[second].error.is_none());
    assert!(workspace.error.is_none());
}

use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use caelum_protocol::{CodexEvent, OpenCodeEvent, ProjectInfo, SessionHistoryPage};
use serde_json::json;

use super::*;

fn scope(provider: Provider, project: &str) -> Scope {
    Scope {
        provider,
        project: project.into(),
    }
}

fn summary(scope: Scope, id: &str) -> Summary {
    Summary {
        key: SessionKey {
            scope,
            id: id.into(),
        },
        title: "Saved session".into(),
        working: false,
        options: Options::default(),
    }
}

fn codex(project: &str, id: &str, method: &str, mut data: serde_json::Value) -> Event {
    data["threadId"] = id.into();
    Event::Codex(CodexEvent {
        project_path: project.into(),
        method: method.into(),
        params: data,
        request_id: None,
    })
}

fn page(data: Vec<serde_json::Value>, working: bool) -> Reply {
    Reply::History(SessionHistoryPage {
        data,
        next_cursor: None,
        is_working: Some(working),
        model: None,
        effort: None,
        agent: None,
    })
}

fn started(workspace: &mut Workspace, scope: Scope, remote: &str) -> usize {
    let index = workspace.new_session(scope.clone());
    assert!(workspace.send(index, "Hello".into()));
    let (request, _) = workspace.commands().pop().unwrap();
    workspace.apply(Event::Completed(
        request,
        Ok(Reply::Session(summary(scope, remote))),
    ));
    let (request, _) = workspace.commands().pop().unwrap();
    workspace.apply(Event::Completed(request, Ok(Reply::Done)));
    index
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

#[test]
fn history_reconciles_overlaps_lagging_deltas_and_unicode_for_both_providers() {
    for provider in Provider::ALL {
        let mut conversation = Conversation::default();
        let (method, delta, history, complete) = match provider {
            Provider::Codex => (
                "item/agentMessage/delta",
                json!({"itemId":"answer","delta":"café"}),
                vec![
                    json!({"status":"inProgress", "items":[{"id":"answer","type":"agentMessage","text":"Hello café ☁️"}]}),
                ],
                (
                    "item/completed",
                    json!({"item":{"id":"answer","type":"agentMessage","text":"Hello café ☁️ done"}}),
                ),
            ),
            Provider::OpenCode => (
                "session.text.delta",
                json!({"assistantMessageID":"answer","delta":"café"}),
                vec![
                    json!({"id":"answer","type":"assistant","content":[{"type":"text","text":"Hello café ☁️"}]}),
                ],
                (
                    "session.text.ended",
                    json!({"assistantMessageID":"answer","text":"Hello café ☁️ done"}),
                ),
            ),
        };
        conversation.apply(provider, method, &delta);
        conversation.history(provider, &history, false);
        let mut next = delta.clone();
        next["delta"] = " ☁️ done".into();
        conversation.apply(provider, method, &next);
        assert_eq!(conversation.entries.len(), 1);
        assert_eq!(conversation.entries[0].text, "Hello café ☁️ done");
        conversation.apply(provider, complete.0, &complete.1);
        conversation.apply(provider, method, &delta);
        conversation.history(provider, &history, false);
        assert_eq!(
            conversation.entries[0].text, "Hello café ☁️ done",
            "late snapshots/deltas cannot truncate completed text"
        );
    }
}

#[test]
fn repeated_prompts_and_older_pages_do_not_duplicate_or_adopt_the_wrong_message() {
    let mut conversation = Conversation::default();
    conversation.optimistic("local-1".into(), "again".into());
    conversation.history(
        Provider::OpenCode,
        &[json!({"id":"old","type":"user","text":"again"})],
        true,
    );
    assert_eq!(conversation.entries.len(), 2);
    assert!(conversation.entries[1].optimistic);
    conversation.history(
        Provider::OpenCode,
        &[
            json!({"id":"old","type":"user","text":"again"}),
            json!({"id":"new","type":"user","text":"again"}),
        ],
        false,
    );
    assert_eq!(conversation.entries.len(), 2);
    assert!(conversation.entries.iter().all(|entry| !entry.optimistic));
    conversation.history(
        Provider::OpenCode,
        &[json!({"id":"old","type":"user","text":"again"})],
        true,
    );
    assert_eq!(conversation.entries[1].id, "opencode-new");
}

#[test]
fn delayed_history_preserves_live_tool_output_snapshots() {
    let mut conversation = Conversation::default();
    let history = vec![json!({"id":"message","type":"assistant","content":[{
        "id":"tool", "type":"tool", "name":"bash", "state":{"status":"running","input":{"command":"test"},"content":[{"type":"text","text":"prefix"}]}
    }]})];
    conversation.history(Provider::OpenCode, &history, false);
    conversation.apply(
        Provider::OpenCode,
        "session.tool.updated",
        &json!({"id":"tool","output":"prefix and live output"}),
    );
    conversation.history(Provider::OpenCode, &history, false);
    assert_eq!(conversation.entries.len(), 1);
    assert_eq!(conversation.entries[0].output, "prefix and live output");
}

#[test]
fn delayed_history_cannot_overwrite_live_activity_or_another_session() {
    let mut workspace = Workspace::default();
    let first = started(&mut workspace, scope(Provider::Codex, "/a"), "one");
    workspace.refresh(first);
    let (resume, _) = workspace.commands().pop().unwrap();
    workspace.apply(Event::Completed(
        resume,
        Ok(Reply::Session(summary(scope(Provider::Codex, "/a"), "one"))),
    ));
    let (history, _) = workspace.commands().pop().unwrap();
    let second = workspace.new_session(scope(Provider::Codex, "/a"));
    workspace.apply(codex(
        "/a",
        "one",
        "item/agentMessage/delta",
        json!({"itemId":"answer","delta":"live"}),
    ));
    workspace.apply(Event::Completed(history, Ok(page(vec![json!({"status":"inProgress","items":[{"id":"answer","type":"agentMessage","text":"li"}]})], false))));
    assert!(workspace.sessions[first].working());
    assert_eq!(
        workspace.sessions[first]
            .conversation
            .entries
            .last()
            .unwrap()
            .text,
        "live"
    );
    assert_eq!(workspace.selected, Some(second));
    assert!(workspace.sessions[second].conversation.entries.is_empty());
}

#[test]
fn history_started_before_cancellation_cannot_resurrect_work() {
    let mut workspace = Workspace::default();
    let index = started(&mut workspace, scope(Provider::Codex, "/a"), "one");
    workspace.refresh(index);
    let resume = workspace.commands().pop().unwrap().0;
    workspace.apply(Event::Completed(
        resume,
        Ok(Reply::Session(Summary {
            working: true,
            ..summary(scope(Provider::Codex, "/a"), "one")
        })),
    ));
    let history = workspace.commands().pop().unwrap().0;
    workspace.cancel(index);
    let cancel = workspace.commands().pop().unwrap().0;
    workspace.apply(Event::Completed(cancel, Ok(Reply::Done)));
    workspace.apply(Event::Completed(history, Ok(page(Vec::new(), true))));
    assert!(!workspace.sessions[index].working());
    assert!(!workspace.sessions[index].stopping);
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

#[test]
fn discovery_during_creation_adopts_the_stable_local_session_once() {
    let mut workspace = Workspace::default();
    let origin = scope(Provider::Codex, "/a");
    let index = workspace.new_session(origin.clone());
    workspace.refresh_project();
    let list = workspace.commands().into_iter().find(|(_, command)| matches!(command, Command::List(scope) if scope.provider == Provider::Codex)).unwrap().0;
    workspace.send(index, "Hello".into());
    let create = workspace.commands().pop().unwrap().0;
    workspace.apply(Event::Completed(
        list,
        Ok(Reply::Sessions(vec![summary(origin.clone(), "created")])),
    ));
    assert_eq!(workspace.sessions.len(), 1);
    workspace.apply(Event::Completed(
        create,
        Ok(Reply::Session(summary(origin, "created"))),
    ));
    assert_eq!(workspace.sessions.len(), 1);
    assert_eq!(
        workspace.sessions[index].remote_id.as_deref(),
        Some("created")
    );
    assert!(workspace.sessions[index].working());
}

#[test]
fn initial_history_precedes_nonoverlapping_live_output_and_refresh_keeps_pagination() {
    let mut workspace = Workspace::default();
    let index = workspace.new_session(scope(Provider::Codex, "/a"));
    workspace.sessions[index].remote_id = Some("one".into());
    workspace.sessions[index].loaded = false;
    workspace.refresh(index);
    let resume = workspace.commands().pop().unwrap().0;
    workspace.apply(Event::Completed(
        resume,
        Ok(Reply::Session(summary(scope(Provider::Codex, "/a"), "one"))),
    ));
    let history = workspace.commands().pop().unwrap().0;
    workspace.apply(codex(
        "/a",
        "one",
        "item/agentMessage/delta",
        json!({"itemId":"new", "delta":"New response"}),
    ));
    workspace.apply(Event::Completed(history, Ok(Reply::History(SessionHistoryPage {
        data: vec![json!({"status":"completed","items":[{"id":"old","type":"agentMessage","text":"Older response"}]})],
        next_cursor: Some("older".into()), is_working: Some(false), model: None, effort: None, agent: None,
    }))));
    assert_eq!(
        workspace.sessions[index].conversation.entries[0].id,
        "codex-old"
    );
    assert_eq!(
        workspace.sessions[index].conversation.entries[1].id,
        "codex-new"
    );
    assert!(workspace.sessions[index].working());
    workspace.older(index);
    let older = workspace.commands().pop().unwrap().0;
    workspace.apply(Event::Completed(older, Ok(page(Vec::new(), false))));
    workspace.refresh(index);
    let resume = workspace.commands().pop().unwrap().0;
    workspace.apply(Event::Completed(
        resume,
        Ok(Reply::Session(summary(scope(Provider::Codex, "/a"), "one"))),
    ));
    let refresh = workspace.commands().pop().unwrap().0;
    workspace.apply(Event::Completed(
        refresh,
        Ok(Reply::History(SessionHistoryPage {
            data: Vec::new(),
            next_cursor: Some("older".into()),
            is_working: None,
            model: None,
            effort: None,
            agent: None,
        })),
    ));
    assert!(
        workspace.sessions[index].next_cursor.is_none(),
        "refresh must not reset the already-exhausted older cursor"
    );
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

#[test]
fn blocking_requests_run_independently_and_wake_the_consumer() {
    let (release, gate) = mpsc::channel();
    let gate = Mutex::new(gate);
    let (entered, started) = mpsc::channel();
    let (waker, wakes) = mpsc::channel();
    let mut runtime = Runtime::with_executor(
        Arc::new(move |command| {
            if matches!(command, Command::Open(path) if path == "slow") {
                entered.send(()).unwrap();
                gate.lock().unwrap().recv().unwrap();
            }
            Ok(Reply::Done)
        }),
        Arc::new(move || {
            let _ = waker.send(());
        }),
    );
    runtime.submit(1, Command::Open("slow".into())).unwrap();
    started.recv_timeout(Duration::from_secs(2)).unwrap();
    runtime.submit(2, Command::Open("fast".into())).unwrap();
    wakes.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(matches!(
        runtime.drain().as_slice(),
        [Event::Completed(2, Ok(Reply::Done))]
    ));
    release.send(()).unwrap();
    wakes.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(matches!(
        runtime.drain().as_slice(),
        [Event::Completed(1, Ok(Reply::Done))]
    ));
}

#[cfg(unix)]
#[test]
fn shared_shutdown_denies_approvals_and_reaps_children_even_with_cloned_adapters() {
    use crate::processes::ProcessError;
    use caelum_protocol::{ApprovalDecision, PermissionRequest};
    let providers = ProviderRuntime::default();
    let clone = providers.clone();
    let waiter = providers
        .approvals
        .request(PermissionRequest {
            provider: "codex".into(),
            project_path: "/a".into(),
            session_id: Some("one".into()),
            request_id: "approval".into(),
            summary: "Run".into(),
            description: "test".into(),
            provider_details: json!({}),
        })
        .unwrap();
    providers
        .processes
        .register(
            std::process::Command::new("/bin/sleep")
                .arg("30")
                .spawn()
                .unwrap(),
        )
        .unwrap();
    providers.shutdown();
    assert_eq!(waiter.wait(Duration::from_secs(1)), ApprovalDecision::Deny);
    assert_eq!(
        clone
            .processes
            .register(
                std::process::Command::new("/bin/sleep")
                    .arg("30")
                    .spawn()
                    .unwrap()
            )
            .err(),
        Some(ProcessError::Closed)
    );
    clone.shutdown();
}

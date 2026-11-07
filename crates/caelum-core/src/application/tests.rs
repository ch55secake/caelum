use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use caelum_protocol::{CodexEvent, OpenCodeEvent, ProjectInfo, SessionHistoryPage};
use serde_json::json;

use super::*;

#[test]
fn real_checkout_watcher_refreshes_edits_and_staging_without_losing_the_draft() {
    use std::{fs, process::Command as Process, time::Instant};
    let directory =
        std::env::temp_dir().join(format!("caelum-live-checkout-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let directory = directory.canonicalize().unwrap();
    let git = |args: &[&str]| {
        let result = Process::new("git")
            .args(args)
            .current_dir(&directory)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    };
    git(&["init", "-q"]);
    fs::write(directory.join("Cargo.lock"), "before\n").unwrap();
    git(&["add", "Cargo.lock"]);
    git(&[
        "-c",
        "user.name=Fixture",
        "-c",
        "user.email=fixture@example.com",
        "-c",
        "commit.gpgsign=false",
        "commit",
        "-qm",
        "fixture",
    ]);
    git(&["checkout", "--detach", "-q"]);
    let project = directory.to_string_lossy().into_owned();
    let mut runtime = Runtime::new(|| {});
    runtime.watch_checkouts(vec![project.clone()]);
    let mut app = Workspace::default();
    app.projects.push(ProjectInfo {
        path: project.clone(),
        name: "Fixture".into(),
    });
    let index = app.new_session(scope(Provider::Codex, &project));
    app.sessions[index].draft = "Keep this independent draft".into();
    app.commands();
    app.open_file(project.clone(), "Cargo.lock".into());
    app.checkout_changed(&project);
    let mut wait = |app: &mut Workspace, ready: &dyn Fn(&Workspace) -> bool| {
        let deadline = Instant::now() + Duration::from_secs(6);
        loop {
            for (id, command) in app.commands() {
                runtime.submit(id, command).unwrap();
            }
            for event in runtime.drain() {
                app.apply(event);
            }
            if ready(app) {
                break;
            }
            assert!(Instant::now() < deadline, "checkout refresh timed out");
            std::thread::sleep(Duration::from_millis(20));
        }
    };
    wait(&mut app, &|a| {
        a.services.checkouts[&project].git.is_some()
            && a.services.checkouts[&project]
                .files
                .contains_key("Cargo.lock")
    });
    fs::write(directory.join("Cargo.lock"), "after\n").unwrap();
    wait(&mut app, &|a| {
        let c = &a.services.checkouts[&project];
        c.files["Cargo.lock"].content == "after\n" && c.git.as_ref().unwrap().changes.len() == 1
    });
    app.task(Task::Diff {
        project: project.clone(),
        path: "Cargo.lock".into(),
    });
    wait(&mut app, &|a| {
        a.services.checkouts[&project]
            .diffs
            .contains_key("Cargo.lock")
    });
    git(&["add", "Cargo.lock"]);
    wait(&mut app, &|a| {
        a.services.checkouts[&project].diffs["Cargo.lock"]
            .staged
            .contains("+after")
    });
    assert_eq!(app.sessions[index].draft, "Keep this independent draft");
    assert_eq!(
        app.services.checkouts[&project].selected_file.as_deref(),
        Some("Cargo.lock")
    );
    drop(runtime);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn checkout_refresh_coalesces_edits_without_losing_inflight_file_updates() {
    let mut app = Workspace::default();
    app.projects.push(ProjectInfo {
        path: "/a".into(),
        name: "a".into(),
    });
    let checkout = app.services.checkouts.entry("/a".into()).or_default();
    checkout.tabs.push("file.rs".into());
    checkout.selected_file = Some("file.rs".into());
    app.checkout_changed("/a");
    let first = app.commands();
    let git = first
        .iter()
        .find(|(_, c)| matches!(c, Command::Task(Task::Git { .. })))
        .unwrap()
        .0;
    let file = first
        .iter()
        .find(|(_, c)| matches!(c, Command::Task(Task::File { .. })))
        .unwrap()
        .0;
    for _ in 0..20 {
        app.checkout_changed("/a");
    }
    assert!(app.commands().is_empty());
    let status = serde_json::from_value(json!({"projectPath":"/a","repositoryRoot":"/a","branch":null,"changes":[{"path":"file.rs","indexStatus":" ","worktreeStatus":"M"}]})).unwrap();
    app.apply(Event::Completed(
        git,
        Ok(Reply::Task(TaskReply::Git(status, vec![]))),
    ));
    let second = app.commands();
    assert_eq!(
        second
            .iter()
            .filter(|(_, c)| matches!(c, Command::Task(Task::Git { .. })))
            .count(),
        1
    );
    assert!(!second
        .iter()
        .any(|(_, c)| matches!(c, Command::Task(Task::File { .. }))));
    app.apply(Event::Completed(
        file,
        Ok(Reply::Task(TaskReply::File(
            caelum_protocol::ProjectFileContent {
                path: "file.rs".into(),
                content: "older snapshot".into(),
                size_bytes: 14,
                truncated: false,
            },
        ))),
    ));
    assert!(
        app.commands()
            .iter()
            .any(|(_, c)| matches!(c, Command::Task(Task::File { path, .. }) if path == "file.rs")),
        "a change during an in-flight read must refresh again"
    );
    assert_eq!(
        app.services.checkouts["/a"].selected_file.as_deref(),
        Some("file.rs")
    );
}

#[test]
fn durable_submission_waits_for_its_snapshot_but_not_for_later_typing() {
    let mut app = Workspace::default();
    let index = app.new_session(scope(Provider::Codex, "/a"));
    app.enable_store("/unused-in-reducer-test".into());
    let (load, _) = app.commands().pop().unwrap();
    app.apply(Event::Completed(load, Ok(Reply::Snapshot(None))));
    app.sessions[index].draft = "durable prompt".into();
    assert!(app.send(index, "durable prompt".into()));
    let (save, command) = app.commands().pop().unwrap();
    let Command::Save(_, snapshot) = command else {
        panic!("save must precede provider admission")
    };
    assert_eq!(
        snapshot.sessions[index]
            .pending_submission
            .as_ref()
            .map(|p| p.text.as_str()),
        Some("durable prompt")
    );
    app.sessions[index].draft = "newer draft".into();
    assert!(app.commands().is_empty());
    app.apply(Event::Completed(save, Ok(Reply::Done)));
    let commands = app.commands();
    assert_eq!(
        commands
            .iter()
            .filter(|(_, c)| matches!(c, Command::Start(..)))
            .count(),
        1
    );
    assert!(commands.iter().any(
        |(_, c)| matches!(c, Command::Save(_, s) if s.sessions[index].draft == "newer draft")
    ));
}

#[test]
fn failed_disk_write_blocks_admission_and_retry_keeps_the_original_prompt() {
    let mut app = Workspace::default();
    let index = app.new_session(scope(Provider::Codex, "/a"));
    app.enable_store("/unused-in-reducer-test".into());
    let (load, _) = app.commands().pop().unwrap();
    app.apply(Event::Completed(load, Ok(Reply::Snapshot(None))));
    app.send(index, "keep through disk failure".into());
    let (save, _) = app.commands().pop().unwrap();
    app.apply(Event::Completed(save, Err("disk full".into())));
    assert!(app.commands().is_empty());
    assert_eq!(
        app.snapshot().sessions[index]
            .pending_submission
            .as_ref()
            .map(|p| p.text.as_str()),
        Some("keep through disk failure")
    );
    app.retry_save();
    let (save, _) = app.commands().pop().unwrap();
    app.apply(Event::Completed(save, Ok(Reply::Done)));
    assert!(matches!(
        app.commands().as_slice(),
        [(_, Command::Start(..))]
    ));
}

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
fn search_and_review_pages_reject_late_results_from_old_targets() {
    let mut app = Workspace::default();
    for query in ["old", "new"] {
        app.task(Task::Search {
            project: "/a".into(),
            query: query.into(),
            mode: caelum_protocol::ProjectSearchMode::Files,
        });
    }
    let commands = app.commands();
    let result = || {
        Reply::Task(TaskReply::Search(caelum_protocol::ProjectSearchResults {
            matches: vec![],
            truncated: false,
            skipped: 0,
        }))
    };
    app.apply(Event::Completed(commands[1].0, Ok(result())));
    app.apply(Event::Completed(commands[0].0, Ok(result())));
    assert_eq!(app.services.checkouts["/a"].search_query, "new");
    let key = ("/a".into(), "https://github.com/a/b/pull/1".into());
    let details: caelum_protocol::GitHubPullRequestDetails = serde_json::from_value(json!({"number":1,"title":"PR","url":key.1,"isDraft":false,"body":"","state":"OPEN","author":null,"baseRefName":"main","headRefName":"feature","headRefOid":"new","mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","additions":1,"deletions":0,"changedFiles":1,"reviewDecision":null,"statusCheckRollup":[]})).unwrap();
    app.services.reviews.entry(key.clone()).or_default().details = Some(details);
    app.task(Task::ReviewFiles {
        project: key.0.clone(),
        url: key.1.clone(),
        head: "old".into(),
        page: 1,
    });
    let (id, _) = app.commands().pop().unwrap();
    app.apply(Event::Completed(
        id,
        Ok(Reply::Task(TaskReply::ReviewFiles(
            caelum_protocol::GitHubPullRequestFiles {
                files: vec![],
                base_oid: "base".into(),
                next_page: Some(2),
            },
        ))),
    ));
    assert!(!app.services.reviews[&key].files_loaded);
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
    let other = app.new_session(super::Scope {
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
fn workspace_roundtrip_keeps_drafts_and_unknown_versions_are_not_imported() {
    let directory = std::env::temp_dir().join(format!(
        "caelum-snapshot-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let store = Store(directory.join("workspace.json"));
    let mut app = Workspace::default();
    app.preferences.black_backgrounds = true;
    app.preferences.theme_family = "nord".into();
    app.preferences.accent_color = "teal".into();
    let index = app.new_session(scope(Provider::OpenCode, "/project with spaces"));
    app.sessions[index].draft = "café ☁️\nnext line".into();
    app.attach(
        index,
        Attachment::new("a.rs".into(), "内容".into(), Some((2, 2)), false),
    );
    store.save(&app.snapshot()).unwrap();
    assert_eq!(store.load().unwrap(), Some(app.snapshot()));
    let mut old = serde_json::to_value(app.snapshot()).unwrap();
    old["preferences"]
        .as_object_mut()
        .unwrap()
        .remove("blackBackgrounds");
    let old: Snapshot = serde_json::from_value(old).unwrap();
    assert!(!old.preferences.black_backgrounds);
    assert_eq!(old.preferences.theme_family, "nord");
    assert_eq!(old.preferences.accent_color, "teal");
    let mut future = app.snapshot();
    future.version = 2;
    store.save(&future).unwrap();
    assert!(store.load().is_err());
    assert_eq!(
        serde_json::from_slice::<Snapshot>(&std::fs::read(&store.0).unwrap())
            .unwrap()
            .version,
        2
    );
    std::fs::remove_dir_all(directory).unwrap();
}

fn scope(provider: Provider, project: &str) -> Scope {
    Scope {
        provider,
        project: project.into(),
    }
}

#[test]
fn stale_opencode_archive_error_forgets_the_session_locally() {
    let mut app = Workspace::default();
    let project = "/project";
    let index = app.new_session(scope(Provider::OpenCode, project));
    app.sessions[index].remote_id = Some("ses_stale".into());
    app.sessions[index].draft = "Keep this draft".into();
    app.attach(
        index,
        Attachment::new("src/main.rs".into(), "snapshot".into(), None, false),
    );
    app.archive(index, true);
    let (request, command) = app.commands().pop().unwrap();
    assert!(matches!(command, Command::Archive(_, true)));

    app.apply(Event::Completed(
        request,
        Err("OpenCode session does not belong to this project".into()),
    ));

    assert!(app.sessions[index].forgotten);
    assert!(!app.sessions[index].archiving);
    assert_eq!(app.last_archived, None, "there is no provider Undo action");
    let selected = app.selected.unwrap();
    assert_ne!(selected, index);
    assert_eq!(app.sessions[selected].draft, "Keep this draft");
    assert_eq!(app.sessions[selected].attachments[0].content, "snapshot");
    assert!(app.sessions[selected]
        .error
        .as_deref()
        .is_some_and(|error| error.contains("not archived in OpenCode")));

    let snapshot = app.snapshot();
    assert_eq!(snapshot.sessions.len(), 1);
    assert_eq!(snapshot.selected, Some(0));
    assert_eq!(snapshot.sessions[0].draft, "Keep this draft");
    assert_eq!(snapshot.sessions[0].remote_id, None);
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
    restored.apply(Event::Completed(load, Ok(Reply::Snapshot(Some(snapshot)))));
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
fn restart_preserves_unsent_and_unacknowledged_prompts_and_pauses_all_queues() {
    let mut original = Workspace::default();
    let index = original.new_session(scope(Provider::Codex, "/a"));
    original.sessions[index].failed_prompt = Some("previous failed prompt".into());
    original.send(index, "pending admission".into());
    original.send(index, "queued after admission".into());
    original.sessions[index].draft = "newer draft".into();
    let mut restored = Workspace::default();
    restored.enable_store("/unused-in-reducer-test".into());
    let (load, _) = restored.commands().pop().unwrap();
    restored.apply(Event::Completed(
        load,
        Ok(Reply::Snapshot(Some(original.snapshot()))),
    ));
    assert_eq!(restored.sessions[index].draft, "newer draft");
    assert_eq!(
        restored.sessions[index].failed_prompt.as_deref(),
        Some("previous failed prompt")
    );
    assert_eq!(
        restored.sessions[index]
            .queue
            .iter()
            .map(|p| p.text.as_str())
            .collect::<Vec<_>>(),
        ["pending admission", "queued after admission"]
    );
    assert!(restored.sessions[index].queue_paused);
    assert!(!restored
        .commands()
        .iter()
        .any(|(_, c)| matches!(c, Command::Start(..) | Command::Send(..))));
}

#[test]
fn saving_errors_never_block_cancellation_or_permission_replies() {
    let mut app = Workspace::default();
    let index = started(&mut app, scope(Provider::Codex, "/a"), "s");
    app.enable_store("/unused-in-reducer-test".into());
    let (load, _) = app.commands().pop().unwrap();
    app.apply(Event::Completed(load, Ok(Reply::Snapshot(None))));
    let (save, _) = app.commands().pop().unwrap();
    app.apply(Event::Completed(save, Err("disk full".into())));
    app.cancel(index);
    app.approve(
        caelum_protocol::ApprovalPrompt {
            provider: "codex".into(),
            project_path: "/a".into(),
            session_id: Some("s".into()),
            request_id: "approval".into(),
            summary: "Test".into(),
            description: String::new(),
        },
        caelum_protocol::ApprovalDecision::Deny,
    );
    let commands = app.commands();
    assert!(commands
        .iter()
        .any(|(_, c)| matches!(c, Command::Cancel(_))));
    assert!(commands
        .iter()
        .any(|(_, c)| matches!(c, Command::Approve(..))));
}

#[test]
fn same_head_refresh_rejects_old_pages_without_unlocking_an_active_mutation() {
    let mut app = Workspace::default();
    let key = ("/a".to_owned(), "https://github.com/a/b/pull/1".to_owned());
    let details = serde_json::from_value(json!({"number":1,"title":"PR","url":key.1,"isDraft":false,"body":"","state":"OPEN","author":null,"baseRefName":"main","headRefName":"feature","headRefOid":"head","mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","additions":1,"deletions":0,"changedFiles":1,"reviewDecision":null,"statusCheckRollup":[]})).unwrap();
    app.services.reviews.entry(key.clone()).or_default().details = Some(details);
    let files = Task::ReviewFiles {
        project: key.0.clone(),
        url: key.1.clone(),
        head: "head".into(),
        page: 1,
    };
    let mutation = Task::UpdateBranch {
        project: key.0.clone(),
        url: key.1.clone(),
        head: "head".into(),
    };
    app.task(files.clone());
    let (old, _) = app.commands().pop().unwrap();
    app.task(mutation.clone());
    let (update, _) = app.commands().pop().unwrap();
    app.services.refresh_review(&key.0, &key.1);
    assert!(app.services.busy(&mutation));
    app.task(files.clone());
    let (new, _) = app.commands().pop().unwrap();
    app.apply(Event::Completed(old, Err("stale failure".into())));
    assert!(app.services.busy(&files));
    assert!(app.services.reviews[&key].error.is_none());
    app.apply(Event::Completed(
        new,
        Ok(Reply::Task(TaskReply::ReviewFiles(
            caelum_protocol::GitHubPullRequestFiles {
                files: vec![],
                base_oid: "base".into(),
                next_page: None,
            },
        ))),
    ));
    assert!(app.services.reviews[&key].files_loaded);
    app.apply(Event::Completed(update, Ok(Reply::Task(TaskReply::Done))));
    assert!(!app.services.busy(&mutation));
    assert!(app.services.reviews[&key].notice.is_some());
}

#[test]
fn disconnect_recovery_is_bounded_and_resuming_restored_queues_loads_history_first() {
    let mut app = Workspace::default();
    let index = started(&mut app, scope(Provider::Codex, "/a"), "s");
    app.send(index, "queued".into());
    app.apply(codex("/a", "s", "codex/processExited", json!({})));
    let (resume, _) = app.commands().pop().unwrap();
    app.apply(Event::Completed(resume, Err("still offline".into())));
    app.apply(codex("/a", "s", "codex/processExited", json!({})));
    assert!(app.commands().is_empty());
    app.resume_queue(index);
    assert!(matches!(
        app.commands().as_slice(),
        [(_, Command::Resume(_))]
    ));
    assert_eq!(app.sessions[index].queue.len(), 1);
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

use super::{Command, Desktop, Event, Notice, Options, Reply, Scope, SessionKey};
use crate::application::{Task, TaskReply};
use caelum_protocol::{ApprovalDecision, ProjectSearchMode};
use std::{fs, path::Path};

pub(super) fn codex(desktop: &mut Desktop, scope: Scope) {
    let dir = Path::new(&scope.project).to_owned();
    let options = Options {
        model: Some("test-model".into()),
        effort: Some("high".into()),
        agent: None,
    };
    let Reply::Session(session) = desktop.request(Command::Start(scope, options.clone())) else {
        panic!("no Codex thread")
    };
    let key = session.key;
    desktop.id += 1;
    desktop
        .client
        .submit(
            desktop.id,
            Command::Send(key.clone(), "request-approval".into(), options),
        )
        .unwrap();
    desktop.until(|n| matches!(n,Notice::Event(e) if matches!(*e,Event::Codex(ref e) if e.method=="item/agentMessage/delta" && e.params["delta"]=="Hello from Codex")).then_some(()));
    let turn = dir.join("turn-request.json");
    let sent: serde_json::Value = serde_json::from_slice(&fs::read(&turn).unwrap()).unwrap();
    assert_eq!(sent["threadId"], key.id);
    assert_eq!(sent["model"], "test-model");
    assert_eq!(sent["effort"], "high");
    fs::rename(&turn, dir.join("accepted-turn.json")).unwrap();
    desktop.interrupt_link();
    let approval = desktop
        .approvals
        .iter()
        .find(|a| a.provider == "codex" && a.session_id.as_ref() == Some(&key.id))
        .expect("same pending Codex request")
        .clone();
    assert!(matches!(
        desktop.request(Command::Approve(approval, ApprovalDecision::Deny)),
        Reply::Done
    ));
    assert!(
        matches!(desktop.request(Command::Resume(key.clone())),Reply::Session(s) if s.key==key)
    );
    assert!(
        matches!(desktop.request(Command::History(key,None)),Reply::History(page) if page.data.iter().any(|t|t["items"][0]["aggregatedOutput"]=="clean"))
    );
    assert!(!turn.exists(), "reconnecting resubmitted the Codex turn");
}

pub(super) fn checkout(desktop: &mut Desktop, key: &SessionKey) {
    let project = &key.scope.project;
    let task = |desktop: &mut Desktop, t| match desktop.request(Command::Task(t)) {
        Reply::Task(reply) => reply,
        _ => panic!("expected a checkout reply"),
    };
    let listing = task(
        desktop,
        Task::Directory {
            project: project.clone(),
            path: String::new(),
            offset: 0,
        },
    );
    assert!(
        matches!(listing,TaskReply::Directory(d) if d.entries.iter().any(|f|f.name=="note.txt"))
    );
    let note = task(
        desktop,
        Task::File {
            project: project.clone(),
            path: "note.txt".into(),
        },
    );
    assert!(matches!(note,TaskReply::File(f) if f.content=="VM edit\n"));
    let results = task(
        desktop,
        Task::Search {
            project: project.clone(),
            query: "VM edit".into(),
            mode: ProjectSearchMode::Content,
        },
    );
    assert!(matches!(results,TaskReply::Search(r) if r.matches.iter().any(|m|m.path=="note.txt")));
    let git = task(
        desktop,
        Task::Git {
            project: project.clone(),
        },
    );
    assert!(
        matches!(git,TaskReply::Git(g,_) if g.branch.as_deref()==Some("feat/native") && g.changes.iter().any(|f|f.path=="note.txt"))
    );
    let diff = task(
        desktop,
        Task::Diff {
            project: project.clone(),
            path: "note.txt".into(),
        },
    );
    assert!(matches!(diff,TaskReply::Diff(d) if d.unstaged.contains("+VM edit")));
    assert!(
        matches!(desktop.request(Command::Shells(key.clone())),Reply::Shells(s) if s[0].id=="sh_example")
    );
    let output = task(
        desktop,
        Task::Shell {
            project: project.clone(),
            session: key.id.clone(),
            id: "sh_example".into(),
            stop: true,
        },
    );
    assert!(matches!(output,TaskReply::Shell(s) if s.ends_with("latest café output")));
    assert!(
        matches!(desktop.request(Command::Shells(key.clone())),Reply::Shells(s) if s.is_empty())
    );
}

use super::*;
use crate::application::{
    Command as Op, Options, Provider, Reply, Scope, SessionKey, Task, TaskReply,
};
use std::sync::Arc;

fn attach_test(tx: &mpsc::SyncSender<Action>) -> UnixStream {
    let (client, peer) = UnixStream::pair().unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    tx.send(Action::Attach(peer)).unwrap();
    client
}

fn event(c: &mut UnixStream) -> Event {
    let ServerFrame::Event(e) = read_frame(c).unwrap() else {
        panic!("expected peer event")
    };
    *e
}

fn request(c: &mut UnixStream, id: u64, op: Op) -> (Result<Reply, String>, Vec<Event>) {
    write_frame(c, &CommandFrame(id, op)).unwrap();
    let mut events = Vec::new();
    loop {
        match event(c) {
            Event::Completed(owner, reply) if owner == id => return (reply, events),
            e => events.push(e),
        }
    }
}

#[test]
fn reconnect_reads_native_history_and_files_without_replaying_a_provider_turn() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("project");
    fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap().to_str().unwrap().to_owned();
    fs::write(Path::new(&root).join("remote.txt"), "from the VM").unwrap();
    let py = temp.path().join("codex.py");
    fs::write(&py, include_str!("../../../codex/tests/fixtures/server.py")).unwrap();
    let codex = temp.path().join("codex");
    crate::test_support::write_executable(
        &codex,
        format!(
            "#!/bin/sh\n[ \"$1\" = login ] && exit 0\nexec python3 '{}'\n",
            py.display()
        ),
    );
    let opencode = temp.path().join("opencode");
    crate::test_support::write_executable(
        &opencode,
        include_str!("../../../opencode/tests/fixtures/managed_server.py"),
    );
    let mut providers = ProviderRuntime::default();
    providers.codex = crate::codex::CodexProvider::with_executable(
        Arc::clone(&providers.processes),
        providers.approvals.clone(),
        providers.questions.clone(),
        codex,
    );
    providers.opencode = crate::opencode::OpenCodeProvider::with_executable(
        Arc::clone(&providers.processes),
        providers.approvals.clone(),
        providers.questions.clone(),
        opencode,
    );
    let (tx, rx) = mpsc::sync_channel(128);
    let wake = tx.clone();
    let rt = Runtime::with_providers(providers.clone(), move || {
        let _ = wake.try_send(Action::Wake);
    });
    let input = tx.clone();
    let remote = root.clone();
    let handle =
        thread::spawn(move || dispatch(remote, "example/project".into(), providers, rt, input, rx));
    let mut first = attach_test(&tx);
    assert!(
        matches!(read_frame(&mut first).unwrap(),ServerFrame::Ready { root: r, .. } if r==root)
    );
    let scope = |provider| Scope {
        project: root.clone(),
        provider,
    };
    let (oc, events) = request(&mut first, 1, Op::List(scope(Provider::OpenCode)));
    assert!(matches!(oc,Ok(Reply::Sessions(s)) if s[0].key.id=="ses_fake"));
    if !events
        .iter()
        .any(|e| matches!(e,Event::OpenCode(e) if e.event_type=="session.text.delta"))
    {
        while !matches!(event(&mut first),Event::OpenCode(e) if e.event_type=="session.text.delta")
        {
        }
    }
    let (file, _) = request(
        &mut first,
        2,
        Op::Task(Task::File {
            project: root.clone(),
            path: "remote.txt".into(),
        }),
    );
    assert!(matches!(file,Ok(Reply::Task(TaskReply::File(f))) if f.content=="from the VM"));
    assert!(request(&mut first, 3, Op::Open("/other-checkout".into()))
        .0
        .is_err());
    let (created, _) = request(
        &mut first,
        4,
        Op::Start(scope(Provider::Codex), Options::default()),
    );
    let Ok(Reply::Session(s)) = created else {
        panic!("native session was not created")
    };
    let key = s.key;
    let (sent, events) = request(
        &mut first,
        5,
        Op::Send(key.clone(), "Run once".into(), Options::default()),
    );
    assert!(matches!(sent, Ok(Reply::Done)));
    if !events
        .iter()
        .any(|e| matches!(e,Event::Codex(e) if e.method=="item/agentMessage/delta"))
    {
        while !matches!(event(&mut first),Event::Codex(e) if e.method=="item/agentMessage/delta") {}
    }
    let path = Path::new(&root).join("turn-request.json");
    fs::rename(&path, Path::new(&root).join("acknowledged")).unwrap();
    first.shutdown(Shutdown::Both).unwrap();
    // Deliver the observed EOF before the next client attaches.
    tx.send(Action::Closed(1)).unwrap();
    let mut next = attach_test(&tx);
    assert!(matches!(
        read_frame(&mut next).unwrap(),
        ServerFrame::Ready { .. }
    ));
    let (resumed, _) = request(&mut next, 1, Op::Resume(key.clone()));
    assert!(matches!(resumed,Ok(Reply::Session(s)) if s.key==key));
    let (recovered, _) = request(
        &mut next,
        2,
        Op::Resume(SessionKey {
            scope: scope(Provider::OpenCode),
            id: "ses_fake".into(),
        }),
    );
    assert!(matches!(recovered,Ok(Reply::Resumed(_,p)) if !p.data.is_empty()));
    assert!(!path.exists(), "reconnecting replayed the Codex prompt");
    tx.send(Action::Shutdown).unwrap();
    drop(next);
    handle.join().unwrap().unwrap();
}

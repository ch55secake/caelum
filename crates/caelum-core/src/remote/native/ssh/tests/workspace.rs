use super::super::{connect, Endpoint};
use crate::{
    application::{Command, Event, Options, Provider, Reply, Scope, SessionKey},
    remote::native::{Client, Notice, Status},
};
use caelum_protocol::{ApprovalDecision, ApprovalPrompt, QuestionPrompt};
use std::{
    collections::VecDeque,
    fs,
    path::PathBuf,
    sync::mpsc,
    time::{Duration, Instant},
};

struct Desktop {
    client: Client,
    wake: mpsc::Receiver<()>,
    notices: VecDeque<Notice>,
    approvals: Vec<ApprovalPrompt>,
    questions: Vec<QuestionPrompt>,
    id: u64,
}

impl Desktop {
    #[track_caller]
    fn until<T>(&mut self, mut accept: impl FnMut(Notice) -> Option<T>) -> T {
        let deadline = Instant::now() + Duration::from_secs(25);
        loop {
            self.notices.extend(self.client.drain());
            while let Some(n) = self.notices.pop_front() {
                if let Notice::Event(e) = &n {
                    match e.as_ref() {
                        Event::Approvals(a) => self.approvals.clone_from(a),
                        Event::Questions(q) => self.questions.clone_from(q),
                        _ => {}
                    }
                }
                if let Some(value) = accept(n) {
                    return value;
                }
            }
            self.wake
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .expect("timed out waiting for the SSH fixture");
        }
    }
    #[track_caller]
    fn connected(&mut self) {
        self.until(|n| match n {
            Notice::Status(Status::Connected) => Some(()),
            Notice::Status(Status::Disconnected(e)) => panic!("SSH connection failed: {e}"),
            _ => None,
        })
    }
    #[track_caller]
    fn request(&mut self, cmd: Command) -> Reply {
        self.id += 1;
        let id = self.id;
        self.client.submit(id, cmd).unwrap();
        self.until(|n| {
            if let Notice::Event(e) = n {
                if let Event::Completed(i, r) = *e {
                    assert_eq!(i, id);
                    return Some(r.unwrap());
                }
                None
            } else {
                None
            }
        })
    }
}

fn attach() -> (Desktop, PathBuf, u32) {
    let fixture =
        PathBuf::from(std::env::var_os("CAELUM_NATIVE_SSH_FIXTURE").expect("SSH fixture"));
    let root = fixture.join("project").to_str().unwrap().to_owned();
    let repo = "example/project";
    let (wake, events) = mpsc::channel();
    let (children, pids) = mpsc::channel();
    let directory = fixture.clone();
    let mut attempts = 0;
    let client = Client::with_tunnel(
        root.clone(),
        repo.into(),
        move || {
            let _ = wake.send(());
        },
        async move || {
            attempts += 1;
            let mut endpoint: boat_sdk::access::models::SshKeyResponse =
                serde_json::from_slice(&fs::read(directory.join("endpoint.json")).unwrap())
                    .unwrap();
            if attempts == 1 {
                endpoint.host_key = Some(
                    fs::read_to_string(directory.join("client.pub"))
                        .unwrap()
                        .trim()
                        .into(),
                );
            }
            let tunnel = connect(directory.join("client"), Endpoint::new(endpoint)?, repo)?;
            let _ = children.send(tunnel.child.id().unwrap());
            Ok(tunnel)
        },
    )
    .unwrap();
    let mut desktop = Desktop {
        client,
        wake: events,
        notices: VecDeque::new(),
        approvals: vec![],
        questions: vec![],
        id: 0,
    };
    desktop.until(|n| matches!(n, Notice::Status(Status::Disconnected(_))).then_some(()));
    pids.recv().unwrap();
    desktop.client.reconnect();
    desktop.connected();
    let tunnel = pids.recv().unwrap();
    (desktop, fixture, tunnel)
}

#[test]
#[ignore = "run scripts/dev/smoke_native_workspace.py with the loopback SSH fixture"]
fn native_workspace_over_openssh() {
    let (mut desktop, fixture, tunnel) = attach();
    let root = fixture.join("project").to_str().unwrap().to_owned();
    let scope = |provider| Scope {
        project: root.clone(),
        provider,
    };
    let Reply::Sessions(mut list) = desktop.request(Command::List(scope(Provider::OpenCode)))
    else {
        panic!("no session list")
    };
    let key = list.remove(0).key;
    assert_eq!(key.id, "ses_fake");
    assert!(matches!(
        desktop.request(Command::Choices(key.scope.clone())),
        Reply::Choices(_)
    ));
    assert!(
        matches!(desktop.request(Command::Resume(key.clone())),Reply::Resumed(_,page) if !page.data.is_empty())
    );
    let path = fixture.join("project/requests.jsonl");
    assert!(
        !path.exists(),
        "opening or selecting a session sent a provider mutation"
    );
    let options = Options {
        model: Some("local/model/a".into()),
        effort: Some("high".into()),
        agent: Some("plan".into()),
    };
    desktop.id += 1;
    desktop
        .client
        .submit(
            desktop.id,
            Command::Send(key.clone(), "SSH smoke turn".into(), options),
        )
        .unwrap();
    desktop.until(|n|matches!(n,Notice::Event(e) if matches!(*e,Event::OpenCode(ref e) if e.data["delta"]=="SSH smoke answer")).then_some(()));
    rustix::process::kill_process(
        rustix::process::Pid::from_raw(tunnel as i32).unwrap(),
        rustix::process::Signal::KILL,
    )
    .unwrap();
    desktop.until(|n| matches!(n, Notice::Status(Status::Disconnected(_))).then_some(()));
    desktop.connected();
    let form = desktop
        .questions
        .iter()
        .find(|q| q.request_id == "frm_saved" && q.session_id == key.id)
        .expect("restored question")
        .clone();
    let approval = desktop
        .approvals
        .iter()
        .find(|a| a.session_id.as_ref() == Some(&key.id))
        .expect("restored permission")
        .clone();
    let Reply::Resumed(summary, page) = desktop.request(Command::Resume(key.clone())) else {
        panic!("no recovered history")
    };
    assert_eq!(summary.key, key);
    assert!(page.data.iter().any(|m| m["text"] == "SSH smoke turn"));
    assert!(page
        .data
        .iter()
        .any(|m| m["content"][0]["text"] == "SSH smoke answer"));
    assert!(matches!(
        desktop.request(Command::Answer(
            form,
            Some(
                serde_json::from_value(serde_json::json!({"q0":["macOS","custom target"]}))
                    .unwrap()
            )
        )),
        Reply::Done
    ));
    assert!(matches!(
        desktop.request(Command::Approve(approval, ApprovalDecision::AllowOnce)),
        Reply::Done
    ));
    assert_eq!(
        fs::read_to_string(path)
            .unwrap()
            .lines()
            .filter(|l| l.contains("/prompt\""))
            .count(),
        1
    );
}

#[test]
#[ignore = "run scripts/dev/smoke_native_workspace.py --real-opencode BINARY"]
fn native_new_session_over_openssh() {
    let (mut desktop, fixture, _) = attach();
    let scope = Scope {
        project: fixture.join("project").to_str().unwrap().into(),
        provider: Provider::OpenCode,
    };
    let Reply::Sessions(before) = desktop.request(Command::List(scope.clone())) else {
        panic!("could not list sessions")
    };
    assert!(
        before.is_empty(),
        "isolated VM checkout already contained sessions"
    );
    let Reply::Session(s) = desktop.request(Command::Start(scope, Options::default())) else {
        panic!("could not create OpenCode session")
    };
    fs::write(
        fixture.join("native-session.json"),
        serde_json::to_vec(&s.key).unwrap(),
    )
    .unwrap();
}

#[test]
#[ignore = "run scripts/dev/smoke_native_workspace.py --real-opencode BINARY"]
fn native_resume_after_vm_stop() {
    let (mut desktop, fixture, _) = attach();
    let key: SessionKey =
        serde_json::from_slice(&fs::read(fixture.join("native-session.json")).unwrap()).unwrap();
    assert!(
        matches!(desktop.request(Command::List(key.scope.clone())),Reply::Sessions(s) if s.len()==1 && s[0].key==key)
    );
    assert!(
        matches!(desktop.request(Command::Resume(key.clone())),Reply::Resumed(s,p) if s.key==key && p.data.is_empty())
    );
}

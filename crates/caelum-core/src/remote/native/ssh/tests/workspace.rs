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
    tunnels: mpsc::Receiver<u32>,
}

impl Desktop {
    fn interrupt_link(&mut self) {
        let tunnel = self.tunnels.recv_timeout(Duration::from_secs(5)).unwrap();
        rustix::process::kill_process(
            rustix::process::Pid::from_raw(tunnel as i32).unwrap(),
            rustix::process::Signal::KILL,
        )
        .unwrap();
        self.until(|n| matches!(n, Notice::Status(Status::Disconnected(_))).then_some(()));
        self.connected();
    }
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

fn attach() -> (Desktop, PathBuf) {
    let fixture =
        PathBuf::from(std::env::var_os("CAELUM_NATIVE_SSH_FIXTURE").expect("SSH fixture"));
    let root = std::env::var("CAELUM_NATIVE_SSH_ROOT").unwrap();
    let repo = "example/project";
    let (wake, events) = mpsc::channel();
    let (children, pids) = mpsc::channel();
    let project = PathBuf::from(&root);
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
                serde_json::from_slice(&fs::read(fixture.join("endpoint.json")).unwrap()).unwrap();
            if attempts == 1 {
                endpoint.host_key = Some(
                    fs::read_to_string(fixture.join("client.pub"))
                        .unwrap()
                        .trim()
                        .into(),
                );
            }
            let tunnel = connect(
                fixture.join("client"),
                Endpoint::new(endpoint)?,
                repo,
                &root,
            )?;
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
        tunnels: pids,
    };
    desktop.until(|n| matches!(n, Notice::Status(Status::Disconnected(_))).then_some(()));
    desktop.tunnels.recv().unwrap();
    desktop.client.reconnect();
    desktop.connected();
    (desktop, project)
}

#[test]
#[ignore = "run scripts/dev/smoke_native_workspace.py with the loopback SSH fixture"]
fn native_workspace_over_openssh() {
    let (mut desktop, fixture) = attach();
    let root = fixture.to_str().unwrap().to_owned();
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
    let path = fixture.join("requests.jsonl");
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
    desktop.interrupt_link();
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
    parity::codex(&mut desktop, scope(Provider::Codex));
    parity::checkout(&mut desktop, &key);
}

mod parity;

#[test]
#[ignore = "run scripts/dev/smoke_native_workspace.py --real-opencode BINARY"]
fn native_new_session_over_openssh() {
    let (mut desktop, fixture) = attach();
    let provider = if std::env::var("CAELUM_NATIVE_SMOKE_PROVIDER").as_deref() == Ok("codex") {
        Provider::Codex
    } else {
        Provider::OpenCode
    };
    let scope = Scope {
        project: fixture.to_str().unwrap().into(),
        provider,
    };
    let Reply::Sessions(before) = desktop.request(Command::List(scope.clone())) else {
        panic!("could not list sessions")
    };
    assert!(
        before.is_empty(),
        "isolated VM checkout already contained sessions"
    );
    let Reply::Session(s) = desktop.request(Command::Start(scope, Options::default())) else {
        panic!("could not create a native session")
    };
    {
        desktop.id += 1;
        desktop
            .client
            .submit(
                desktop.id,
                Command::Send(
                    s.key.clone(),
                    "Confirm the checkout.".into(),
                    Options::default(),
                ),
            )
            .unwrap();
        let mut text = String::new();
        desktop.until(|n| match n {
            Notice::Event(e) => match *e {
                Event::OpenCode(e) if e.event_type == "session.text.delta" => {
                    text.push_str(e.data["delta"].as_str().unwrap());
                    None
                }
                Event::Codex(e) if e.method == "item/agentMessage/delta" => {
                    text.push_str(e.params["delta"].as_str().unwrap());
                    None
                }
                Event::OpenCode(e) if e.event_type == "session.execution.succeeded" => Some(()),
                Event::OpenCode(e) if e.event_type == "session.execution.failed" => {
                    panic!("OpenCode turn failed: {}", e.data)
                }
                Event::Codex(e) if e.method == "turn/completed" => {
                    assert_eq!(
                        e.params["turn"]["status"], "completed",
                        "{:?}",
                        e.params["turn"]["error"]
                    );
                    Some(())
                }
                Event::Completed(_, Err(e)) => panic!("Native send failed: {e}"),
                _ => None,
            },
            _ => None,
        });
        assert!(
            text.contains("Confirmed in the VM checkout."),
            "native text was not streamed"
        );
    }
    fs::write(
        fixture.join("native-session.json"),
        serde_json::to_vec(&s.key).unwrap(),
    )
    .unwrap();
}

#[test]
#[ignore = "run scripts/dev/smoke_native_workspace.py --real-opencode BINARY"]
fn native_resume_after_vm_stop() {
    let (mut desktop, fixture) = attach();
    let key: SessionKey =
        serde_json::from_slice(&fs::read(fixture.join("native-session.json")).unwrap()).unwrap();
    assert!(
        matches!(desktop.request(Command::List(key.scope.clone())),Reply::Sessions(s) if s.len()==1 && s[0].key==key)
    );
    match desktop.request(Command::Resume(key.clone())) {
        Reply::Resumed(s, p) => {
            assert_eq!(s.key, key);
            assert!(p
                .data
                .iter()
                .any(|m| m["content"].as_array().is_some_and(|parts| parts
                    .iter()
                    .any(|part| part["text"] == "Confirmed in the VM checkout."))));
        }
        Reply::Session(s) if key.scope.provider == Provider::Codex => {
            assert_eq!(s.key, key);
            let Reply::History(p) = desktop.request(Command::History(key, None)) else {
                panic!("missing Codex history")
            };
            assert!(p
                .data
                .iter()
                .any(|turn| turn["items"].as_array().is_some_and(|items| items
                    .iter()
                    .any(|item| item["type"] == "agentMessage"
                        && item["text"] == "Confirmed in the VM checkout."))));
        }
        _ => panic!("cold resume did not recover the native session"),
    }
}

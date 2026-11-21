use super::*;
use crate::application::{Options, Provider, Reply, Scope, SessionKey};
use std::{io::Read, os::unix::net::UnixStream, sync::mpsc as std_mpsc, time::Instant};

const ROOT: &str = "/home/user/project";
const REPO: &str = "example/project";
const TIMEOUT: Duration = Duration::from_secs(6);

fn until(client: &mut Client, wake: &std_mpsc::Receiver<()>, mut seen: impl FnMut(Notice) -> bool) {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let mut matched = false;
        for notice in client.drain() {
            matched |= seen(notice);
        }
        if matched {
            return;
        }
        wake.recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .unwrap();
    }
}

fn hello(peer: &mut UnixStream, repository: &str) {
    peer.set_read_timeout(Some(TIMEOUT)).unwrap();
    protocol::write_frame(
        peer,
        &ServerFrame::Ready {
            version: VERSION,
            root: ROOT.into(),
            repository: repository.into(),
        },
    )
    .unwrap();
    if repository == REPO {
        for e in [Event::Approvals(vec![]), Event::Questions(vec![])] {
            protocol::write_frame(peer, &ServerFrame::from(e)).unwrap();
        }
    }
}

#[test]
fn a_new_connection_cannot_consume_an_old_reply_or_replay_an_uncertain_send() {
    let (connections, peers) = std_mpsc::channel();
    let (wake, wakes) = std_mpsc::channel();
    let mut client = Client::with_connection(
        ROOT.into(),
        move || {
            let _ = wake.send(());
        },
        async move |link, out| {
            let (local, peer) = UnixStream::pair().unwrap();
            local.set_nonblocking(true).unwrap();
            connections.send(peer).unwrap();
            let mut io = tokio::net::UnixStream::from_std(local).unwrap();
            let (mut input, mut output) = io.split();
            session(ROOT, REPO, link, out, &mut input, &mut output).await
        },
    )
    .unwrap();
    let connected = |n| matches!(n, Notice::Status(Status::Connected));
    let key = SessionKey {
        scope: Scope {
            project: ROOT.into(),
            provider: Provider::OpenCode,
        },
        id: "ses_example".into(),
    };
    let send = || Command::Send(key.clone(), "Only once".into(), Options::default());

    let mut wrong = peers.recv_timeout(TIMEOUT).unwrap();
    hello(&mut wrong, "another/project");
    until(
        &mut client,
        &wakes,
        |n| matches!(n,Notice::Status(Status::Disconnected(e)) if e.contains("different")),
    );
    assert!(client.submit(7, send()).is_err());
    assert_eq!(wrong.read(&mut [0]).unwrap(), 0);
    client.reconnect();
    let mut peer = peers.recv_timeout(TIMEOUT).unwrap();
    hello(&mut peer, REPO);
    until(&mut client, &wakes, connected);
    client.submit(7, send()).unwrap();
    assert!(
        matches!(protocol::read_frame(&mut peer).unwrap(),CommandFrame(7,Command::Send(k,_,_)) if k==key)
    );
    drop(peer); // The provider received the prompt, but the desktop never received its reply.
    until(
        &mut client,
        &wakes,
        |n| matches!(n,Notice::Event(e) if matches!(*e,Event::Completed(7,Err(ref e)) if e.contains("unconfirmed"))),
    );
    assert!(client.submit(7, send()).is_err());
    let mut next = peers.recv_timeout(TIMEOUT).unwrap();
    hello(&mut next, REPO);
    until(&mut client, &wakes, connected);
    client.submit(8, Command::Resume(key.clone())).unwrap();
    assert!(
        matches!(protocol::read_frame(&mut next).unwrap(),CommandFrame(8,Command::Resume(k)) if k==key)
    );
    for id in [7, 8] {
        protocol::write_frame(
            &mut next,
            &ServerFrame::from(Event::Completed(id, Ok(Reply::Done))),
        )
        .unwrap();
    }
    until(&mut client, &wakes, |n| {
        let Notice::Event(event) = n else {
            return false;
        };
        assert!(matches!(*event, Event::Completed(8, Ok(Reply::Done))));
        true
    });
    drop(client);
    assert_eq!(next.read(&mut [0]).unwrap(), 0);
}

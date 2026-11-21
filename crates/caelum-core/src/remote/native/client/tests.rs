use super::*;
use crate::application::{Options, Provider, Scope, SessionKey};
use std::sync::mpsc as blocking;

#[test]
fn losing_a_link_retires_the_send_and_only_admits_new_work_after_its_connected_notice() {
    let (attempt, rx) = blocking::channel();
    let (wake, notices) = blocking::channel();
    let mut client = Client::with_connection(
        "/vm".into(),
        move || {
            let _ = wake.send(());
        },
        async move |link, out| {
            let (lost, signal) = tokio::sync::oneshot::channel::<()>();
            let mut commands = activate(link);
            let (sent, observed) = blocking::channel();
            attempt.send((lost, observed)).unwrap();
            out.publish(Message::Connected(link.clone())).await;
            tokio::pin!(signal);
            loop {
                tokio::select! {
                    _=&mut signal=>return "tunnel closed".into(),
                    cmd=commands.recv()=>if let Some(cmd)=cmd{sent.send(cmd).unwrap()}
                }
            }
        },
    )
    .unwrap();
    let wait = |client: &mut Client, accept: fn(&Notice) -> bool| loop {
        if client.drain().iter().any(accept) {
            return;
        }
        notices.recv_timeout(Duration::from_secs(5)).unwrap();
    };
    let connected = |n: &Notice| matches!(n, Notice::Status(Status::Connected));
    let (lost, first) = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    wait(&mut client, connected);
    let key = SessionKey {
        scope: Scope {
            project: "/vm".into(),
            provider: Provider::Codex,
        },
        id: "thread".into(),
    };
    client
        .submit(
            1,
            Command::Send(key.clone(), "Once".into(), Options::default()),
        )
        .unwrap();
    assert!(matches!(first.recv().unwrap(), (1, Command::Send(..))));
    lost.send(()).unwrap();
    wait(
        &mut client,
        |n| matches!(n,Notice::Event(e) if matches!(e.as_ref(),Event::Completed(1,Err(e)) if e.contains("unconfirmed"))),
    );
    let (_lost, second) = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(client.submit(2, Command::Resume(key.clone())).is_err());
    wait(&mut client, connected);
    client.submit(2, Command::Resume(key)).unwrap();
    assert!(matches!(second.recv().unwrap(), (2, Command::Resume(_))));
}

use super::*;

#[derive(Clone, Default)]
struct Gate(Arc<(Mutex<bool>, std::sync::Condvar)>);

impl Gate {
    fn wait(&self) {
        let (lock, ready) = &*self.0;
        drop(
            ready
                .wait_while(lock.lock().unwrap(), |open| !*open)
                .unwrap(),
        );
    }

    fn release(&self) {
        let (lock, ready) = &*self.0;
        *lock.lock().unwrap() = true;
        ready.notify_all();
    }
}

struct Release(Gate, Gate);

impl Drop for Release {
    fn drop(&mut self) {
        self.0.release();
        self.1.release();
    }
}

#[test]
fn history_and_controls_progress_while_background_workers_are_occupied() {
    let background = Gate::default();
    let history = Gate::default();
    let (entered, started) = mpsc::channel();
    let (wake, wakes) = mpsc::channel();
    let mut runtime = Runtime::with_executor(
        {
            let background = background.clone();
            let history = history.clone();
            Arc::new(move |command| {
                match command {
                    Command::Details(_) => {
                        entered.send(()).unwrap();
                        background.wait();
                    }
                    Command::History(key, _) if key.id == "blocked" => {
                        entered.send(()).unwrap();
                        history.wait();
                    }
                    _ => {}
                }
                Ok(Reply::Done)
            })
        },
        Arc::new(move || {
            let _ = wake.send(());
        }),
    );
    let release = Release(background, history);
    let key = SessionKey {
        scope: scope(Provider::OpenCode, "/project"),
        id: "selected".into(),
    };
    for id in 0..8 {
        runtime.submit(id, Command::Details(key.clone())).unwrap();
    }
    for _ in 0..8 {
        started.recv_timeout(Duration::from_secs(2)).unwrap();
    }
    let check = (|| {
        for (id, command) in [
            (10, Command::Resume(key.clone())),
            (11, Command::History(key.clone(), None)),
            (12, Command::Inbox(key.clone())),
        ] {
            runtime.submit(id, command).unwrap();
            wakes.recv_timeout(Duration::from_secs(2))?;
            assert!(
                matches!(runtime.drain().as_slice(), [Event::Completed(found, Ok(Reply::Done))] if *found == id)
            );
        }
        for id in 13..15 {
            runtime
                .submit(
                    id,
                    Command::History(
                        SessionKey {
                            id: "blocked".into(),
                            ..key.clone()
                        },
                        None,
                    ),
                )
                .unwrap();
        }
        for _ in 0..2 {
            started.recv_timeout(Duration::from_secs(2))?;
        }
        runtime.submit(15, Command::Cancel(key)).unwrap();
        wakes.recv_timeout(Duration::from_secs(2))?;
        Ok::<_, mpsc::RecvTimeoutError>(runtime.drain())
    })();
    drop(release);
    assert!(matches!(
        check.unwrap().as_slice(),
        [Event::Completed(15, Ok(Reply::Done))]
    ));
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

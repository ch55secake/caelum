use super::*;

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

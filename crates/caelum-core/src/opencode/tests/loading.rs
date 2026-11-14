use super::*;

struct Service {
    child: std::process::Child,
    lines: Receiver<String>,
    url: String,
    directory: TestDirectory,
}

impl Service {
    fn new() -> Self {
        let directory = TestDirectory::new();
        for name in ["fast", "slow", "fresh"] {
            fs::create_dir(directory.0.join(name)).unwrap();
        }
        let script = directory.0.join("service.py");
        fs::write(&script, include_str!("fixtures/loading_service.py")).unwrap();
        let mut child = Command::new("python3")
            .arg(script)
            .current_dir(&directory.0)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let (send, lines) = mpsc::channel();
        let output = child.stdout.take().unwrap();
        thread::spawn(move || {
            for line in BufReader::new(output).lines().map_while(Result::ok) {
                if send.send(line).is_err() {
                    break;
                }
            }
        });
        let url = lines.recv_timeout(Duration::from_secs(3)).unwrap();
        fs::write(
            directory.0.join("service.json"),
            json!({"url": url, "pid": child.id(), "version": "2.2.1"}).to_string(),
        )
        .unwrap();
        Self {
            child,
            lines,
            url,
            directory,
        }
    }

    fn provider(&self) -> OpenCodeProvider {
        let mut provider = OpenCodeProvider::new(
            Arc::new(ProcessRegistry::default()),
            ApprovalBroker::default(),
            QuestionBroker::default(),
        );
        Arc::get_mut(&mut provider.inner).unwrap().service_file =
            Some(self.directory.0.join("service.json"));
        provider
    }

    fn project(&self, name: &str) -> Project {
        Project::open(self.directory.0.join(name)).unwrap()
    }

    fn get(&self, route: &str) -> Value {
        Client::new()
            .get(format!("{}{route}", self.url))
            .send()
            .unwrap()
            .json()
            .unwrap()
    }

    fn blocked(&self) {
        assert_eq!(
            self.lines.recv_timeout(Duration::from_secs(3)).unwrap(),
            "blocked"
        );
    }
}

impl Drop for Service {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn slow_project_initialization_does_not_block_existing_or_new_projects() {
    let service = Service::new();
    let provider = service.provider();
    provider.server(&service.project("fast")).unwrap();
    let slow = {
        let provider = provider.clone();
        let project = service.project("slow");
        thread::spawn(move || provider.server(&project))
    };
    service.blocked();
    let same_project = {
        let provider = provider.clone();
        let project = service.project("slow");
        thread::spawn(move || provider.server(&project))
    };
    let (send, replies) = mpsc::channel();
    let readers: Vec<_> = ["fast", "fresh"]
        .into_iter()
        .map(|name| {
            let provider = provider.clone();
            let project = service.project(name);
            let send = send.clone();
            thread::spawn(move || {
                send.send(provider.session_history(project.path(), &format!("ses_{name}"), None))
                    .unwrap();
            })
        })
        .collect();
    let first = replies.recv_timeout(Duration::from_secs(2));
    let second = replies.recv_timeout(Duration::from_secs(2));
    // Always unblock before asserting, including when the regression fails.
    service.get("/test/release");
    for reader in readers {
        reader.join().unwrap();
    }
    let slow = slow.join().unwrap().unwrap();
    assert!(Arc::ptr_eq(&slow, &same_project.join().unwrap().unwrap()));
    for reply in [first, second] {
        let page = reply
            .expect("unrelated history must finish while startup is blocked")
            .unwrap();
        assert_eq!(page.data[0]["text"], "Saved");
        assert_eq!(page.next_cursor.as_deref(), Some("opaque"));
    }
    assert_eq!(service.get("/test/counts")["streams"], 3);
    provider.shutdown();
}

#[test]
fn retired_startup_cannot_replace_a_new_connection_or_restore_stale_questions() {
    for shutdown in [false, true] {
        let service = Service::new();
        let provider = service.provider();
        let project = service.project("slow");
        let old = {
            let provider = provider.clone();
            let project = project.clone();
            thread::spawn(move || provider.server(&project))
        };
        service.blocked();
        let slot = provider
            .inner
            .lock_servers()
            .get(project.path())
            .unwrap()
            .clone();
        let waiter = {
            let processes = provider.inner.processes.clone();
            thread::spawn(move || slot.connect(&processes, |_| panic!("retired slot reconnected")))
        };
        let replacement = if shutdown {
            provider.shutdown();
            assert!(matches!(
                provider.server(&project),
                Err(OpenCodeError::Closed)
            ));
            None
        } else {
            provider.release_checkout(project.path());
            Some(provider.server(&project).unwrap())
        };
        // Waiters are released even though the original form request is blocked.
        assert!(matches!(waiter.join().unwrap(), Err(OpenCodeError::Closed)));
        service.get("/test/release");
        assert!(matches!(old.join().unwrap(), Err(OpenCodeError::Closed)));
        if let Some(replacement) = replacement {
            assert!(Arc::ptr_eq(
                &provider.server(&project).unwrap(),
                &replacement
            ));
            assert!(replacement.alive.load(Ordering::Acquire));
            let questions = provider.inner.questions.pending();
            assert_eq!(questions.len(), 1);
            assert_eq!(questions[0].request_id, "frm_attempt_2");
        } else {
            assert!(provider.inner.questions.pending().is_empty());
            assert!(provider.inner.lock_servers().is_empty());
        }
        provider.shutdown();
    }
}

#[test]
fn failed_and_disconnected_slots_can_retry_but_closed_slots_cannot() {
    let slot = ServerSlot::default();
    let processes = ProcessRegistry::default();
    assert!(slot
        .connect(&processes, |_| Err(OpenCodeError::Closed))
        .is_err());
    let connect = |alive| {
        let mut server = OpenCodeServer::new(None, "http://127.0.0.1:1".into(), None)?;
        server.alive = alive;
        Ok(Arc::new(server))
    };
    let first = slot.connect(&processes, connect).unwrap();
    first.alive.store(false, Ordering::Release);
    let second = slot.connect(&processes, connect).unwrap();
    assert!(!Arc::ptr_eq(&first, &second));
    slot.close(&processes);
    assert!(!second.alive.load(Ordering::Acquire));
    assert!(matches!(
        slot.connect(&processes, connect),
        Err(OpenCodeError::Closed)
    ));
}

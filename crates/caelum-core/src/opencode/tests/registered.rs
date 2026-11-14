use super::*;

#[test]
fn registered_service_streams_external_work_and_survives_provider_shutdown() {
    if Command::new("python3").arg("--version").output().is_err() {
        return;
    }
    struct ExternalServer(std::process::Child);
    impl Drop for ExternalServer {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    for password in [None, Some("shared-secret")] {
        let directory = TestDirectory::new();
        let script = directory.0.join("service.py");
        fs::write(&script, include_str!("fixtures/registered_service.py")).unwrap();
        let mut command = Command::new("python3");
        command
            .arg(script)
            .current_dir(&directory.0)
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        if let Some(password) = password {
            command.env("TEST_PASSWORD", password);
        } else {
            command.env_remove("TEST_PASSWORD");
        }
        let mut external = ExternalServer(command.spawn().unwrap());
        let mut url = String::new();
        BufReader::new(external.0.stdout.take().unwrap())
            .read_line(&mut url)
            .unwrap();
        let url = url.trim();
        let registration = directory.0.join("service.json");
        let mut record =
            json!({ "url": url, "pid": external.0.id(), "version": "2.2.1", "password": password });
        // Stale registrations must not attach to a different process/version.
        record["pid"] = json!(external.0.id() + 1);
        fs::write(&registration, record.to_string()).unwrap();
        assert!(discover_service(&registration).is_none());
        record["pid"] = json!(external.0.id());
        record["version"] = json!("2.0.0");
        fs::write(&registration, record.to_string()).unwrap();
        assert!(discover_service(&registration).is_none());
        record["version"] = json!("2.2.1");
        fs::write(&registration, record.to_string()).unwrap();

        let mut provider = OpenCodeProvider::new(
            Arc::new(ProcessRegistry::default()),
            ApprovalBroker::default(),
            QuestionBroker::default(),
        );
        Arc::get_mut(&mut provider.inner).unwrap().service_file = Some(registration);
        let events = provider.subscribe();
        let sessions = provider.list_sessions(&directory.0).unwrap();
        assert!(sessions.sessions[0].is_working);
        assert_eq!(
            provider
                .session_history(&directory.0, "ses_shared", None)
                .unwrap()
                .is_working,
            Some(true)
        );
        let server = provider
            .inner
            .lock_servers()
            .values()
            .next()
            .unwrap()
            .current()
            .unwrap();
        assert!(server.process_id.is_none());
        let checkout = directory.0.canonicalize().unwrap();
        assert!(provider.ensure_checkout_idle(&checkout).is_err());
        // Trigger the service independently of Caelum's prompt endpoint.
        server
            .request_json(Method::POST, "/external", &[], None)
            .unwrap();
        let event = events.recv_timeout(Duration::from_secs(3)).unwrap();
        assert_eq!(event.event_type, "session.reasoning.delta");
        assert_eq!(event.data["delta"], "External thinking");
        assert!(events.try_recv().is_err());
        // Repeated history polls and resume must agree with the runtime,
        // even though persisted session metadata has only an old idle time.
        for _ in 0..3 {
            assert!(
                provider
                    .resume_session(&directory.0, "ses_shared")
                    .unwrap()
                    .session
                    .is_working
            );
            assert_eq!(
                provider
                    .session_history(&directory.0, "ses_shared", None)
                    .unwrap()
                    .is_working,
                Some(true)
            );
        }
        // An unavailable or malformed activity response must not report idle.
        fs::write(directory.0.join("activity-error"), "").unwrap();
        assert!(matches!(
            provider.session_history(&directory.0, "ses_shared", None),
            Err(OpenCodeError::HttpStatus { status: 503, .. })
        ));
        fs::remove_file(directory.0.join("activity-error")).unwrap();
        fs::write(directory.0.join("activity-invalid"), "").unwrap();
        assert!(matches!(
            provider.list_sessions(&directory.0),
            Err(OpenCodeError::Protocol(_))
        ));
        fs::remove_file(directory.0.join("activity-invalid")).unwrap();
        fs::write(directory.0.join("idle"), "").unwrap();
        assert!(!provider.list_sessions(&directory.0).unwrap().sessions[0].is_working);
        assert_eq!(
            provider
                .session_history(&directory.0, "ses_shared", None)
                .unwrap()
                .is_working,
            Some(false)
        );
        provider.ensure_checkout_idle(&checkout).unwrap();
        provider.release_checkout(&checkout);
        assert!(provider.inner.lock_servers().is_empty());
        assert!(!server.alive.load(Ordering::Acquire));
        provider.shutdown();
        assert!(external.0.try_wait().unwrap().is_none());
        assert!(server
            .request_json(Method::GET, "/api/info", &[], None)
            .is_ok());
    }
}

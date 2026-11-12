use super::*;

#[cfg(unix)]
#[test]
fn title_regeneration_uses_native_patch_checks_ownership_and_streams_the_new_name() {
    if Command::new("python3").arg("--version").output().is_err() {
        return;
    }
    let directory = TestDirectory::new();
    let project = directory.0.join("project");
    fs::create_dir_all(&project).unwrap();
    let executable = directory.0.join("fake-opencode");
    crate::test_support::write_executable(&executable, include_str!("fixtures/managed_server.py"));
    let provider = OpenCodeProvider::with_executable(
        Arc::new(ProcessRegistry::default()),
        ApprovalBroker::default(),
        QuestionBroker::default(),
        &executable,
    );
    let events = provider.subscribe();
    assert!(matches!(
        provider.regenerate_title(&project, "../ses_fake"),
        Err(OpenCodeError::InvalidSessionId)
    ));
    assert!(matches!(
        provider.regenerate_title(&project, "ses_foreign"),
        Err(OpenCodeError::SessionNotInProject)
    ));
    let log = project.join("title-requests.jsonl");
    assert!(!log.exists());

    provider.regenerate_title(&project, "ses_fake").unwrap();
    let request: Value = serde_json::from_str(fs::read_to_string(&log).unwrap().trim()).unwrap();
    assert_eq!(
        request,
        json!({"path":"/api/session/ses_fake","body":{"title":""}})
    );
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let event = events
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .unwrap();
        if event.event_type == "session.renamed" {
            assert_eq!(
                event.project_path,
                project.canonicalize().unwrap().to_string_lossy()
            );
            assert_eq!(
                event.data,
                json!({"sessionID":"ses_fake","title":"Regenerated session"})
            );
            break;
        }
    }
    let session = provider
        .resume_session(&project, "ses_fake")
        .unwrap()
        .session;
    assert_eq!(session.title.as_deref(), Some("Regenerated session"));
    assert_eq!(session.model.as_deref(), Some("local/original"));
    assert_eq!(session.agent.as_deref(), Some("build"));
    // Title generation uses neither the prompt nor transient-generation route.
    assert!(!project.join("requests.jsonl").exists());

    fs::write(project.join("title-error"), "").unwrap();
    assert!(provider.regenerate_title(&project, "ses_fake").is_err());
    provider.shutdown();
}

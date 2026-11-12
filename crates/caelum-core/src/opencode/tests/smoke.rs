use super::*;

#[cfg(unix)]
#[test]
#[ignore = "requires a locally installed OpenCode V2; runs an isolated shell without model inference"]
fn real_background_shell_lifecycle_smoke() {
    let directory = TestDirectory::new();
    let project = Project::open(&directory.0).unwrap();
    let path = project.path().to_string_lossy().into_owned();
    let provider = OpenCodeProvider::with_executable(
        Arc::new(ProcessRegistry::default()),
        ApprovalBroker::default(),
        QuestionBroker::default(),
        find_opencode_executable().unwrap(),
    );
    let session = provider.start_session(&path).unwrap().session.id;
    let server = provider.server(&project).unwrap();
    let query = [("location[directory]", path.clone())];
    let created = server.request_json(Method::POST, "/api/shell", &query,
        Some(json!({"command":"echo caelum-shell-ready; sleep 30", "timeout":45000, "metadata":{"sessionID":session}}))).unwrap();
    let shell = created["data"]["id"].as_str().unwrap();
    assert!(provider
        .background_shells(&path, &session)
        .unwrap()
        .iter()
        .any(|s| s.id == shell));
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        if provider
            .shell_action(&path, &session, shell, false)
            .unwrap()
            .contains("caelum-shell-ready")
        {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "shell output did not arrive"
        );
        thread::sleep(Duration::from_millis(40));
    }
    assert!(provider
        .shell_action(&path, &session, shell, true)
        .unwrap()
        .contains("caelum-shell-ready"));
    assert!(provider
        .background_shells(&path, &session)
        .unwrap()
        .is_empty());
    server
        .request_json(
            Method::PATCH,
            &format!("/api/session/{session}"),
            &[],
            Some(json!({"archived":true})),
        )
        .unwrap();
    provider.shutdown();
}

use super::sessions::file_change_lines;
use super::*;
use caelum_protocol::{ApprovalDecision, LineChanges};
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        loop {
            let path = std::env::temp_dir().join(format!(
                "caelum-codex-test-{}-{}",
                std::process::id(),
                NEXT_DIR.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("create test directory: {error}"),
            }
        }
    }

    fn project(&self) -> PathBuf {
        let project = self.0.join("project");
        fs::create_dir_all(&project).expect("create project directory");
        project
    }

    fn fake_codex(&self, authenticated: bool) -> PathBuf {
        let executable = self.0.join("fake-codex");
        let script = format!(
                "#!/bin/sh\nif [ \"$1 $2\" = \"login status\" ]; then exit {}; fi\nexec python3 -c '{}'\n",
                if authenticated { 0 } else { 1 },
                PYTHON_FAKE_SERVER.replace('\'', "'\\''")
            );
        crate::test_support::write_executable(&executable, script);
        executable
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

const PYTHON_FAKE_SERVER: &str = include_str!("tests/fixtures/server.py");

#[test]
fn session_archiving_is_reversible_scoped_and_keeps_recorded_branch_details() {
    let directory = TestDirectory::new();
    let project_path = directory.project();
    let provider = CodexProvider::with_executable(
        Arc::new(ProcessRegistry::default()),
        ApprovalBroker::default(),
        QuestionBroker::default(),
        directory.fake_codex(true),
    );
    let details = provider
        .session_details(&project_path, "thread-existing")
        .unwrap();
    assert_eq!(details.branch.as_deref(), Some("feature/sidebar"));
    assert!(!details.branch_is_current);
    assert_eq!(
        details.changes,
        Some(LineChanges {
            additions: 4,
            deletions: 1
        })
    );
    fs::write(project_path.join("missing-details"), "").unwrap();
    let details = provider
        .session_details(&project_path, "thread-existing")
        .unwrap();
    assert_eq!(details.branch.as_deref(), Some("feature/sidebar"));
    assert_eq!(details.changes, None);
    assert!(matches!(
        provider.session_details(&project_path, "foreign-thread"),
        Err(CodexError::ThreadNotInProject)
    ));
    for archived in [true, false] {
        assert!(matches!(
            provider.set_session_archived(&project_path, "foreign-thread", archived),
            Err(CodexError::ThreadNotInProject)
        ));
    }
    fs::write(project_path.join("active-thread"), "").unwrap();
    assert!(provider
        .set_session_archived(&project_path, "thread-existing", true)
        .is_err());
    assert_eq!(
        provider.list_sessions(&project_path).unwrap().threads.len(),
        1
    );
    fs::remove_file(project_path.join("active-thread")).unwrap();
    fs::write(project_path.join("archive-error"), "").unwrap();
    assert!(matches!(
        provider.set_session_archived(&project_path, "thread-existing", true),
        Err(CodexError::Rpc(_))
    ));
    fs::remove_file(project_path.join("archive-error")).unwrap();
    provider
        .set_session_archived(&project_path, "thread-existing", true)
        .unwrap();
    assert_eq!(
        provider
            .list_archived_sessions(&project_path)
            .unwrap()
            .threads[0]
            .id,
        "thread-existing"
    );
    assert!(provider
        .list_sessions(&project_path)
        .unwrap()
        .threads
        .is_empty());
    provider
        .set_session_archived(&project_path, "thread-existing", false)
        .unwrap();
    assert_eq!(
        provider.list_sessions(&project_path).unwrap().threads[0].id,
        "thread-existing"
    );
    assert!(matches!(
        provider.delete_session(&project_path, "thread-existing"),
        Err(CodexError::Protocol(message)) if message.contains("only archived")
    ));
    fs::write(project_path.join("active-thread"), "").unwrap();
    assert!(provider
        .delete_session(&project_path, "thread-existing")
        .is_err());
    fs::remove_file(project_path.join("active-thread")).unwrap();
    assert!(matches!(
        provider.delete_session(&project_path, "foreign-thread"),
        Err(CodexError::ThreadNotInProject)
    ));
    provider
        .set_session_archived(&project_path, "thread-existing", true)
        .unwrap();
    fs::write(project_path.join("active-thread"), "").unwrap();
    assert!(provider
        .delete_session(&project_path, "thread-existing")
        .is_err());
    fs::remove_file(project_path.join("active-thread")).unwrap();
    fs::write(project_path.join("delete-error"), "").unwrap();
    assert!(provider
        .delete_session(&project_path, "thread-existing")
        .is_err());
    fs::remove_file(project_path.join("delete-error")).unwrap();
    provider
        .delete_session(&project_path, "thread-existing")
        .unwrap();
    assert!(provider
        .list_sessions(&project_path)
        .unwrap()
        .threads
        .is_empty());
    provider.shutdown();
}

#[test]
fn checkout_cleanup_rejects_active_turns_and_releases_only_its_connections() {
    let directory = TestDirectory::new();
    let project = Project::open(directory.project()).unwrap();
    let nested = project.path().join("src");
    let other = directory.0.join("project-other");
    fs::create_dir_all(&nested).unwrap();
    fs::create_dir_all(&other).unwrap();
    let processes = Arc::new(ProcessRegistry::default());
    let provider = CodexProvider::with_executable(
        Arc::clone(&processes),
        ApprovalBroker::default(),
        QuestionBroker::default(),
        directory.fake_codex(true),
    );
    for path in [project.path(), &nested, &other] {
        provider.list_sessions(path).unwrap();
    }
    fs::write(nested.join("active-thread"), "").unwrap();
    assert!(provider.ensure_checkout_idle(project.path()).is_err());
    fs::remove_file(nested.join("active-thread")).unwrap();
    let connection = provider.connection(&project).unwrap();
    connection.queue_turn("thread-existing");
    assert!(provider.ensure_checkout_idle(project.path()).is_err());
    connection.remove_pending_turn("thread-existing");
    provider.ensure_checkout_idle(project.path()).unwrap();
    let nested_connection = Arc::clone(provider.inner.lock_connections().get(&nested).unwrap());
    nested_connection.alive.store(false, Ordering::Release);
    provider.ensure_checkout_idle(project.path()).unwrap();
    assert_eq!(
        provider
            .inner
            .lock_connections()
            .get(&nested)
            .unwrap()
            .process_id,
        nested_connection.process_id
    );
    provider.release_checkout(project.path());
    assert_eq!(provider.inner.lock_connections().len(), 1);
    for connection in [connection, nested_connection] {
        assert!(matches!(
            processes.terminate(connection.process_id),
            Err(crate::processes::ProcessError::UnknownProcess)
        ));
    }
    provider.list_sessions(&other).unwrap();
    provider.shutdown();
}

#[test]
fn line_counts_handle_patch_headers_context_and_full_file_contents() {
    for (kind, diff, additions, deletions) in [
            ("update", "--- a/file\n+++ b/file\n@@ -1,2 +1,3 @@\n context\n-old\n+++literal\n+\n\\ No newline at end of file\n@@ -9 +10 @@\n-old2\n+new2\n", 3, 2),
            ("add", "first\n\nlast", 3, 0),
            ("delete", "first\nlast\n", 0, 2),
            ("add", "", 0, 0),
            ("update", "Binary files a/file and b/file differ", 0, 0),
        ] {
            assert_eq!(file_change_lines(&json!({ "kind": { "type": kind }, "diff": diff })), LineChanges { additions, deletions });
        }
}

#[test]
fn project_sessions_can_be_listed_started_resumed_and_cancelled() {
    let directory = TestDirectory::new();
    let project_path = directory.project();
    let processes = Arc::new(ProcessRegistry::default());
    let provider = CodexProvider::with_executable(
        Arc::clone(&processes),
        ApprovalBroker::default(),
        QuestionBroker::default(),
        directory.fake_codex(true),
    );

    let sessions = provider.list_sessions(&project_path).unwrap();
    assert_eq!(
        sessions.project.path,
        project_path.canonicalize().unwrap().to_string_lossy()
    );
    assert_eq!(sessions.threads[0].id, "thread-existing");
    assert_eq!(sessions.threads[0].updated_at, Some(123));
    assert!(matches!(
        provider.send_prompt(&project_path, "another-project-thread", "Do work"),
        Err(CodexError::ThreadNotInProject)
    ));

    let created = provider.start_session(&project_path).unwrap();
    assert_eq!(created.thread.id, "thread-new");
    assert_eq!(created.model.as_deref(), Some("default-model"));
    assert_eq!(created.effort.as_deref(), Some("medium"));
    let resumed = provider
        .resume_session(&project_path, "thread-existing")
        .unwrap();
    assert_eq!(resumed.thread.name.as_deref(), Some("Resumed"));
    assert_eq!(resumed.model.as_deref(), Some("saved-model"));
    assert_eq!(resumed.effort.as_deref(), Some("low"));
    let image =
        crate::application::Attachment::image("café.png".into(), b"\x89PNG\r\n\x1a\n").unwrap();
    let prompt = format!(
        "Describe this\n\n<caelum-image-context>\n{}\n</caelum-image-context>",
        json!({"version":1,"images":[image.clone()]})
    );
    provider
        .send_prompt(&project_path, "thread-existing", &prompt)
        .unwrap();
    let body: Value =
        serde_json::from_slice(&fs::read(project_path.join("turn-request.json")).unwrap()).unwrap();
    assert_eq!(body["input"][0]["text"], "Describe this");
    assert_eq!(
        body["input"][1],
        json!({"type":"image","url":image.content})
    );
    let choices = provider.choices(&project_path).unwrap();
    assert_eq!(choices.models[0].id, "test-model");
    assert_eq!(
        choices.models[0]
            .efforts
            .iter()
            .map(|choice| choice.id.as_str())
            .collect::<Vec<_>>(),
        ["low", "medium", "high"]
    );
    assert_eq!(choices.models[0].default_effort.as_deref(), Some("medium"));
    assert_eq!(choices.default_model.as_deref(), Some("configured-model"));
    let history = provider
        .session_history(&project_path, "thread-existing", None)
        .unwrap();
    assert_eq!(history.data[0]["id"], "turn-a");
    assert_eq!(history.data[1]["items"][0]["aggregatedOutput"], "clean");
    let older = provider
        .session_history(
            &project_path,
            "thread-existing",
            history.next_cursor.as_deref(),
        )
        .unwrap();
    assert_eq!(older.data[0]["items"][0]["content"][0]["text"], "Oldest");
    assert_eq!(older.next_cursor, None);
    assert!(matches!(
        provider.session_history(&project_path, "another-project-thread", None),
        Err(CodexError::ThreadNotInProject)
    ));

    let events = provider.subscribe();
    provider
        .send_prompt(&project_path, "thread-existing", "Say hello")
        .unwrap();
    let streamed = events.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(streamed.method, "turn/started");
    let streamed = events.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(streamed.method, "item/agentMessage/delta");
    assert_eq!(streamed.params["delta"], "Hello from Codex");
    let completed = events.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(completed.method, "turn/completed");
    assert_eq!(completed.params["threadId"], "thread-existing");

    provider
        .cancel_turn(&project_path, "thread-existing")
        .unwrap();
    provider
        .send_prompt_with_options(
            &project_path,
            "thread-existing",
            "Use the selected model",
            Some("test-model"),
            Some("high"),
        )
        .unwrap();
    let resumed = provider
        .resume_session(&project_path, "thread-existing")
        .unwrap();
    assert_eq!(resumed.thread.id, "thread-existing");
    assert_eq!(resumed.model.as_deref(), Some("test-model"));
    assert_eq!(resumed.effort.as_deref(), Some("high"));
    provider
        .send_prompt(&project_path, "thread-existing", "Keep this model")
        .unwrap();
    assert_eq!(
        provider
            .resume_session(&project_path, "thread-existing")
            .unwrap()
            .model
            .as_deref(),
        Some("test-model")
    );
    assert_eq!(
        provider
            .resume_session(&project_path, "thread-existing")
            .unwrap()
            .effort
            .as_deref(),
        Some("high")
    );
    provider
        .send_prompt_with_options(
            &project_path,
            "thread-existing",
            "Change only effort",
            None,
            Some("low"),
        )
        .unwrap();
    let resumed = provider
        .resume_session(&project_path, "thread-existing")
        .unwrap();
    assert_eq!(resumed.model.as_deref(), Some("test-model"));
    assert_eq!(resumed.effort.as_deref(), Some("low"));
    assert!(matches!(
        provider.send_prompt(&project_path, "thread-existing", "rpc-error"),
        Err(CodexError::Rpc(message)) if message == "fake request error"
    ));
    provider.shutdown();
    assert!(processes.poll_exited().unwrap().is_empty());
}

#[test]
fn a_crashed_app_server_is_reaped_and_restarted_for_the_project() {
    let directory = TestDirectory::new();
    let project_path = directory.project();
    fs::write(project_path.join("die-after-list"), "").unwrap();
    let provider = CodexProvider::with_executable(
        Arc::new(ProcessRegistry::default()),
        ApprovalBroker::default(),
        QuestionBroker::default(),
        directory.fake_codex(true),
    );
    let events = provider.subscribe();

    provider.list_sessions(&project_path).unwrap();
    loop {
        let event = events.recv_timeout(Duration::from_secs(2)).unwrap();
        if event.method == "codex/processExited" {
            break;
        }
    }

    let session = provider.start_session(&project_path).unwrap();
    assert_eq!(session.thread.id, "thread-new");
    provider.shutdown();
}

#[test]
fn command_approval_is_forwarded_and_resolved_over_jsonl() {
    let directory = TestDirectory::new();
    let project_path = directory.project();
    let approvals = ApprovalBroker::default();
    let updates = approvals.subscribe();
    let _initial_snapshot = updates.recv_timeout(Duration::from_secs(1)).unwrap();
    let provider = CodexProvider::with_executable(
        Arc::new(ProcessRegistry::default()),
        approvals.clone(),
        QuestionBroker::default(),
        directory.fake_codex(true),
    );
    provider.list_sessions(&project_path).unwrap();
    let events = provider.subscribe();

    provider
        .send_prompt(&project_path, "thread-existing", "request-approval")
        .unwrap();
    let prompts = updates.recv_timeout(Duration::from_secs(2)).unwrap();
    let prompt = prompts.first().expect("Codex approval reaches the broker");
    assert_eq!(prompt.provider, PROVIDER_NAME);
    assert_eq!(
        prompt.project_path,
        project_path.canonicalize().unwrap().to_string_lossy()
    );
    assert_eq!(prompt.session_id.as_deref(), Some("thread-existing"));
    assert!(prompt.description.contains("git status"));
    approvals
        .resolve(
            PROVIDER_NAME,
            &prompt.project_path,
            &prompt.request_id,
            ApprovalDecision::AllowOnce,
        )
        .unwrap();

    let resolved = loop {
        let event = events.recv_timeout(Duration::from_secs(2)).unwrap();
        if event.method == "test/approvalResolved" {
            break event;
        }
    };
    assert_eq!(resolved.params["decision"], "accept");
    provider.shutdown();
}

#[test]
fn missing_installation_and_authentication_have_actionable_errors() {
    let directory = TestDirectory::new();
    let project_path = directory.project();
    let processes = Arc::new(ProcessRegistry::default());
    let approvals = ApprovalBroker::default();

    let missing = CodexProvider::with_executable(
        Arc::clone(&processes),
        approvals.clone(),
        QuestionBroker::default(),
        directory.0.join("missing-codex"),
    );
    assert!(matches!(
        missing.list_sessions(&project_path),
        Err(CodexError::ExecutableNotFound(_))
    ));

    let signed_out = CodexProvider::with_executable(
        processes,
        approvals,
        QuestionBroker::default(),
        directory.fake_codex(false),
    );
    assert!(matches!(
        signed_out.list_sessions(&project_path),
        Err(CodexError::NotAuthenticated)
    ));
}

#[test]
fn questions_round_trip_original_rpc_ids_and_clear_on_resolution_or_interrupt() {
    let directory = TestDirectory::new();
    let project = directory.project();
    let questions = QuestionBroker::default();
    let provider = CodexProvider::with_executable(
        Arc::new(ProcessRegistry::default()),
        ApprovalBroker::default(),
        questions.clone(),
        directory.fake_codex(true),
    );
    provider.list_sessions(&project).unwrap();
    let events = provider.subscribe();
    let next = |method: &str| loop {
        let event = events.recv_timeout(Duration::from_secs(3)).unwrap();
        if event.method == method {
            break event;
        }
    };
    provider
        .send_prompt(&project, "thread-existing", "request-question")
        .unwrap();
    assert_eq!(questions.pending().len(), 2);
    let path = project
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let answer = json!({"choice": "Small", "detail": "Keep compatibility"})
        .as_object()
        .unwrap()
        .clone();
    assert!(questions
        .resolve(
            PROVIDER_NAME,
            &path,
            "thread-existing",
            "41",
            Some(serde_json::Map::new())
        )
        .is_err());
    assert_eq!(questions.pending().len(), 2);
    assert_eq!(questions.pending()[0].fields[1]["secret"], true);
    questions
        .resolve(PROVIDER_NAME, &path, "thread-existing", "41", Some(answer))
        .unwrap();
    assert_eq!(
        next("test/questionResolved").params,
        json!({ "id": 41, "result": {"answers": {
            "choice": {"answers": ["Small"]}, "detail": {"answers": ["Keep compatibility"]}
        }}})
    );
    questions
        .resolve(PROVIDER_NAME, &path, "thread-existing", "\"41\"", None)
        .unwrap();
    assert_eq!(
        next("test/questionResolved").params,
        json!({ "id": "41", "result": { "answers": {} }})
    );
    provider
        .send_prompt(&project, "thread-existing", "request-question")
        .unwrap();
    provider
        .send_prompt(&project, "thread-existing", "resolve-question")
        .unwrap();
    assert!(questions.pending().is_empty());
    provider
        .send_prompt(&project, "thread-existing", "request-question")
        .unwrap();
    provider.cancel_turn(&project, "thread-existing").unwrap();
    assert!(questions.pending().is_empty());
    provider
        .send_prompt(&project, "thread-existing", "request-question")
        .unwrap();
    provider.shutdown();
    assert!(questions.pending().is_empty());
}

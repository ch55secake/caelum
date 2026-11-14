use super::*;

#[cfg(unix)]
#[test]
fn managed_server_lists_creates_restores_prompts_streams_and_shuts_down() {
    if Command::new("python3").arg("--version").output().is_err() {
        return;
    }

    let directory = TestDirectory::new();
    let project_path = directory.0.join("project");
    fs::create_dir_all(&project_path).unwrap();
    let executable = directory.0.join("fake-opencode");
    crate::test_support::write_executable(&executable, include_str!("fixtures/managed_server.py"));

    let processes = Arc::new(ProcessRegistry::default());
    let approvals = ApprovalBroker::default();
    let questions = QuestionBroker::default();
    let provider = OpenCodeProvider::with_executable(
        Arc::clone(&processes),
        approvals.clone(),
        questions.clone(),
        &executable,
    );
    let events = provider.subscribe();

    let listed = provider.list_sessions(&project_path).unwrap();
    assert_eq!(listed.sessions[0].id, "ses_fake");
    let path = project_path
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    assert_eq!(questions.pending().len(), 4);
    assert!(questions
        .pending()
        .iter()
        .all(|request| request.project_path == path));
    assert_eq!(questions.pending()[0].fields[0]["type"], "multiselect");
    let answer = json!({"q0": ["macOS", "custom target"]})
        .as_object()
        .unwrap()
        .clone();
    fs::write(project_path.join("fail-form-reply"), "").unwrap();
    assert!(questions
        .resolve(
            PROVIDER_NAME,
            &path,
            "ses_fake",
            "frm_saved",
            Some(answer.clone())
        )
        .is_err());
    assert_eq!(questions.pending().len(), 4);
    fs::remove_file(project_path.join("fail-form-reply")).unwrap();
    questions
        .resolve(PROVIDER_NAME, &path, "ses_fake", "frm_saved", Some(answer))
        .unwrap();
    questions
        .resolve(PROVIDER_NAME, &path, "ses_fake", "frm_live", None)
        .unwrap();
    assert_eq!(questions.pending().len(), 2);
    // Rehydrate forms that predate a UI connection, without replaying SSE.
    questions.clear(PROVIDER_NAME, &path, None);
    provider.refresh_questions(&project_path).unwrap();
    assert_eq!(questions.pending().len(), 2);
    let updates = questions.subscribe();
    fs::write(project_path.join("external-form-resolution"), "").unwrap();
    while !updates
        .recv_timeout(Duration::from_secs(3))
        .unwrap()
        .is_empty()
    {}
    provider.refresh_questions(&project_path).unwrap();
    assert!(questions.pending().is_empty());
    let commands = provider.commands(&project_path).unwrap();
    assert_eq!(commands.len(), 2);
    assert_eq!(commands[0].name, "goal");
    assert_eq!(
        commands[0].description.as_deref(),
        Some("Set or control an objective")
    );
    assert_eq!(commands[1].name, "team/review");
    assert_eq!(commands[1].description, None);
    let limits = json!({
        "workspace": { "id": "workspace", "usageURL": "https://opencode.ai/workspace/usage" },
        "go": {
            "rolling": { "percent": 42, "limited": false, "resetsAt": 1800000000000_u64 },
            "weekly": { "percent": 20, "limited": false, "resetsAt": 1800100000000_u64 },
            "monthly": { "percent": 100, "limited": true, "resetsAt": 1800200000000_u64 }
        },
        "budgets": []
    });
    fs::write(
        project_path.join("usage.json"),
        serde_json::to_vec(&limits).unwrap(),
    )
    .unwrap();
    assert_eq!(provider.usage_limits(&project_path).unwrap(), limits);
    fs::write(project_path.join("usage.json"), "null").unwrap();
    assert!(provider.usage_limits(&project_path).unwrap().is_null());
    let projects = provider.stats_projects(&project_path).unwrap();
    assert_eq!(projects[0].id, "project_fake");
    assert_eq!(projects[0].canonical, path);
    let stats = json!({
        "range": {"from": 1000, "to": 2000}, "sessions": 2, "subagents": 1,
        "prompts": 8, "steps": 12, "tokens": {"input": 100, "output": 50,
            "reasoning": 20, "cache": {"read": 200, "write": 10}},
        "cost": 0.12, "tools": {"mode": "none"}, "activeDays": 1, "streak": 1,
        "activity": [{"date": "2026-11-01", "steps": 12}], "models": []
    });
    fs::write(project_path.join("stats.json"), stats.to_string()).unwrap();
    let filter = caelum_protocol::OpenCodeStatsFilter {
        from: Some(1793491200000),
        to: Some(1793577600000),
        project: Some("project_fake".into()),
        timezone: Some("Europe/London".into()),
    };
    let result = provider.session_stats(&project_path, &filter).unwrap();
    assert_eq!(result.sessions, 2);
    assert_eq!(result.tokens.cache.read, 200.0);
    let query: Value =
        serde_json::from_slice(&fs::read(project_path.join("stats-query.json")).unwrap()).unwrap();
    assert_eq!(
        query,
        json!({"from": ["1793491200000"], "to": ["1793577600000"], "project": ["project_fake"], "timezone": ["Europe/London"], "tools": ["none"]})
    );
    provider
        .session_stats(&project_path, &Default::default())
        .unwrap();
    let query: Value =
        serde_json::from_slice(&fs::read(project_path.join("stats-query.json")).unwrap()).unwrap();
    assert_eq!(query, json!({"tools": ["none"]}));
    assert!(provider
        .session_stats(
            &project_path,
            &caelum_protocol::OpenCodeStatsFilter {
                from: Some(2000),
                to: Some(1000),
                ..Default::default()
            }
        )
        .is_err());
    fs::write(project_path.join("stats.json"), "{}").unwrap();
    assert!(provider
        .session_stats(&project_path, &Default::default())
        .is_err());
    fs::write(project_path.join("command-api-error"), "").unwrap();
    assert!(provider.commands(&project_path).is_err());
    assert!(provider.usage_limits(&project_path).is_err());
    fs::remove_file(project_path.join("command-api-error")).unwrap();
    let choices = provider.choices(&project_path).unwrap();
    assert_eq!(choices.models.len(), 3);
    assert_eq!(choices.models[0].id, "local/model/a");
    assert_eq!(choices.models[1].id, "router/model/a");
    assert_eq!(choices.models[0].context_window, Some(200_000));
    assert_eq!(choices.models[1].context_window, None);
    assert_eq!(choices.models[2].context_window, None);
    assert_eq!(
        choices.models[0].provider.as_ref().unwrap().name,
        "Local gateway"
    );
    assert_eq!(choices.models[1].provider.as_ref().unwrap().id, "router");
    assert_eq!(
        choices.models[1].provider.as_ref().unwrap().name,
        "OpenRouter"
    );
    assert_eq!(choices.models[2].provider.as_ref().unwrap().name, "custom");
    fs::write(project_path.join("no-providers"), "").unwrap();
    let fallback = provider.choices(&project_path).unwrap();
    assert_eq!(fallback.models.len(), 3);
    assert_eq!(fallback.models[0].provider.as_ref().unwrap().name, "local");
    assert_eq!(fallback.models[1].provider.as_ref().unwrap().name, "router");
    assert_eq!(choices.agents.len(), 2);
    assert_eq!(choices.default_model.as_deref(), Some("local/model/a"));
    assert_eq!(
        choices.models[0]
            .efforts
            .iter()
            .map(|choice| choice.id.as_str())
            .collect::<Vec<_>>(),
        ["low", "high"]
    );
    let started = provider
        .start_session_with_options(&project_path, Some(&choices.models[0].id), Some("build"))
        .unwrap();
    assert_eq!(started.session.id, "ses_fake");
    assert_eq!(started.session.model.as_deref(), Some("local/model/a"));
    assert_eq!(started.session.agent.as_deref(), Some("build"));
    assert!(provider
        .read_image(&project_path, &"a".repeat(64))
        .unwrap()
        .starts_with("data:image/png;base64,"));
    assert!(provider.read_image(&project_path, "../info").is_err());
    let image =
        crate::application::Attachment::image("café.png".into(), b"\x89PNG\r\n\x1a\n").unwrap();
    let prompt = format!(
        "Describe this\n\n<caelum-image-context>\n{}\n</caelum-image-context>",
        json!({"version":1,"images":[image.clone()]})
    );
    provider
        .send_prompt(&project_path, "ses_fake", &prompt)
        .unwrap();
    let body: Value =
        serde_json::from_slice(&fs::read(project_path.join("prompt-request.json")).unwrap())
            .unwrap();
    assert_eq!(body["text"], "Describe this");
    assert_eq!(
        body["files"],
        json!([{"uri":image.content,"name":"café.png"}])
    );
    provider
        .enqueue_prompt(&project_path, "ses_fake", "msg_image", &prompt)
        .unwrap();
    let body: Value =
        serde_json::from_slice(&fs::read(project_path.join("prompt-request.json")).unwrap())
            .unwrap();
    assert_eq!(body["delivery"], "queue");
    assert_eq!(body["text"], "Describe this");
    assert_eq!(body["files"][0]["uri"], image.content);
    provider
        .cancel_queued_prompt(&project_path, "ses_fake", "msg_image")
        .unwrap();
    provider
        .enqueue_prompt(&project_path, "ses_fake", "msg_queue", "Next task")
        .unwrap();
    provider
        .enqueue_prompt(&project_path, "ses_fake", "msg_queue", "Next task")
        .unwrap();
    let inbox = provider.inbox(&project_path, "ses_fake").unwrap();
    assert_eq!(inbox.len(), 1);
    assert_eq!(inbox[0]["delivery"], "queue");
    assert_eq!(inbox[0]["payload"]["text"], "Next task");
    assert!(provider
        .enqueue_prompt(&project_path, "ses_foreign", "msg_queue", "Wrong checkout")
        .is_err());
    assert!(provider
        .cancel_queued_prompt(&project_path, "ses_fake", "../escape")
        .is_err());
    provider
        .cancel_queued_prompt(&project_path, "ses_fake", "msg_queue")
        .unwrap();
    assert!(provider
        .inbox(&project_path, "ses_fake")
        .unwrap()
        .is_empty());
    let jobs = project_path.join("background-shells.json");
    fs::write(&jobs, serde_json::to_vec(&json!({ "data": [
        { "id": "sh_live", "metadata": { "sessionID": "ses_fake" }, "status": "running", "command": "cargo test", "time": { "started": 1234 } },
        { "id": "sh_foreign", "metadata": { "sessionID": "ses_foreign" }, "status": "running" },
        { "metadata": {}, "status": "running" },
        { "metadata": { "sessionID": "ses_fake" }, "status": "exited" },
        { "metadata": { "sessionID": "ses_fake" }, "status": "timeout" },
        { "metadata": { "sessionID": "ses_fake" }, "status": "killed" }
    ] })).unwrap()).unwrap();
    let shells = provider
        .background_shells(&project_path, "ses_fake")
        .unwrap();
    assert_eq!(shells.len(), 1);
    assert_eq!(shells[0].id, "sh_live");
    assert_eq!(shells[0].command, "cargo test");
    assert_eq!(shells[0].started_at, 1234);
    for stop in [false, true] {
        assert!(provider
            .shell_action(&path, "ses_fake", "sh_foreign", stop)
            .is_err());
        assert!(provider
            .shell_action(&path, "ses_foreign", "sh_live", stop)
            .is_err());
        assert!(provider
            .shell_action(&path, "ses_fake", "../escape", stop)
            .is_err());
    }
    assert!(
        !project_path.join("shell-reads.jsonl").exists(),
        "foreign shells must never be read"
    );
    let output = provider
        .shell_action(&path, "ses_fake", "sh_live", false)
        .unwrap();
    assert_eq!(output.len(), 12000);
    assert!(output.ends_with("latest café output"));
    assert_eq!(
        fs::read_to_string(project_path.join("shell-reads.jsonl"))
            .unwrap()
            .lines()
            .count(),
        2
    );
    assert_eq!(
        provider
            .shell_action(&path, "ses_fake", "sh_live", true)
            .unwrap(),
        output
    );
    assert!(provider
        .background_shells(&path, "ses_fake")
        .unwrap()
        .is_empty());
    assert!(provider
        .shell_action(&path, "ses_fake", "sh_live", false)
        .is_err());
    let remaining: Value = serde_json::from_slice(&fs::read(&jobs).unwrap()).unwrap();
    assert!(remaining["data"]
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s["id"] == "sh_foreign"));
    assert!(matches!(
        provider.background_shells(&project_path, "ses_foreign"),
        Err(OpenCodeError::SessionNotInProject)
    ));
    fs::write(&jobs, r#"{"data":[]}"#).unwrap();
    assert!(provider
        .background_shells(&project_path, "ses_fake")
        .unwrap()
        .is_empty());
    for invalid in [
        json!({ "unexpected": [] }),
        json!({ "data": [{ "metadata": { "sessionID": "ses_fake" }, "status": "running" }] }),
    ] {
        fs::write(&jobs, serde_json::to_vec(&invalid).unwrap()).unwrap();
        assert!(matches!(
            provider.background_shells(&project_path, "ses_fake"),
            Err(OpenCodeError::Protocol(_))
        ));
    }
    let history = provider
        .session_history(&project_path, "ses_fake", None)
        .unwrap();
    assert_eq!(history.data[0]["text"], "Earlier prompt");
    assert_eq!(history.model.as_deref(), Some("local/model/a"));
    assert_eq!(history.agent.as_deref(), Some("build"));
    assert_eq!(
        history.data[1]["content"][1]["state"]["content"][0]["text"],
        "All passed"
    );
    let older = provider
        .session_history(&project_path, "ses_fake", history.next_cursor.as_deref())
        .unwrap();
    assert_eq!(older.data[0]["text"], "Oldest");
    assert_eq!(older.next_cursor, None);
    provider
        .send_prompt(&project_path, "ses_fake", "Continue")
        .unwrap();
    provider
        .send_prompt_with_options(
            &project_path,
            "ses_fake",
            "Switch model",
            Some("msg_selected"),
            Some("other/model/b"),
            Some("high"),
        )
        .unwrap();
    assert_eq!(
        provider
            .resume_session(&project_path, "ses_fake")
            .unwrap()
            .session
            .model
            .as_deref(),
        Some("other/model/b")
    );
    let requests = || {
        fs::read_to_string(project_path.join("requests.jsonl"))
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .collect::<Vec<_>>()
    };
    let sent = requests();
    let answer = provider
        .generate_from_context(&project_path, "ses_fake", "Side question α\nKeep context")
        .unwrap();
    assert_eq!(answer, "Transient answer: Side question α\nKeep context");
    let generated = requests();
    assert_eq!(generated.len(), sent.len() + 1);
    assert_eq!(
        generated.last().unwrap()["path"],
        "/api/session/ses_fake/generate"
    );
    assert_eq!(
        generated.last().unwrap()["body"],
        json!({"prompt": "Side question α\nKeep context"})
    );
    assert!(provider
        .generate_from_context(&project_path, "ses_foreign", "Wrong checkout")
        .is_err());
    assert!(provider
        .generate_from_context(&project_path, "../ses_fake", "Wrong ID")
        .is_err());
    assert!(provider
        .generate_from_context(&project_path, "ses_fake", " \n ")
        .is_err());
    assert_eq!(requests().len(), generated.len());
    fs::write(project_path.join("generate-error"), "").unwrap();
    assert!(provider
        .generate_from_context(&project_path, "ses_fake", "Retry later")
        .is_err());
    fs::remove_file(project_path.join("generate-error")).unwrap();
    fs::write(project_path.join("generate-malformed"), "").unwrap();
    assert!(provider
        .generate_from_context(&project_path, "ses_fake", "Malformed")
        .is_err());
    fs::remove_file(project_path.join("generate-malformed")).unwrap();
    // Keep subsequent model-selection assertions based on the current log.
    let before_rejected_model = requests().len();
    let switch = &sent[sent.len() - 2];
    assert_eq!(switch["path"], "/api/session/ses_fake/model");
    assert_eq!(
        switch["body"],
        json!({ "model": { "providerID": "other", "id": "model/b", "variant": "high" } })
    );
    assert_eq!(
        sent.last().unwrap()["body"],
        json!({ "text": "Switch model", "id": "msg_selected" })
    );
    assert!(provider
        .send_prompt_with_options(
            &project_path,
            "ses_fake",
            "Do not send with a rejected model",
            None,
            Some("other/missing"),
            None,
        )
        .is_err());
    assert_eq!(requests().len(), before_rejected_model + 1);
    assert_eq!(
        requests().last().unwrap()["path"],
        "/api/session/ses_fake/model"
    );
    assert_eq!(
        provider
            .session_history(&project_path, "ses_fake", None)
            .unwrap()
            .effort
            .as_deref(),
        Some("high")
    );
    provider
        .send_prompt_with_options(
            &project_path,
            "ses_fake",
            "Change effort",
            None,
            None,
            Some("low"),
        )
        .unwrap();
    assert_eq!(
        provider
            .resume_session(&project_path, "ses_fake")
            .unwrap()
            .session
            .effort
            .as_deref(),
        Some("low")
    );
    let sent = requests();
    assert_eq!(
        sent[sent.len() - 2]["body"],
        json!({ "model": { "providerID": "other", "id": "model/b", "variant": "low" } })
    );
    assert!(provider
        .send_prompt_with_options(
            &project_path,
            "ses_fake",
            "Rejected effort must not send",
            None,
            None,
            Some("invalid")
        )
        .is_err());
    assert_eq!(requests().len(), sent.len() + 1);
    let mut workspace = crate::application::Workspace::default();
    let index = workspace.new_session(crate::application::Scope {
        project: project_path.to_string_lossy().into_owned(),
        provider: crate::application::Provider::OpenCode,
    });
    workspace.sessions[index].remote_id = Some("ses_fake".into());
    workspace.sessions[index].options.model = Some("other/model/b".into());
    workspace.sessions[index].options.effort = Some("low".into());
    workspace.set_effort(index, None);
    assert!(workspace.send(index, "Default effort".into()));
    let (_, command) = workspace.commands().pop().unwrap();
    let crate::application::Command::Send(key, prompt, options) = command else {
        panic!("expected a prompt with the explicit effort reset");
    };
    provider
        .send_prompt_with_options(
            &key.scope.project,
            &key.id,
            &prompt,
            None,
            options.model.as_deref(),
            options.effort.as_deref(),
        )
        .unwrap();
    let sent = requests();
    assert_eq!(
        sent[sent.len() - 2]["body"],
        json!({ "model": { "providerID": "other", "id": "model/b" } })
    );
    assert_eq!(
        provider
            .resume_session(&project_path, "ses_fake")
            .unwrap()
            .session
            .effort
            .as_deref(),
        Some("")
    );
    provider
        .select_agent(&project_path, "ses_fake", "plan")
        .unwrap();
    assert_eq!(
        requests().last().unwrap(),
        &json!({
            "path": "/api/session/ses_fake/agent", "body": { "agent": "plan" }
        })
    );
    let resumed = provider.resume_session(&project_path, "ses_fake").unwrap();
    assert_eq!(resumed.session.agent.as_deref(), Some("plan"));
    assert_eq!(resumed.session.model.as_deref(), Some("other/model/b"));
    assert_eq!(resumed.session.effort.as_deref(), Some(""));
    assert_eq!(
        provider
            .session_history(&project_path, "ses_fake", None)
            .unwrap()
            .agent
            .as_deref(),
        Some("plan")
    );
    let count = requests().len();
    provider
        .select_agent(&project_path, "ses_fake", "plan")
        .unwrap();
    assert!(matches!(
        provider.select_agent(&project_path, "ses_foreign", "build"),
        Err(OpenCodeError::SessionNotInProject)
    ));
    assert!(provider
        .select_agent(&project_path, "ses_fake", "")
        .is_err());
    assert_eq!(requests().len(), count);
    assert!(provider
        .select_agent(&project_path, "ses_fake", "missing")
        .is_err());
    assert_eq!(
        provider
            .resume_session(&project_path, "ses_fake")
            .unwrap()
            .session
            .agent
            .as_deref(),
        Some("plan")
    );
    let mut command: OpenCodeCommandInput = serde_json::from_value(json!({
        "name": "team/review", "text": "src/main.rs\nKeep Unicode: 日本語 🚀\n",
        "files": [{ "uri": "data:text/plain;base64,aGVsbG8K", "name": "src/main.rs", "description": "Lines 2–3 of src/main.rs" }]
    })).unwrap();
    let count = requests().len();
    assert!(matches!(
        provider.run_command(
            &project_path,
            "ses_foreign",
            &command,
            Some("local/model/a"),
            Some("low")
        ),
        Err(OpenCodeError::SessionNotInProject)
    ));
    assert_eq!(requests().len(), count);
    assert!(provider
        .run_command(
            &project_path,
            "ses_fake",
            &command,
            Some("other/missing"),
            None
        )
        .is_err());
    assert_eq!(requests().len(), count + 1);
    assert_eq!(
        requests().last().unwrap()["path"],
        "/api/session/ses_fake/model"
    );
    provider
        .run_command(
            &project_path,
            "ses_fake",
            &command,
            Some("local/model/a"),
            Some("low"),
        )
        .unwrap();
    let sent = requests();
    assert_eq!(
        sent[sent.len() - 2]["body"],
        json!({ "model": { "providerID": "local", "id": "model/a", "variant": "low" } })
    );
    assert_eq!(
        sent.last().unwrap(),
        &json!({ "path": "/api/session/ses_fake/command", "body": command })
    );
    command.name = "goal".into();
    command.files.clear();
    fs::write(project_path.join("busy"), "").unwrap();
    let checkout = project_path.canonicalize().unwrap();
    assert!(provider.ensure_checkout_idle(&checkout).is_err());
    for text in [
        "Finish the task\nThen summarize.",
        "pause",
        "resume",
        "clear",
        "",
    ] {
        command.text = text.into();
        let count = requests().len();
        provider
            .run_command(&project_path, "ses_fake", &command, None, None)
            .unwrap();
        assert_eq!(requests().len(), count + 1);
        assert_eq!(
            requests().last().unwrap()["body"],
            json!({ "name": "goal", "text": text, "files": [] })
        );
    }
    fs::remove_file(project_path.join("busy")).unwrap();
    provider.ensure_checkout_idle(&checkout).unwrap();
    command.name = "unknown".into();
    assert!(provider
        .run_command(&project_path, "ses_fake", &command, None, None)
        .is_err());
    command.name = " ".into();
    let count = requests().len();
    assert!(provider
        .run_command(&project_path, "ses_fake", &command, None, None)
        .is_err());
    assert_eq!(requests().len(), count);
    provider.cancel_turn(&project_path, "ses_fake").unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut received_text = false;
    let mut received_permission = false;
    while Instant::now() < deadline && !(received_text && received_permission) {
        let streamed = events.recv_timeout(Duration::from_millis(100)).unwrap();
        if streamed.event_type == "session.text.delta" {
            received_text = streamed.data["delta"] == "hello";
        }
        if streamed.event_type == "permission.asked" {
            received_permission = true;
        }
    }
    assert!(received_text);
    assert!(received_permission);
    let prompt_deadline = Instant::now() + Duration::from_secs(3);
    while approvals.pending().is_empty() && Instant::now() < prompt_deadline {
        thread::sleep(Duration::from_millis(10));
    }
    let prompt = approvals
        .pending()
        .pop()
        .expect("permission prompt is pending");
    assert_eq!(prompt.provider, PROVIDER_NAME);
    assert_eq!(
        prompt.project_path,
        project_path.canonicalize().unwrap().to_string_lossy()
    );
    assert_eq!(prompt.session_id.as_deref(), Some("ses_fake"));
    approvals
        .resolve(
            PROVIDER_NAME,
            &prompt.project_path,
            &prompt.request_id,
            ApprovalDecision::AllowOnce,
        )
        .unwrap();
    let approval_path = project_path.join("approval.json");
    let reply_deadline = Instant::now() + Duration::from_secs(3);
    while !approval_path.exists() && Instant::now() < reply_deadline {
        thread::sleep(Duration::from_millis(10));
    }
    let reply: Value = serde_json::from_slice(&fs::read(approval_path).unwrap()).unwrap();
    assert_eq!(reply["decision"], "once");

    let details = provider.session_details(&project_path, "ses_fake").unwrap();
    assert_eq!(details.branch, None);
    assert!(!details.branch_is_current);
    assert_eq!(
        details.changes,
        Some(LineChanges {
            additions: 15,
            deletions: 7
        })
    );
    fs::write(project_path.join("no-vcs"), "").unwrap();
    let details = provider.session_details(&project_path, "ses_fake").unwrap();
    assert_eq!(details.branch, None);
    assert_eq!(details.changes, None);
    assert!(matches!(
        provider.session_details(&project_path, "ses_foreign"),
        Err(OpenCodeError::SessionNotInProject)
    ));
    for archived in [true, false] {
        assert!(matches!(
            provider.set_session_archived(&project_path, "ses_foreign", archived),
            Err(OpenCodeError::SessionNotInProject)
        ));
    }
    fs::write(project_path.join("busy"), "").unwrap();
    assert!(provider
        .set_session_archived(&project_path, "ses_fake", true)
        .is_err());
    assert!(provider.delete_session(&project_path, "ses_fake").is_err());
    fs::remove_file(project_path.join("busy")).unwrap();
    fs::write(project_path.join("archive-error"), "").unwrap();
    assert!(provider
        .set_session_archived(&project_path, "ses_fake", true)
        .is_err());
    assert_eq!(
        provider
            .list_sessions(&project_path)
            .unwrap()
            .sessions
            .len(),
        1
    );
    fs::remove_file(project_path.join("archive-error")).unwrap();
    provider
        .set_session_archived(&project_path, "ses_fake", true)
        .unwrap();
    assert_eq!(
        provider
            .list_archived_sessions(&project_path)
            .unwrap()
            .sessions[0]
            .id,
        "ses_fake"
    );
    assert!(provider
        .list_sessions(&project_path)
        .unwrap()
        .sessions
        .is_empty());
    provider
        .set_session_archived(&project_path, "ses_fake", false)
        .unwrap();
    assert_eq!(
        provider.list_sessions(&project_path).unwrap().sessions[0].id,
        "ses_fake"
    );
    assert!(matches!(
        provider.delete_session(&project_path, "ses_fake"),
        Err(OpenCodeError::Protocol(message)) if message.contains("only archived")
    ));
    provider
        .set_session_archived(&project_path, "ses_fake", true)
        .unwrap();
    fs::write(project_path.join("busy"), "").unwrap();
    assert!(provider.delete_session(&project_path, "ses_fake").is_err());
    fs::remove_file(project_path.join("busy")).unwrap();
    fs::write(project_path.join("delete-error"), "").unwrap();
    assert!(provider.delete_session(&project_path, "ses_fake").is_err());
    fs::remove_file(project_path.join("delete-error")).unwrap();
    provider.delete_session(&project_path, "ses_fake").unwrap();
    assert!(project_path.join("deleted").exists());
    let server = provider
        .inner
        .lock_servers()
        .get(&Project::open(&project_path).unwrap().path().to_path_buf())
        .unwrap()
        .current()
        .unwrap();
    assert!(!server.owns_session("ses_fake"));

    let process_id = server.process_id.unwrap();
    provider.release_checkout(&checkout);
    assert!(provider.inner.lock_servers().is_empty());
    provider.shutdown();
    assert!(matches!(
        processes.terminate(process_id),
        Err(crate::processes::ProcessError::UnknownProcess)
    ));
}

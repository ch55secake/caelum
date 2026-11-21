use super::*;
mod checkout;
mod feed;
mod transcript;
use crate::application::Provider;
use crate::remote::{Prompt, Size};
use serde_json::{json, Value};
use wiremock::{
    matchers::{header, method, path, query_param},
    Mock, MockServer, ResponseTemplate,
};

async fn fixture() -> (MockServer, Boat) {
    let server = MockServer::start().await;
    let config = boat_sdk::Config::new("fixture-key")
        .unwrap()
        .with_base_url(&server.uri())
        .unwrap();
    Mock::given(method("GET"))
        .and(path("/me"))
        .and(header("authorization", "Bearer fixture-key"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"ok":true,"type":"me","user":{"id":"user-1"}})),
        )
        .expect(1)
        .mount(&server)
        .await;
    let boat = Boat::authenticate(Client::from_config(config).unwrap())
        .await
        .unwrap();
    (server, boat)
}

#[tokio::test]
async fn lists_every_page_without_exposing_sandbox_secrets() {
    let (server, boat) = fixture().await;
    for (rank, cursor, id) in [(2, None, "vm-1"), (1, Some("second"), "vm-2")] {
        let mut mock = Mock::given(method("GET")).and(path("/sandboxes"));
        if let Some(cursor) = cursor {
            mock = mock.and(query_param("cursor", cursor));
        }
        mock.respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "ok":true,"type":"list","sandboxes":[{"id":id,"name":"dev","state":"running","desktopAvailable":true,
            "snapshotAvailable":true,"desktopUrl":"secret-url"}],
            "pageInfo":{"limit":100,"hasMore":cursor.is_none(),"nextCursor":cursor.is_none().then_some("second")}
        }))).with_priority(rank).expect(1).mount(&server).await;
    }
    let list = boat.inventory().await.unwrap();
    assert_eq!(list.machines.len(), 2);
    assert_ne!(list.machines[0].key, list.machines[1].key);
    assert!(!format!("{list:?}").contains("secret-url"));
    assert!(
        !format!("{:?}", super::super::Credential::new("never-print".into()))
            .contains("never-print")
    );
}

fn sandbox(state: &str) -> serde_json::Value {
    json!({"id":"vm-1","name":"dev","state":state,"desktopAvailable":false,"snapshotAvailable":true,
           "environment":"project-env","environmentVersion":2})
}

#[tokio::test]
async fn deletion_confirms_the_exact_boat_id_and_requires_its_owning_account() {
    let (server, boat) = fixture().await;
    let key = MachineId {
        host: Host::Boat,
        account: boat.account.clone(),
        id: "vm-1".into(),
    };
    assert!(boat
        .delete(&MachineId {
            account: "other".into(),
            ..key.clone()
        })
        .await
        .is_err());
    for (target, status, accepted) in [
        ("vm-2", 202, false),
        ("vm-1", 403, false),
        ("vm-1", 202, true),
    ] {
        let mock=Mock::given(method("DELETE")).and(path("/sandboxes/vm-1"))
            .and(header("x-ascii-confirm-delete","vm-1")).and(header("authorization","Bearer fixture-key"))
            .respond_with(ResponseTemplate::new(status).set_body_json(json!({"ok":true,"type":"sandbox.deleting",
              "operation":{"id":"op-example","kind":"sandbox","targetId":target,"status":"pending","reason":"explicit",
              "attemptCount":0,"requestedAt":"2026-01-01T00:00:00Z","completedAt":null}})))
            .expect(1).mount_as_scoped(&server).await;
        let result = boat.delete(&key).await;
        if accepted {
            assert!(matches!(result,Ok(Outcome::Deleted(k)) if k==key));
        } else {
            let error = result.unwrap_err();
            assert!(error.contains(if status == 403 {
                "sandbox.delete-own"
            } else {
                "did not confirm"
            }));
        }
        drop(mock);
    }
    assert_eq!(server.received_requests().await.unwrap().len(), 4);
}

#[tokio::test]
async fn running_machine_lifetime_uses_explicit_seconds_or_null_and_boats_actual_deadline() {
    let (server, boat) = fixture().await;
    let key = MachineId {
        host: Host::Boat,
        account: boat.account.clone(),
        id: "vm-1".into(),
    };
    for (lifetime, value, deadline) in [
        (
            Lifetime::EightHours,
            json!(28800),
            Some("2026-11-20T00:00:00Z"),
        ),
        (Lifetime::UntilStopped, Value::Null, None),
    ] {
        let mut machine = sandbox("running");
        machine["archiveAfter"] = json!(deadline);
        Mock::given(method("PATCH"))
            .and(path("/sandboxes/vm-1"))
            .and(wiremock::matchers::body_json(json!({"ttlSeconds":value})))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({"ok":true,"type":"info","sandbox":machine})),
            )
            .expect(1)
            .mount(&server)
            .await;
        let actual = boat.lifetime(&key, lifetime).await.unwrap();
        assert_eq!(actual.key, key);
        assert_eq!(actual.auto_stop_unix.is_some(), deadline.is_some());
    }
}

async fn environment(server: &MockServer) {
    environment_on(server, "main", 5).await;
}

async fn environment_on(server: &MockServer, branch: &str, priority: u8) {
    Mock::given(method("GET")).and(path("/environments")).respond_with(ResponseTemplate::new(200).set_body_json(json!({
        "environments":[{"id":"env-1","name":"project-env","isDefault":true,"latestVersionNumber":2,
        "safeForThirdParties":false,"passGithub":true,"passSecrets":false,"passSandboxCredentials":false,
        "passAgentsCredentials":true,"envContents":"","secretFiles":[],"versions":[],"selectedRepositories":[{"fullName":"example/project","baseBranch":branch}]}]
    }))).with_priority(priority).mount(server).await;
}

#[tokio::test]
async fn create_retry_reuses_body_and_idempotency_key_without_changing_shared_environment() {
    let (server, boat) = fixture().await;
    environment(&server).await;
    let source = Source {
        published_tip: None,
        project: "/local/project".into(),
        account: boat.account.clone(),
        repository: "example/project".into(),
        base: "main".into(),
        environment: Some("project-env".into()),
    };
    let plan = Plan {
        token: uuid::Uuid::new_v4().to_string(),
        branch: "feat/new".into(),
        size: Size::Small,
        lifetime: crate::remote::Lifetime::HalfHour,
    };
    Mock::given(method("POST")).and(path("/sandboxes")).and(header("idempotency-key",plan.token.as_str()))
        .and(wiremock::matchers::body_partial_json(json!({"environment":"project-env","type":"small","ttlSeconds":1800})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok":true,"type":"sandbox","status":"provisioning","sandbox":sandbox("provisioning")})))
        .expect(2).mount(&server).await;
    assert_eq!(
        boat.create(&source, &plan).await.unwrap(),
        boat.create(&source, &plan).await.unwrap()
    );
    let requests = server.received_requests().await.unwrap();
    let posts: Vec<_> = requests
        .iter()
        .filter(|r| r.method.as_str() == "POST")
        .collect();
    assert_eq!(posts.len(), 2);
    assert_eq!(posts[0].body, posts[1].body);
}

#[tokio::test]
async fn a_prepared_environment_reports_create_scope_and_plan_denials_at_the_vm_step() {
    let (server, boat) = fixture().await;
    environment(&server).await;
    let source = Source {
        project: "/local/project".into(),
        account: boat.account.clone(),
        repository: "example/project".into(),
        base: "main".into(),
        published_tip: None,
        environment: Some("project-env".into()),
    };
    let plan = Plan {
        token: uuid::Uuid::new_v4().to_string(),
        branch: "feat/new".into(),
        size: Size::Large,
        lifetime: Lifetime::Hour,
    };
    for (body, hint) in [
        (json!({"code":"api_key_action_forbidden"}), "sandbox.create"),
        (
            json!({"code":"api_key_sandbox_forbidden"}),
            "allowed resources",
        ),
        (
            json!({"code":"trial_machine_class_not_allowed"}),
            "Small or Standard",
        ),
        (
            json!({"code":"private-detail", "requestId":"https://example.invalid/private-detail"}),
            "sandbox.create",
        ),
        (json!({"error":"Forbidden"}), "sandbox.create"),
    ] {
        let denied = Mock::given(method("POST"))
            .and(path("/sandboxes"))
            .respond_with(ResponseTemplate::new(403).set_body_json(body))
            .expect(1)
            .mount_as_scoped(&server)
            .await;
        let error = boat.create(&source, &plan).await.unwrap_err();
        assert!(
            error.starts_with("Create VM:") && error.contains("403") && error.contains(hint),
            "{error}"
        );
        assert!(!error.contains("environment.write") && !error.contains("private-detail"));
        drop(denied);
    }
    assert!(server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .all(|r| r.method.as_str() == "GET" || r.url.path() == "/sandboxes"));
}

fn agent_target() -> Target {
    Target {
        root: Some("/workspace/checkout".into()),
        required_base: None,
        machine: MachineId {
            host: Host::Boat,
            account: "user-1".into(),
            id: "vm-1".into(),
        },
        repository: "example/project".into(),
    }
}

fn default_prompt(provider: Provider, conversation: Option<&str>, text: &str) -> Prompt {
    Prompt {
        provider,
        model: None,
        effort: None,
        conversation: conversation.map(str::to_owned),
        text: text.into(),
    }
}

#[tokio::test]
async fn switching_harnesses_uses_the_exact_machine_conversation_cursor_and_terminal_run() {
    use crate::remote::harness::Harness;
    use wiremock::matchers::body_json;
    let (server, boat) = fixture().await;
    let target = agent_target();
    let run = json!({"id":"r","promptId":"p","sandboxId":"vm-1","conversationId":"c","status":"finished","done":true});
    Mock::given(method("POST")).and(path("/sandboxes/vm-1/commands"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok":true,"type":"command","success":true,"exitCode":0,"stdout":"/workspace/checkout\n","stderr":"","timedOut":false}))).expect(3).mount(&server).await;
    for (scope, message, provider, prompt, id) in [
        (
            json!({"new":true,"model":"boat-model-b","reasoningEffort":"high"}),
            "Start",
            "codex",
            "p",
            "r",
        ),
        (
            json!({"conversationId":"c","model":"boat-model-b","reasoningEffort":"none"}),
            "Quick follow-up",
            "codex",
            "p-effort",
            "r-effort",
        ),
        (
            json!({"conversationId":"c","model":"example:vendor/next"}),
            "Continue",
            "opencode",
            "p2",
            "r2",
        ),
    ] {
        let mut body = scope;
        body["provider"] = provider.into();
        body["prompt"] = message.into();
        let mut receipt = run.clone();
        receipt["promptId"] = prompt.into();
        receipt["id"] = id.into();
        Mock::given(method("POST")).and(path("/sandboxes/vm-1/prompt")).and(body_json(body))
            .respond_with(ResponseTemplate::new(202).set_body_json(json!({"ok":true,"type":"queued","id":"vm-1","promptId":prompt,"conversationId":"c","promptRun":receipt,"status":"queued","provider":provider}))).expect(1).mount(&server).await;
    }
    let Outcome::Accepted(first) = boat
        .send(
            &target,
            &Prompt {
                model: Some("boat-model-b".into()),
                effort: Some("high".into()),
                ..default_prompt(Provider::Codex, None, "Start")
            },
        )
        .await
        .unwrap()
    else {
        panic!()
    };
    Mock::given(path("/sandboxes/vm-1/prompts/p"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"ok":true,"type":"status","id":"vm-1","promptRun":run})),
        )
        .mount(&server)
        .await;
    Mock::given(path("/sandboxes/vm-1/events")).and(query_param("conversation","c")).and(query_param("cursor","before")).and(query_param("sort","asc"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok":true,"type":"events","id":"vm-1","events":[
            {"type":"response","id":"1","conversationId":"other","timestamp":10,"data":{"content":"another session"}},
            {"type":"response","id":"old","conversationId":"c","taskId":"old-prompt","timestamp":10,"data":{"content":"another turn"}},
            {"type":"response","id":"unowned","timestamp":10,"data":{"content":"unowned"}},
            {"type":"response","id":"text","conversationId":"c","timestamp":11,"data":{"content":"Editing…","is_streaming":true}},
            {"type":"response","id":"tools","conversationId":"c","timestamp":12,"data":{"tools":[{"use":{"id":"call-1","name":"Edit","input":{"filePath":"/workspace/checkout/src/lib.rs","token":"private-tool-input"}}}]}},
            {"type":"response","id":"done","taskId":"p","timestamp":13,"data":{"content":"Edited the file."}},
            {"type":"response","id":"result","conversationId":"c","timestamp":14,"data":{"tools":[{"use":{"id":"call-1","name":"Edit"},"result":{"is_error":false,"content":"private-tool-output"}},{"use":{"id":"call-2","name":"Bash","input":{"command":"cargo test --token private-tool-input"}},"result":{"is_error":true}}]}},
            {"type":"response","id":"reverted","conversationId":"c","timestamp":15,"data":{"content":"obsolete","is_reverted":true}}
        ],"pageInfo":{"hasMore":false,"nextCursor":"after","limit":100}}))).expect(1).mount(&server).await;
    let page = boat.follow(&target, &first, Some("before")).await.unwrap();
    use crate::remote::{Speaker, Thread, ToolState};
    assert_eq!(page.lines[1].speaker, Speaker::Tool(ToolState::Running));
    let mut thread = Thread::default();
    for line in page.lines {
        thread.append(line);
    }
    assert_eq!(thread.lines.len(), 3);
    assert_eq!(thread.lines[0].text, "Edited the file.");
    assert_eq!(thread.lines[0].speaker, Speaker::Codex);
    assert!(!thread.lines[0].partial);
    assert_eq!(thread.lines[1].speaker, Speaker::Tool(ToolState::Complete));
    assert_eq!(thread.lines[1].tool_detail.as_deref(), Some("src/lib.rs"));
    assert_eq!(thread.lines[2].tool_detail.as_deref(), Some("cargo"));
    assert_eq!(thread.lines[2].speaker, Speaker::Tool(ToolState::Failed));
    assert!(!serde_json::to_string(&thread)
        .unwrap()
        .contains("private-tool"));
    assert_eq!(page.cursor.as_deref(), Some("after"));
    assert_eq!(
        page.status,
        crate::remote::RunStatus::Ended(crate::remote::TurnEnd::Finished)
    );
    let Outcome::Accepted(changed) = boat
        .send(
            &target,
            &Prompt {
                model: Some("boat-model-b".into()),
                effort: Some("none".into()),
                ..default_prompt(
                    Provider::Codex,
                    Some(&first.conversation),
                    "Quick follow-up",
                )
            },
        )
        .await
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(changed.conversation, first.conversation);
    let Outcome::Accepted(next) = boat
        .send(
            &target,
            &Prompt {
                model: Some("example:vendor/next".into()),
                ..default_prompt(Provider::OpenCode, Some(&changed.conversation), "Continue")
            },
        )
        .await
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(next.provider, Provider::OpenCode);
    assert_eq!(next.conversation, first.conversation);
    assert_ne!(next.id, first.id);
    Mock::given(method("POST"))
        .and(path("/sandboxes/vm-1/interrupt"))
        .and(query_param("conversation", "c"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"ok":true,"type":"interrupted","id":"vm-1","status":"ok"})),
        )
        .expect(1)
        .mount(&server)
        .await;
    boat.interrupt(&target, &next).await.unwrap();
}

#[tokio::test]
async fn a_mismatched_receipt_is_uncertain_and_recovery_checks_the_latest_harness_and_prompt() {
    use crate::remote::harness::Harness;
    use wiremock::matchers::body_json;
    let (server, boat) = fixture().await;
    let target = agent_target();
    let run = |p: &str| json!({"id":format!("run-{p}"),"promptId":p,"sandboxId":"vm-1","conversationId":"c","status":"finished","done":true});
    Mock::given(method("POST")).and(path("/sandboxes/vm-1/commands"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok":true,"type":"command","success":true,"exitCode":0,"stdout":"/workspace/checkout\n","stderr":"","timedOut":false}))).expect(1).mount(&server).await;
    Mock::given(method("POST")).and(path("/sandboxes/vm-1/prompt"))
        .and(body_json(json!({"provider":"codex","conversationId":"c","prompt":"Next"})))
        .respond_with(ResponseTemplate::new(202).set_body_json(json!({"ok":true,"type":"queued","id":"vm-1","promptId":"p","conversationId":"c","promptRun":run("p"),"status":"queued","provider":"opencode"})))
        .expect(1).mount(&server).await;
    assert!(boat
        .send(&target, &default_prompt(Provider::Codex, Some("c"), "Next"))
        .await
        .is_err());

    for p in ["p", "older"] {
        Mock::given(method("GET"))
            .and(path(format!("/sandboxes/vm-1/prompts/{p}")))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(
                    json!({"ok":true,"type":"status","id":"vm-1","promptRun":run(p)}),
                ),
            )
            .mount(&server)
            .await;
    }
    Mock::given(method("GET")).and(path("/sandboxes/vm-1/conversations"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok":true,"type":"conversations","id":"vm-1","conversations":[{"id":"c","lastHarness":"codex","prompts":2,"running":false,"current":true}]})))
        .mount(&server).await;
    Mock::given(method("GET")).and(path("/sandboxes/vm-1/events")).and(query_param("conversation","c"))
        .and(query_param("type","prompt")).and(query_param("sort","desc")).and(query_param("limit","1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok":true,"type":"events","id":"vm-1","events":[{"type":"prompt","conversationId":"c","taskId":"p"}]})))
        .expect(2).mount(&server).await;
    assert!(boat
        .recover(&target, Provider::OpenCode, Some("c"), "p")
        .await
        .is_err());
    assert!(boat
        .recover(&target, Provider::Codex, Some("c"), "older")
        .await
        .is_err());
    assert!(boat
        .recover(&target, Provider::Codex, None, "p")
        .await
        .is_err());
    let found = boat
        .recover(&target, Provider::Codex, Some("c"), "p")
        .await
        .unwrap();
    assert_eq!(found.provider, Provider::Codex);
    assert_eq!(
        (found.conversation.as_str(), found.id.as_str()),
        ("c", "run-p")
    );
}

#[tokio::test]
async fn live_and_historical_process_samples_never_expose_command_arguments() {
    use crate::remote::Activity;
    let (server, boat) = fixture().await;
    let mut vm = sandbox("idle");
    vm["vcpu"] = 4.into();
    vm["memoryGB"] = 8.into();
    vm["archiveAfter"] = "2026-01-01T12:00:00Z".into();
    Mock::given(path("/sandboxes/vm-1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"ok":true,"type":"info","sandbox":vm})),
        )
        .mount(&server)
        .await;
    let key = MachineId {
        host: Host::Boat,
        account: "user-1".into(),
        id: "vm-1".into(),
    };
    for live in [true, false] {
        let guard=Mock::given(path("/sandboxes/vm-1/processes")).respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "ok":true,"type":"processes","live":live,"total":2,"processes":[
                {"pid":25,"cpuPercent":1,"memoryMB":42,"command":"sh"},
                {"pid":26,"cpuPercent":84.3,"memoryMB":610,"command":"/usr/bin/cargo --token fixture-secret"}
            ]
        }))).expect(1).mount_as_scoped(&server).await;
        let sample = boat.stats(&key).await.unwrap();
        assert_eq!((sample.vcpu, sample.memory_gb), (Some(4), Some(8)));
        assert!(sample.machine.auto_stop_unix.is_some());
        assert!(!format!("{sample:?}").contains("fixture-secret"));
        match sample.activity {
            Activity::Live(Some(p)) if live => assert_eq!(p.name, "cargo"),
            Activity::LastSample(Some(p)) if !live => assert_eq!(p.cpu_percent, 84.3),
            other => panic!("wrong sample {other:?}"),
        }
        drop(guard);
    }
}

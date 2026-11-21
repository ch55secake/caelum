use super::{agent_target, fixture};
use crate::{
    application::Provider,
    remote::{boat::events, Run, Speaker, Thread, ToolState},
};
use serde_json::{json, Value};
use wiremock::{
    matchers::{method, path, query_param},
    Mock, ResponseTemplate,
};

fn run() -> Run {
    Run {
        provider: Provider::Codex,
        conversation: "chat".into(),
        prompt: "p".into(),
        id: "r".into(),
    }
}

#[test]
fn typed_public_summaries_update_independently_of_text_and_never_fall_back_to_private_content() {
    let mut thread = Thread::default();
    for running in [true, false] {
        let item = json!({"type":"reasoning","id":"rs.1","status":if running {"in_progress"} else {"completed"},
            "summary":[{"type":"summary_text","text":if running {"Inspecting the tests."} else {"Checked the tests."}},{"type":"reasoning_text","text":"private content"}],
            "content":[{"type":"reasoning_text","text":"private content"}],"encrypted_content":"private content"});
        let content = json!([item, {"type":"thinking","thinking":"private content"}, {"type":"text","id":"first","text":"First reply"}, {"type":"output_text","id":"second","text":"Second reply"}]);
        let page = json!([
            {"type":"response","id":"old","conversationId":"other","data":{"content":content}},
            {"type":"response","id":"ours.1","taskId":"p","data":{"content":content,"is_streaming":running}},
            {"type":"response","id":"undone","taskId":"p","data":{"content":content,"is_reverted":true}}]);
        for l in events::lines(serde_json::from_value(page).unwrap(), &run(), None, None) {
            thread.append(l);
        }
        assert_eq!(thread.lines.len(), 3);
        assert_eq!(thread.lines[0].speaker, Speaker::ReasoningSummary);
        assert_eq!(thread.lines[0].partial, running);
    }
    assert_eq!(thread.lines[0].text, "Checked the tests.");
    assert_eq!(thread.lines[1].text, "First reply");
    assert_eq!(thread.lines[2].text, "Second reply");
    assert!(!serde_json::to_string(&thread)
        .unwrap()
        .contains("private content"));
}

#[tokio::test]
async fn opening_a_saved_call_reads_only_its_event_and_bounds_text_without_persisting_payloads() {
    let (server, boat) = fixture().await;
    let target = agent_target();
    let call = json!({"use":{"id":"c.1","name":"Bash","input":{"command":"cargo test --example transient-command"}},"result":{"tool_use_id":"c.1","is_error":true,"content":[{"type":"image","data":"not text"},{"type":"text","text":format!("transient-output {}","界".repeat(7000))}]}});
    let event = json!({"id":"response.1","type":"response","taskId":"p","conversationId":"chat","data":{"tools":[call]}});
    let mut thread = Thread::default();
    for l in events::lines(
        serde_json::from_value(json!([event])).unwrap(),
        &run(),
        target.root.as_deref(),
        Some("before"),
    ) {
        thread.append(l);
    }
    let saved = serde_json::to_string(&thread).unwrap();
    assert!(!saved.contains("transient-"));
    let restored: Thread = serde_json::from_str(&saved).unwrap();
    let source = restored.lines[0].tool_source.as_ref().unwrap();
    let wrong = |field: &str, value: Value| {
        let mut e = event.clone();
        e[field] = value;
        e["data"]["tools"][0]["result"]["content"] = "other output".into();
        e
    };
    let mut mismatch = event.clone();
    mismatch["data"]["tools"][0]["result"]["tool_use_id"] = "another-call".into();
    let mut reverted = event.clone();
    reverted["data"]["is_reverted"] = true.into();
    let others = json!([
        wrong("conversationId", "other".into()),
        wrong("taskId", "other".into()),
        wrong("id", "other".into()),
        mismatch,
        reverted
    ]);
    for found in [true, false] {
        let mut items = others.clone();
        if found {
            items.as_array_mut().unwrap().push(event.clone());
        }
        let guard = Mock::given(method("GET"))
            .and(path("/sandboxes/vm-1/events"))
            .and(query_param("conversation", "chat"))
            .and(query_param("cursor", "before"))
            .and(query_param("sort", "asc"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({"ok":true,"type":"events","id":"vm-1","events":items})),
            )
            .expect(1)
            .mount_as_scoped(&server)
            .await;
        let result = boat.tool_output(&target.machine, source).await;
        if found {
            let body = result.unwrap();
            assert_eq!(body.state, ToolState::Failed);
            assert_eq!(body.command, "cargo test --example transient-command");
            assert!(
                body.clipped
                    && body.output.starts_with("transient-output")
                    && body.output.ends_with('界')
            );
            assert!(body.output.len() <= 16 * 1024);
            assert!(
                !body.output.contains("not text") && !format!("{body:?}").contains("transient-")
            );
        } else {
            assert!(result.unwrap_err().contains("no longer lists"));
        }
        drop(guard);
    }
}

#[tokio::test]
async fn a_call_without_an_id_requires_its_named_slot_and_displays_command_streams() {
    let (server, boat) = fixture().await;
    let target = agent_target();
    let shell = json!({"use":{"name":"Bash","input":{"command":"cargo test"}},"result":{"content":"{\"stdout\":\"compiling\",\"stderr\":\"test failed\\n\",\"exitCode\":1}"}});
    let read = json!({"use":{"name":"Read","input":{"path":"README.md"}},"result":{"content":"other output"}});
    let event = |tools: Value| json!({"type":"response","id":"reply","taskId":"p","timestamp":10,"data":{"tools":tools}});
    let mut lines = events::lines(
        serde_json::from_value(json!([event(json!([read, shell]))])).unwrap(),
        &run(),
        target.root.as_deref(),
        None,
    );
    let source = lines.pop().unwrap().tool_source.unwrap();
    for reordered in [false, true] {
        let calls = if reordered {
            json!([shell, read])
        } else {
            json!([read, shell])
        };
        let guard = Mock::given(method("GET"))
            .and(path("/sandboxes/vm-1/events"))
            .and(query_param("conversation", "chat"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                json!({"ok":true,"type":"events","id":"vm-1","events":[event(calls)]}),
            ))
            .expect(1)
            .mount_as_scoped(&server)
            .await;
        let result = boat.tool_output(&target.machine, &source).await;
        if reordered {
            assert!(result.is_err());
        } else {
            let body = result.unwrap();
            assert_eq!(body.state, ToolState::Failed);
            assert_eq!(body.output, "compiling\ntest failed\nExit 1");
        }
        drop(guard);
    }
}

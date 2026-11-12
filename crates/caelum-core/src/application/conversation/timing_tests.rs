use super::*;
use serde_json::json;

fn output(conversation: &Conversation, id: &str) -> u64 {
    conversation
        .entries
        .iter()
        .find(|entry| entry.id == id)
        .unwrap()
        .metadata["outputDurationMs"]
        .as_u64()
        .unwrap()
}

#[test]
fn live_duration_includes_submission_wait_thinking_tools_and_retries() {
    for provider in Provider::ALL {
        let mut conversation = Conversation::default();
        conversation.begin_turn(Some("build".into()), 1_000);
        conversation.optimistic("local-1".into(), "Build this".into());
        let events = match provider {
            Provider::OpenCode => vec![
                ("session.execution.started", json!({"_created":3_000})),
                (
                    "session.reasoning.started",
                    json!({"assistantMessageID":"thinking", "_created":4_000}),
                ),
                (
                    "session.tool.called",
                    json!({"id":"build", "name":"bash", "_created":10_000}),
                ),
                (
                    "session.tool.success",
                    json!({"id":"build", "_created":90_000}),
                ),
                ("session.retry.scheduled", json!({"_created":91_000})),
                ("session.execution.started", json!({"_created":100_000})),
                (
                    "session.text.started",
                    json!({"assistantMessageID":"answer", "_created":120_000}),
                ),
                (
                    "session.text.ended",
                    json!({"assistantMessageID":"answer", "text":"Built", "_created":129_000}),
                ),
                ("session.execution.succeeded", json!({"_created":130_000})),
            ],
            Provider::Codex => vec![
                (
                    "turn/started",
                    json!({"turn":{"id":"turn-1"}, "_created":3_000}),
                ),
                (
                    "item/started",
                    json!({"item":{"id":"thinking", "type":"reasoning"}, "_created":4_000}),
                ),
                (
                    "item/started",
                    json!({"item":{"id":"build", "type":"commandExecution"}, "_created":10_000}),
                ),
                (
                    "item/completed",
                    json!({"item":{"id":"build", "type":"commandExecution"}, "_created":90_000}),
                ),
                (
                    "turn/started",
                    json!({"turn":{"id":"turn-1"}, "_created":100_000}),
                ),
                (
                    "item/started",
                    json!({"item":{"id":"answer", "type":"agentMessage"}, "_created":120_000}),
                ),
                (
                    "item/completed",
                    json!({"item":{"id":"answer", "type":"agentMessage", "text":"Built"}, "_created":129_000}),
                ),
                (
                    "turn/completed",
                    json!({"turn":{"id":"turn-1"}, "_created":130_000}),
                ),
            ],
        };
        for (method, data) in events {
            conversation.apply(provider, method, &data);
        }
        let id = format!("{}-answer", provider.as_str());
        assert_eq!(output(&conversation, &id), 129_000, "{provider:?}");
        conversation.settle_at(900_000);
        assert_eq!(output(&conversation, &id), 129_000, "duplicate settlement");

        conversation.begin_turn(Some("plan".into()), 200_000);
        conversation.optimistic("local-2".into(), "Next task".into());
        conversation.provider_started_with_agent(Some("plan".into()), 202_000);
        conversation.delta("next-answer".into(), EntryKind::Assistant, "Next", false);
        conversation.settle_at(209_000);
        assert_eq!(output(&conversation, "next-answer"), 9_000);
        assert_eq!(output(&conversation, &id), 129_000);
    }
}

fn history() -> Vec<Value> {
    vec![
        json!({"id":"prompt", "type":"user", "text":"Build this", "time":{"created":1_000}}),
        json!({"id":"work", "type":"assistant", "agent":"build", "time":{"created":3_000,"completed":119_000}, "finish":"tool-calls", "content":[
            {"type":"reasoning", "text":"Inspecting the project"},
            {"type":"text", "text":"I will build and check it"},
            {"id":"build-tool", "type":"tool", "name":"bash", "state":{"status":"completed", "time":{"start":10_000,"end":118_000}, "content":[{"type":"text","text":"Build complete"}]}}
        ]}),
        json!({"id":"answer", "type":"assistant", "agent":"build", "time":{"created":120_000,"completed":129_000}, "finish":"stop", "content":[
            {"type":"text", "text":"Built successfully"},
            {"type":"text", "text":"All done"}
        ]}),
        json!({"id":"idle", "type":"idle", "outcome":"succeeded", "time":{"created":130_000}}),
    ]
}

#[test]
fn restored_duration_spans_every_step_and_ends_at_execution_completion() {
    let mut conversation = Conversation::default();
    let mut messages = history();
    messages.extend([
        json!({"id":"next-prompt", "type":"user", "text":"Next task", "time":{"created":200_000}}),
        json!({"id":"next-answer", "type":"assistant", "agent":"plan", "time":{"created":205_000,"completed":208_000}, "content":[{"type":"text","text":"Next"}]}),
        json!({"id":"next-idle", "type":"idle", "outcome":"succeeded", "time":{"created":209_000}}),
    ]);
    conversation.history(Provider::OpenCode, &messages, false);
    assert_eq!(output(&conversation, "opencode-work"), 118_000);
    assert_eq!(output(&conversation, "opencode-answer"), 128_000);
    assert_eq!(output(&conversation, "opencode-answer-text-1"), 129_000);
    assert_eq!(output(&conversation, "opencode-next-answer"), 9_000);
    let tool = conversation
        .entries
        .iter()
        .find(|entry| entry.id == "opencode-tool-build-tool")
        .unwrap();
    assert_eq!(tool.started_at, Some(10_000));
    assert_eq!(tool.metadata["ended"], 118_000);

    // An output after an idle boundary must not inherit the previous prompt.
    conversation.history(Provider::OpenCode, &[json!({
        "id":"orphan", "type":"assistant", "agent":"build", "time":{"created":300_000,"completed":301_000}, "content":[{"type":"text","text":"No prompt loaded"}]
    })], false);
    assert!(conversation.entries.last().unwrap().metadata["outputDurationMs"].is_null());
}

#[test]
fn earlier_history_fills_missing_prompt_timing_and_late_snapshots_cannot_shorten_it() {
    let mut conversation = Conversation::default();
    let messages = history();
    conversation.history(Provider::OpenCode, &messages[2..], false);
    assert!(conversation
        .entries
        .iter()
        .all(|entry| entry.metadata["outputDurationMs"].is_null()));
    conversation.history(Provider::OpenCode, &messages[..3], true);
    assert_eq!(output(&conversation, "opencode-answer-text-1"), 129_000);

    let mut stale = messages[2].clone();
    stale["time"].as_object_mut().unwrap().remove("completed");
    stale.as_object_mut().unwrap().remove("finish");
    stale["content"] = json!([{"type":"text", "text":"Built"}]);
    conversation.history(Provider::OpenCode, &[stale], false);
    assert_eq!(output(&conversation, "opencode-answer-text-1"), 129_000);
    conversation.history(Provider::OpenCode, &messages, false);
    assert_eq!(output(&conversation, "opencode-answer-text-1"), 129_000);
}

#[test]
fn refresh_preserves_the_original_live_submission_and_completion_times() {
    let mut conversation = Conversation::default();
    conversation.begin_turn(Some("build".into()), 500);
    conversation.optimistic("local".into(), "Build this".into());
    conversation.apply(
        Provider::OpenCode,
        "session.execution.started",
        &json!({"_created":3_000}),
    );
    conversation.apply(
        Provider::OpenCode,
        "session.text.ended",
        &json!({"assistantMessageID":"answer", "ordinal":1,"text":"All done", "_created":129_000}),
    );
    conversation.apply(
        Provider::OpenCode,
        "session.execution.succeeded",
        &json!({"_created":131_000}),
    );
    assert_eq!(output(&conversation, "opencode-answer-text-1"), 130_500);
    conversation.history(Provider::OpenCode, &history(), false);
    assert_eq!(output(&conversation, "opencode-answer-text-1"), 130_500);
    assert_eq!(
        conversation
            .entries
            .iter()
            .filter(|entry| entry.kind == EntryKind::User)
            .count(),
        1
    );
}

#[test]
fn steering_does_not_restart_the_original_prompt_timer_after_reopening() {
    let mut conversation = Conversation::default();
    let mut messages = history();
    messages.insert(2, json!({"id":"steering", "type":"user", "text":"Also check lint", "time":{"created":119_500}}));
    conversation.history(Provider::OpenCode, &messages, false);
    assert_eq!(output(&conversation, "opencode-answer-text-1"), 129_000);

    let mut paged = Conversation::default();
    paged.history(Provider::OpenCode, &messages[1..], false);
    assert!(paged
        .entries
        .iter()
        .all(|entry| entry.metadata["outputDurationMs"].is_null()));
    paged.history(Provider::OpenCode, &messages[..1], true);
    assert_eq!(output(&paged, "opencode-answer-text-1"), 129_000);
}

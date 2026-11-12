use super::*;

#[test]
fn compaction_without_a_summary_stays_visible_for_both_providers() {
    use presentation::{entry_visible, outcome, Outcome};
    let mut codex = Conversation::default();
    codex.apply(
        Provider::Codex,
        "item/started",
        &json!({"item":{"id":"compact","type":"contextCompaction"},"_created":100}),
    );
    assert!(entry_visible(&codex.entries[0]));
    assert!(!codex.entries[0].complete);
    codex.apply(
        Provider::Codex,
        "item/completed",
        &json!({"item":{"id":"compact","type":"contextCompaction"},"_created":200}),
    );
    codex.history(
        Provider::Codex,
        &[json!({"status":"completed","items":[{"id":"compact","type":"contextCompaction"}]})],
        false,
    );
    assert_eq!(codex.entries.len(), 1);
    assert!(entry_visible(&codex.entries[0]));
    assert_eq!(
        outcome(&codex.entries[0].status, codex.entries[0].complete),
        Outcome::Success
    );

    let mut native = Conversation::default();
    native.used_tokens = Some(1000);
    native.apply(
        Provider::OpenCode,
        "session.compaction.started",
        &json!({"inputID":"compact","_created":100}),
    );
    assert!(entry_visible(&native.entries[0]));
    native.apply(
        Provider::OpenCode,
        "session.compaction.ended",
        &json!({"inputID":"compact","_created":200}),
    );
    native.history(Provider::OpenCode, &[json!({"id":"compact","type":"compaction","status":"completed","summary":"","time":{"created":100}})], false);
    assert_eq!(native.entries.len(), 1);
    assert!(entry_visible(&native.entries[0]));
    assert_eq!(
        outcome(&native.entries[0].status, native.entries[0].complete),
        Outcome::Success
    );
    assert_eq!(native.used_tokens, None);
}

#[test]
fn compaction_failures_and_summaries_survive_refresh_and_late_events() {
    use presentation::{entry_visible, Outcome, Tool};
    let mut c = Conversation::default();
    c.apply(
        Provider::OpenCode,
        "session.compaction.started",
        &json!({"inputID":"first","_created":100}),
    );
    c.apply(
        Provider::OpenCode,
        "session.compaction.delta",
        &json!({"inputID":"first","text":"## Context\n","_created":110}),
    );
    c.apply(
        Provider::OpenCode,
        "session.compaction.delta",
        &json!({"inputID":"first","text":"Preserve drafts.","_created":120}),
    );
    c.apply(Provider::OpenCode, "session.compaction.failed", &json!({"inputID":"first","error":{"message":"Summary service unavailable"},"_created":130}));
    let revision = c.activity;
    c.history(
        Provider::OpenCode,
        &[json!({"id":"first","type":"compaction","status":"failed","time":{"created":100}})],
        false,
    );
    assert_eq!(c.entries.len(), 1);
    assert!(entry_visible(&c.entries[0]));
    assert_eq!(c.entries[0].text, "## Context\nPreserve drafts.");
    assert_eq!(c.entries[0].error, "Summary service unavailable");
    assert_eq!(
        Tool::from_entry(&c.entries[0], 1000).outcome,
        Outcome::Failure
    );
    c.apply(
        Provider::OpenCode,
        "session.compaction.started",
        &json!({"inputID":"first","_created":100}),
    );
    c.apply(
        Provider::OpenCode,
        "session.compaction.delta",
        &json!({"inputID":"first","text":"stale","_created":140}),
    );
    assert_eq!(c.activity, revision);
    c.apply(
        Provider::OpenCode,
        "session.compaction.started",
        &json!({"inputID":"second","_created":200}),
    );
    c.apply(
        Provider::OpenCode,
        "session.compaction.failed",
        &json!({"inputID":"first","error":"late failure","_created":210}),
    );
    c.apply(
        Provider::OpenCode,
        "session.compaction.delta",
        &json!({"text":"stale","_created":150}),
    );
    assert!(!c.entries[1].complete);
    assert!(c.entries[1].text.is_empty());
    c.apply(
        Provider::OpenCode,
        "session.compaction.delta",
        &json!({"inputID":"second","text":"Fresh summary","_created":220}),
    );
    c.apply(
        Provider::OpenCode,
        "session.compaction.ended",
        &json!({"inputID":"second","_created":230}),
    );
    c.history(Provider::OpenCode, &[json!({"id":"second","type":"compaction","status":"completed","summary":"Fresh summary","time":{"created":200}})], false);
    assert_eq!(c.entries.len(), 2);
    assert_eq!(c.entries[1].text, "Fresh summary");
    assert_eq!(
        Tool::from_entry(&c.entries[1], 1000).outcome,
        Outcome::Success
    );
}

#[test]
fn compaction_interruption_does_not_claim_success() {
    use presentation::{Outcome, Tool};
    let mut c = Conversation::default();
    c.apply(
        Provider::OpenCode,
        "session.compaction.started",
        &json!({"inputID":"compact","_created":100}),
    );
    c.settle();
    assert_eq!(
        Tool::from_entry(&c.entries[0], 1000).outcome,
        Outcome::Interrupted
    );
    c.history(
        Provider::OpenCode,
        &[json!({"id":"compact","type":"compaction","status":"running","time":{"created":100}})],
        false,
    );
    assert_eq!(
        Tool::from_entry(&c.entries[0], 1000).outcome,
        Outcome::Interrupted
    );
    let mut restored = Conversation::default();
    restored.history(
        Provider::OpenCode,
        &[json!({"id":"compact","type":"compaction","status":"cancelled"})],
        false,
    );
    assert_eq!(
        Tool::from_entry(&restored.entries[0], 1000).outcome,
        Outcome::Interrupted
    );
    restored.history(Provider::OpenCode, &[json!({"id":"failed","type":"compaction","status":"failed","error":{"message":"Could not compact context"}})], false);
    let failed = Tool::from_entry(&restored.entries[1], 1000);
    assert_eq!(failed.outcome, Outcome::Failure);
    assert_eq!(failed.error, "Could not compact context");
    for status in ["running", "pending", "in_progress"] {
        let mut restored = Conversation::default();
        restored.history(
            Provider::OpenCode,
            &[json!({"id":"active","type":"compaction","status":status})],
            false,
        );
        assert_eq!(
            Tool::from_entry(&restored.entries[0], 1000).outcome,
            Outcome::Running
        );
        restored.settle();
        assert_eq!(
            Tool::from_entry(&restored.entries[0], 1000).outcome,
            Outcome::Interrupted
        );
    }
}

#[test]
fn command_success_omits_exit_zero_but_failure_keeps_its_code() {
    use presentation::{Outcome, Tool};
    for metadata_key in ["exit", "exitCode"] {
        for code in [0, 101] {
            let mut c = Conversation::default();
            c.history(Provider::OpenCode, &[json!({"id":"message","type":"assistant","content":[{"id":"run","type":"tool","name":"bash","state":{"status":"completed","input":{"command":"cargo test","timeout":120000},"metadata":{metadata_key:code},"content":[{"type":"text","text":"test output"}]}}]})], false);
            let tool = Tool::from_entry(&c.entries[0], 0);
            assert_eq!(tool.summary, "cargo test");
            assert_eq!(tool.output, "test output");
            assert_eq!(
                tool.outcome,
                if code == 0 {
                    Outcome::Success
                } else {
                    Outcome::Failure
                }
            );
            assert_eq!(
                tool.details,
                if code == 0 { vec![] } else { vec!["Exit 101"] }
            );
        }
    }
}

#[test]
fn tool_timing_is_frozen_when_complete_and_usage_remains_attributed_to_its_model() {
    let mut c = Conversation::default();
    c.apply(Provider::Codex,"item/started",&json!({"_created":1000,"item":{"id":"run","type":"commandExecution","command":"cargo test"}}));
    c.apply(Provider::Codex,"item/completed",&json!({"_created":3500,"item":{"id":"run","type":"commandExecution","status":"completed","exitCode":1}}));
    let tool = presentation::Tool::from_entry(&c.entries[0], 999999);
    assert_eq!(tool.timing, "2.5s");
    assert_eq!(tool.outcome, presentation::Outcome::Failure);
    c.apply(Provider::OpenCode,"session.step.started",&json!({"assistantMessageID":"answer","_created":100,"model":{"id":"model","providerID":"vendor"}}));
    c.apply(Provider::OpenCode,"session.step.ended",&json!({"assistantMessageID":"answer","_created":200,"tokens":{"input":100,"output":10,"cache":{"read":40,"write":5}}}));
    assert_eq!(c.used_tokens, Some(155));
    assert_eq!(c.usage_model.as_deref(), Some("vendor/model"));
    c.history(Provider::OpenCode,&[json!({"id":"old","type":"assistant","time":{"created":1},"tokens":{"input":1,"output":1,"cache":{"read":0,"write":0}}})],true);
    assert_eq!(c.used_tokens, Some(155));
}

#[test]
fn shell_tool_feed_hides_internal_ids_but_keeps_command_and_output() {
    let id = "sh_internal_fixture";
    let markup = format!(r#"<shell id="{id}" state="completed" command="cargo test">done</shell>"#);
    let mut conversation = Conversation::default();
    conversation.history(
        Provider::OpenCode,
        &[json!({
            "id":"message",
            "type":"assistant",
            "content":[{
                "id":"tool",
                "type":"tool",
                "name":"shell",
                "state":{
                    "status":"completed",
                    "input":markup.clone(),
                    "content":[{"type":"text","text":markup}]
                }
            }]
        })],
        false,
    );
    let tool = presentation::Tool::from_entry(&conversation.entries[0], 0);
    assert!(!tool.summary.contains(id));
    assert!(!tool.input.contains(id));
    assert!(!tool.output.contains(id));
    assert!(tool.summary.contains("cargo test"));
    assert!(tool.output.contains("state=\"completed\""));
    assert!(tool.output.contains("done"));
}

#[test]
fn tool_summary_omits_unrecognized_json_arguments() {
    let mut conversation = Conversation::default();
    conversation.history(
        Provider::OpenCode,
        &[json!({
            "id":"message",
            "type":"assistant",
            "content":[{
                "id":"tool",
                "type":"tool",
                "name":"custom_tool",
                "state":{
                    "status":"completed",
                    "input":{"options":{"enabled":true}},
                    "content":[]
                }
            }]
        })],
        false,
    );
    let tool = presentation::Tool::from_entry(&conversation.entries[0], 0);
    assert!(tool.summary.is_empty());
    assert!(tool.input.contains("enabled"));
}

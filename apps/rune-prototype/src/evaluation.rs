//! Deterministic demanding workload, explicitly opt-in and isolated from user data.
use crate::Workspace;
use caelum_core::application::{Event, Provider, Scope};
use serde_json::json;

pub fn workspace() -> Workspace {
    let mut state = Workspace::live(None);
    let live = state.live.as_mut().unwrap();
    live.evaluation = true;
    live.app.projects.push(caelum_protocol::ProjectInfo {
        name: "Evaluation workload".into(),
        path: "/evaluation".into(),
    });
    for i in 0..6 {
        let index = live.app.new_session(Scope {
            project: "/evaluation".into(),
            provider: Provider::Codex,
        });
        let session = &mut live.app.sessions[index];
        session.title = format!("Concurrent session {}", i + 1);
        session.remote_id = Some(format!("thread-{i}"));
        let turns: Vec<_> = (0..1_000).map(|n| json!({"id":format!("turn-{n}"),"status":"completed","items":[
            {"id":format!("user-{n}"),"type":"userMessage","content":[{"text":format!("Review change {n}, keeping the draft and reading position intact.")}]},
            {"id":format!("answer-{n}"),"type":"agentMessage","text":format!("## Change {n}\n\nThe implementation preserves **session ownership** while background streams continue.\n\n```rust\nlet owner = session.key();\n```\n\n- Check stale replies\n- Keep drafts independent")}
        ]})).collect();
        session.conversation.history(Provider::Codex, &turns, false);
    }
    let url = "https://github.com/example/evaluation/pull/1";
    let review = live
        .app
        .services
        .reviews
        .entry(("/evaluation".into(), url.into()))
        .or_default();
    review.details = Some(serde_json::from_value(json!({"number":1,"title":"Large review workload","url":url,"isDraft":true,"body":"Deterministic, offline review workload.","state":"OPEN","author":null,"baseRefName":"main","headRefName":"evaluation","headRefOid":"a".repeat(40),"mergeable":"UNKNOWN","mergeStateStatus":"UNKNOWN","additions":10000,"deletions":10000,"changedFiles":300,"reviewDecision":null,"statusCheckRollup":[]})).unwrap());
    review.files_loaded = true;
    review.files = (0..300)
        .map(|i| caelum_protocol::GitHubPullRequestFile {
            path: format!("src/file-{i}.rs"),
            status: "modified".into(),
            previous_path: None,
            additions: 100,
            deletions: 100,
            patch: Some(format!(
                "@@ -1,100 +1,100 @@\n{}",
                (0..100)
                    .map(|n| format!("-let value_{n} = {n};\n+let value_{n} = {};\n", n + 1))
                    .collect::<String>()
            )),
        })
        .collect();
    let checkout = live
        .app
        .services
        .checkouts
        .entry("/evaluation".into())
        .or_default();
    checkout
        .pull_requests
        .push(review.details.as_ref().unwrap().pull_request.clone());
    live.app.select(0);
    state
}

pub fn tick(app: &mut caelum_core::application::Workspace) {
    for i in 0..6 {
        app.apply(Event::Codex(caelum_protocol::CodexEvent { project_path: "/evaluation".into(), method: "item/agentMessage/delta".into(), params: json!({"threadId":format!("thread-{i}"),"itemId":"active-answer","delta":"Streaming another small update. "}), request_id: None }));
    }
}

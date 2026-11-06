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

/// The polished reference content through the actual live view, with no workers or disk writes.
pub fn design() -> Workspace {
    let mut state = Workspace::live(None);
    let live = state.live.as_mut().unwrap();
    live.fixture = true;
    let project = "/projects/caelum";
    let sample = &state.sample;
    live.app.projects.push(caelum_protocol::ProjectInfo {
        name: "caelum".into(),
        path: project.into(),
    });
    let scope = Scope {
        project: project.into(),
        provider: Provider::Codex,
    };
    for title in [&sample.title, "OpenCode session", &sample.secondary_title] {
        let i = live.app.new_session(scope.clone());
        live.app.sessions[i].title = title.into();
    }
    let session = &mut live.app.sessions[0];
    session.loaded = true;
    session.options.model = Some("Test model".into());
    session.options.effort = Some("Medium".into());
    session.draft = sample.draft.clone();
    session.conversation.history(Provider::Codex, &[json!({"status":"completed", "items":[
        {"id":"prompt", "type":"userMessage", "content":[{"text":sample.prompt}]},
        {"id":"tool", "type":"commandExecution", "command":sample.command, "aggregatedOutput":sample.output, "status":"completed"},
        {"id":"answer", "type":"agentMessage", "text":sample.response}
    ]})], false);
    live.app.choices.insert(scope, serde_json::from_value(json!({"models":[{"id":"Test model","name":"Test model","efforts":[{"id":"Medium","name":"Medium"}],"defaultEffort":"Medium"}],"agents":[],"defaultModel":"Test model"})).unwrap());
    let checkout = live
        .app
        .services
        .checkouts
        .entry(project.into())
        .or_default();
    checkout.git = Some(serde_json::from_value(json!({"projectPath":project,"repositoryRoot":project,"branch":"main","changes":[{"path":"workspace.css","indexStatus":" ","worktreeStatus":"M"}]})).unwrap());
    checkout.diffs.insert(
        "workspace.css".into(),
        serde_json::from_value(
            json!({"path":"workspace.css","staged":"","unstaged":sample.patch,"untracked":""}),
        )
        .unwrap(),
    );
    live.app.select(0);
    crate::panels::select_change(&mut state, project.into(), "workspace.css".into());
    state.live.as_mut().unwrap().panels.inspector = crate::panels::Screen::Changes;
    state
}

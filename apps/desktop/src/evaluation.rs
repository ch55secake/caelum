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
    checkout.git = Some(serde_json::from_value(json!({"projectPath":project,"repositoryRoot":project,"branch":"main","changes":[{"path":"src/workspace.css","indexStatus":" ","worktreeStatus":"M"}]})).unwrap());
    checkout.diffs.insert(
        "src/workspace.css".into(),
        serde_json::from_value(
            json!({"path":"src/workspace.css","staged":"","unstaged":sample.patch,"untracked":""}),
        )
        .unwrap(),
    );
    live.app.select(0);
    crate::panels::select_change(&mut state, project.into(), "src/workspace.css".into());
    state.live.as_mut().unwrap().panels.inspector = crate::panels::Screen::Changes;
    state
}

/// Populated regression scenes share the production renderer and use no services.
pub fn scene(state: &mut Workspace, name: &str) {
    let Some(l) = state.live.as_mut().filter(|l| l.fixture) else {
        return;
    };
    let project = "/projects/caelum";
    if name == "crowded" {
        for n in 0..30 {
            let i = l.app.new_session(Scope {
                project: project.into(),
                provider: Provider::Codex,
            });
            l.app.sessions[i].title = format!("Workspace task {}", n + 1);
        }
        l.app.archive(3, true);
        l.app.select(0);
        l.app.sessions[0].draft =
            "Preserve this multiline draft.\nKeep model controls below the editor.\n".repeat(12);
        l.sync_editors();
        l.drafts[0].replace_text(&l.app.sessions[0].draft);
        return;
    }
    if name == "tools" {
        let session = &mut l.app.sessions[0];
        session.conversation = Default::default();
        session.conversation.history(Provider::OpenCode,&[json!({"id":"prompt","type":"user","text":"Restore native workspace parity and verify the interactions."}),json!({"id":"work","type":"assistant","content":[
            {"id":"think","type":"reasoning","text":"Check the compact layout, preserve ownership, then verify the patch."},
            {"id":"read","type":"tool","name":"read","state":{"status":"completed","input":{"filePath":"src/workspace.rs"},"content":[{"type":"text","text":"pub fn restore_draft(session: &mut Session) {\n    session.editor.focus();\n}"}],"time":{"start":1000,"end":1250}}},
            {"id":"run","type":"tool","name":"bash","state":{"status":"completed","input":{"command":"cargo test --workspace"},"content":[{"type":"text","text":"test result: ok. 190 passed; 0 failed"}],"time":{"start":2000,"end":4520}}},
            {"id":"patch","type":"tool","name":"apply_patch","state":{"status":"completed","input":{"path":"src/workspace.rs","patch":"@@ -1 +1 @@\n-restore();\n+restore_owned_draft();"},"content":[{"type":"text","text":"Updated src/workspace.rs"}],"time":{"start":4600,"end":4800}}}
        ]})],false);
        for entry in &session.conversation.entries {
            if entry.kind != caelum_core::application::EntryKind::User {
                l.expanded.insert((0, entry.id.clone()));
            }
        }
        return;
    }
    if name == "images" {
        let attachment = caelum_core::application::Attachment::image(
            "Caelum.png".into(),
            include_bytes!("../assets/icon.png"),
        )
        .unwrap();
        l.app.attach(0, attachment.clone());
        l.app.sessions[0].conversation.history(Provider::OpenCode,&[json!({"id":"image","type":"user","text":"Use this icon in the native app.","files":[{"mime":"image/png","uri":attachment.content,"name":"Caelum.png"}]})],false);
        return;
    }
    let url = "https://github.com/ch55secake/caelum/pull/95";
    let review = l
        .app
        .services
        .reviews
        .entry((project.into(), url.into()))
        .or_default();
    review.details=Some(serde_json::from_value(json!({"number":95,"title":"Restore native workspace parity","url":url,"isDraft":false,"body":"Native composer, continuous review and durable session workflows.","state":"OPEN","author":{"login":"ch55secake"},"baseRefName":"feat/rune-design-parity","headRefName":"feat/rune-native-cutover","headRefOid":"a".repeat(40),"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","additions":34,"deletions":8,"changedFiles":3,"reviewDecision":"APPROVED","statusCheckRollup":[{"__typename":"CheckRun","name":"Native interactions","status":"COMPLETED","conclusion":"SUCCESS","detailsUrl":"https://github.com/ch55secake/caelum/actions","startedAt":"2026-11-07T10:00:00Z","completedAt":"2026-11-07T10:02:43Z"},{"__typename":"StatusContext","context":"Provider adapters","state":"SUCCESS","targetUrl":"https://github.com/ch55secake/caelum/actions"}]})).unwrap());
    review.files=vec![
        caelum_protocol::GitHubPullRequestFile {path:"src/workspace.rs".into(),status:"modified".into(),previous_path:None,additions:3,deletions:1,patch:Some("@@ -1,4 +1,6 @@\n pub fn restore(session: &mut Session) {\n-    session.reset();\n+    session.restore_draft();\n+    session.restore_reading_position();\n+    session.focus();\n }\n ".into())},
        caelum_protocol::GitHubPullRequestFile {path:"src/composer.rs".into(),status:"renamed".into(),previous_path:Some("src/input.rs".into()),additions:1,deletions:1,patch:Some("@@ -1 +1 @@\n-const HEIGHT: f32 = 56.0;\n+const MAX_HEIGHT: f32 = 180.0;".into())},
        caelum_protocol::GitHubPullRequestFile {path:"assets/icon.png".into(),status:"modified".into(),previous_path:None,additions:0,deletions:0,patch:None}
    ];
    review.files_loaded = true;
    review.threads=(0..2).map(|n|serde_json::from_value(json!({"id":format!("thread-{n}"),"path":"src/workspace.rs","line":n+2,"startLine":null,"originalLine":n+2,"diffSide":"RIGHT","isResolved":n==1,"isOutdated":false,"comments":[{"id":format!("comment-{n}"),"body":if n==0 {"Keep newer navigation intact while a restore is pending."} else {"Reading position now survives refresh. Looks good."},"url":url,"author":{"login":"reviewer"},"createdAt":"2026-11-07T10:00:00Z","state":null,"diffHunk":null}],"nextCursor":null})).unwrap()).collect();
    l.app
        .services
        .checkouts
        .get_mut(project)
        .unwrap()
        .pull_requests = vec![review.details.as_ref().unwrap().pull_request.clone()];
    l.app.services.revision += 1;
    if name == "mentions" {
        l.sync_editors();
        l.drafts[0].replace_text("Review #9 before sending this draft");
        l.drafts[0].set_caret(9);
        l.drafts[0].focused = true;
    } else {
        crate::panels::open_review(state, project.into(), url.into());
        if name == "checks" {
            state.live.as_mut().unwrap().panels.review_tab = 0;
        }
    }
}

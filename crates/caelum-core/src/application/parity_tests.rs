mod activity;
mod archive;
mod composer;
mod notifications;
mod queue;
mod review;
mod tools;
mod transcript;
mod worktrees;

use super::*;
use serde_json::json;

fn workspace() -> Workspace {
    let mut app = Workspace::default();
    for provider in [Provider::OpenCode, Provider::Codex] {
        let i = app.new_session(Scope {
            project: "/project".into(),
            provider,
        });
        app.sessions[i].remote_id = Some(format!("session-{i}"));
        app.sessions[i].loaded = true;
    }
    app.select(0);
    app.commands();
    app
}

fn opencode(method: &str, data: serde_json::Value) -> Event {
    Event::OpenCode(caelum_protocol::OpenCodeEvent {
        project_path: "/project".into(),
        event_type: method.into(),
        data,
        id: None,
        created: None,
        location: None,
    })
}

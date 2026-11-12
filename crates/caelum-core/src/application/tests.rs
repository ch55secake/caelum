mod archive;
mod checkouts;
mod controls;
mod history;
mod persistence;
mod projects;
mod review;
mod runtime;
mod sessions;
mod submission;
mod titles;
mod worktrees;

use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use caelum_protocol::{CodexEvent, OpenCodeEvent, ProjectInfo, SessionHistoryPage};
use serde_json::json;

use super::*;

fn scope(provider: Provider, project: &str) -> Scope {
    Scope {
        provider,
        project: project.into(),
    }
}

fn summary(scope: Scope, id: &str) -> Summary {
    Summary {
        updated_at: None,
        key: SessionKey {
            scope,
            id: id.into(),
        },
        title: "Saved session".into(),
        working: false,
        options: Options::default(),
    }
}

fn codex(project: &str, id: &str, method: &str, mut data: serde_json::Value) -> Event {
    data["threadId"] = id.into();
    Event::Codex(CodexEvent {
        project_path: project.into(),
        method: method.into(),
        params: data,
        request_id: None,
    })
}

fn page(data: Vec<serde_json::Value>, working: bool) -> Reply {
    Reply::History(SessionHistoryPage {
        data,
        next_cursor: None,
        is_working: Some(working),
        model: None,
        effort: None,
        agent: None,
    })
}

fn started(workspace: &mut Workspace, scope: Scope, remote: &str) -> usize {
    let index = workspace.new_session(scope.clone());
    assert!(workspace.send(index, "Hello".into()));
    let (request, _) = workspace.commands().pop().unwrap();
    workspace.apply(Event::Completed(
        request,
        Ok(Reply::Session(summary(scope, remote))),
    ));
    let (request, _) = workspace.commands().pop().unwrap();
    workspace.apply(Event::Completed(request, Ok(Reply::Done)));
    index
}

fn discover_session_list(workspace: &mut Workspace, scope: &Scope, archived: bool) -> u64 {
    workspace.discover_sessions(scope.clone());
    workspace
        .commands()
        .into_iter()
        .find(|(_, command)| match command {
            Command::List(current) => current == scope && !archived,
            Command::ListArchived(current) => current == scope && archived,
            _ => false,
        })
        .unwrap()
        .0
}

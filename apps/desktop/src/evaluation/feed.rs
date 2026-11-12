use crate::{Workspace, live::Live};
use caelum_core::application::{Provider, Scope, SessionScope};

const NOW: u64 = 1_800_000_000_000;

pub(super) fn scene(state: &mut Workspace, name: &str) -> bool {
    if !matches!(
        name,
        "session-feed"
            | "session-feed-current"
            | "session-feed-empty"
            | "session-feed-many"
            | "session-feed-colours"
    ) {
        return false;
    }
    let l = state.live.as_mut().unwrap();
    l.feed.now = NOW;
    for (i, session) in l.app.sessions.iter_mut().enumerate() {
        session.last_activity = Some(NOW - (i as u64 + 1) * 900_000);
    }
    let mut projects = vec![
        (
            "/projects/rune",
            "rune",
            "Keep keyboard focus in the editor",
            "fix/editor-focus",
            Provider::Codex,
            2,
        ),
        (
            "/projects/website",
            "website",
            "Ship the refreshed landing page",
            "feat/homepage",
            Provider::OpenCode,
            7,
        ),
        (
            "/projects/tools",
            "developer-tools",
            "Make local builds incremental",
            "build/dev-app",
            Provider::Codex,
            45,
        ),
    ];
    if name == "session-feed-colours" {
        projects.extend([
            (
                "/projects/glance",
                "glance",
                "Polish the dashboard",
                "feat/dashboard",
                Provider::Codex,
                1,
            ),
            (
                "/projects/singularity",
                "singularity",
                "Keep workspace state consistent",
                "fix/state",
                Provider::OpenCode,
                3,
            ),
        ]);
    }
    for (path, name, title, branch, provider, age) in projects {
        l.app.projects.push(caelum_protocol::ProjectInfo {
            name: name.into(),
            path: path.into(),
        });
        let i = l.app.new_session(Scope {
            project: path.into(),
            provider,
        });
        let session = &mut l.app.sessions[i];
        session.title = title.into();
        session.remote_id = Some(format!("fixture-{i}"));
        session.last_activity = Some(NOW - age * 60_000);
        session.details = Some(caelum_protocol::SessionDetails {
            branch: Some(branch.into()),
            branches: vec![branch.into()],
            branch_is_current: true,
            changes: None,
        });
        session.unread = age == 7;
        let checkout = l.app.services.checkouts.entry(path.into()).or_default();
        checkout.git = Some(caelum_protocol::GitRepositoryStatus {
            project_path: path.into(),
            repository_root: Some(path.into()),
            branch: Some(branch.into()),
            changes: vec![],
        });
        checkout
            .pull_requests
            .push(caelum_protocol::GitHubPullRequest {
                number: 42,
                title: title.into(),
                url: format!("https://github.com/example/{name}/pull/42"),
                is_draft: false,
                base_ref_name: "main".into(),
            });
    }
    if name == "session-feed-current" {
        l.app.preferences.session_scope = SessionScope::CurrentProject;
    }
    if name == "session-feed-empty" {
        l.panels.sessions.text = "No matching session".into();
    }
    if name == "session-feed-many" {
        many(l, 2000);
    }
    l.app.select(0);
    l.sync_editors();
    true
}

fn many(l: &mut Live, count: usize) {
    for n in 0..count {
        let i = l.app.new_session(Scope {
            project: "/projects/rune".into(),
            provider: Provider::Codex,
        });
        l.app.sessions[i].title = format!("Historical session {n}");
        l.app.sessions[i].last_activity = Some(NOW - (n as u64 + 1) * 86_400_000);
    }
}

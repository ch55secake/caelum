//! Native preview registrations, shared by the app and interaction harness.
use super::scene;
use crate::{Workspace, panels};

type Scene = (&'static str, fn(&mut Workspace));

pub(crate) const SCENES: &[Scene] = &[
    ("native-connected", |s| {
        crate::remote::scene(s, "native-connected")
    }),
    ("native-offline", |s| {
        crate::remote::scene(s, "native-offline")
    }),
    ("remote-creating", |s| {
        crate::remote::scene(s, "remote-creating")
    }),
    ("remote-working", |s| {
        crate::remote::scene(s, "remote-working")
    }),
    ("remote-trace", |s| crate::remote::scene(s, "remote-trace")),
    ("remote-setup-failed", |s| {
        crate::remote::scene(s, "remote-setup-failed")
    }),
    ("remote-stopped", |s| {
        crate::remote::scene(s, "remote-stopped")
    }),
    ("remote-create", |s| {
        crate::remote::scene(s, "remote-create")
    }),
    ("remote-handoff", |s| {
        crate::remote::scene(s, "remote-handoff")
    }),
    ("remote-machine", |s| {
        crate::remote::scene(s, "remote-machine")
    }),
    ("remote-model-draft", |s| {
        crate::remote::scene(s, "remote-model-draft")
    }),
    ("remote-changes", |s| {
        crate::remote::scene(s, "remote-changes")
    }),
    ("remote-picker", |s| {
        crate::remote::scene(s, "remote-picker")
    }),
    ("remote-settings", |s| {
        crate::remote::scene(s, "remote-settings")
    }),
    ("subagent-running", |s| {
        crate::subagents::scene(s, "running")
    }),
    ("subagent-completed", |s| {
        crate::subagents::scene(s, "completed")
    }),
    ("subagent-failed", |s| crate::subagents::scene(s, "error")),
    ("update-downloading", |s| {
        crate::updates::scene(s, "downloading")
    }),
    ("update-verifying", |s| {
        crate::updates::scene(s, "verifying")
    }),
    ("update-preparing", |s| {
        crate::updates::scene(s, "preparing")
    }),
    ("update-cancelling", |s| {
        crate::updates::scene(s, "cancelling")
    }),
    ("update-failed", |s| crate::updates::scene(s, "failed")),
    ("update-success", |s| crate::updates::scene(s, "updated")),
    ("home", |s| {
        crate::home::scene(s, "home");
    }),
    ("home-empty", |s| {
        crate::home::scene(s, "home-empty");
    }),
    ("home-draft", |s| {
        crate::home::scene(s, "home-draft");
    }),
    ("home-projects", |s| {
        crate::home::scene(s, "home-projects");
    }),
    ("composer-options", |s| scene(s, "composer-options")),
    ("composer-empty", |s| scene(s, "composer-empty")),
    ("composer-multiline", |s| scene(s, "composer-multiline")),
    ("composer-queued", |s| scene(s, "composer-queued")),
    ("composer-queue-error", |s| scene(s, "composer-queue-error")),
    ("composer-attachments", |s| scene(s, "composer-attachments")),
    ("composer-long", |s| scene(s, "composer-long")),
    ("composer-attachment-status", |s| {
        scene(s, "composer-attachment-status")
    }),
    ("composer-attachment-preview", |s| {
        scene(s, "composer-attachment-preview")
    }),
    ("composer-image-preview", |s| {
        scene(s, "composer-image-preview")
    }),
    ("files-tabs", |s| scene(s, "files-tabs")),
    ("tools-collapsed", |s| scene(s, "tools-collapsed")),
    ("tool-running", |s| scene(s, "tool-running")),
    ("tool-failure", |s| scene(s, "tool-failure")),
    ("tool-output-long", |s| scene(s, "tool-output-long")),
    ("vercel-tool-output-long", |s| {
        scene(s, "vercel-tool-output-long")
    }),
    ("tool-search", |s| scene(s, "tool-search")),
    ("vercel-tools", |s| scene(s, "vercel-tools")),
    ("compaction", |s| scene(s, "compaction")),
    ("compaction-running", |s| scene(s, "compaction-running")),
    ("compaction-empty", |s| scene(s, "compaction-empty")),
    ("compaction-error", |s| scene(s, "compaction-error")),
    ("compaction-interrupted", |s| {
        scene(s, "compaction-interrupted")
    }),
    ("vercel-compaction", |s| scene(s, "vercel-compaction")),
    ("patch-short", |s| scene(s, "patch-short")),
    ("patch-long", |s| scene(s, "patch-long")),
    ("patch-multi", |s| scene(s, "patch-multi")),
    ("vercel-patch-short", |s| scene(s, "vercel-patch-short")),
    ("vercel-patch-long", |s| scene(s, "vercel-patch-long")),
    ("vercel-patch-multi", |s| scene(s, "vercel-patch-multi")),
    ("merge-ready", |s| scene(s, "merge-ready")),
    ("merge-blocked", |s| scene(s, "merge-blocked")),
    ("merge-pending", |s| scene(s, "merge-pending")),
    ("merge-conflicts", |s| scene(s, "merge-conflicts")),
    ("merge-behind", |s| scene(s, "merge-behind")),
    ("merge-draft", |s| scene(s, "merge-draft")),
    ("merge-merged", |s| scene(s, "merge-merged")),
    ("merge-closed", |s| scene(s, "merge-closed")),
    ("merge-auto-enabled", |s| scene(s, "merge-auto-enabled")),
    ("merge-auto-error", |s| scene(s, "merge-auto-error")),
    ("worktree-create", |s| scene(s, "worktree-create")),
    ("shell-output", |s| scene(s, "shell-output")),
    ("shell-completed", |s| completed_shell(s, false)),
    ("shell-output-completed", |s| completed_shell(s, true)),
    ("shells-many", |s| scene(s, "shells-many")),
    ("shells-collapsed", |s| scene(s, "shells-collapsed")),
    ("vercel", |s| scene(s, "vercel")),
    ("vercel-settings", |s| scene(s, "vercel-settings")),
    ("sidebar-projects", |s| scene(s, "sidebar-projects")),
    ("session-feed", |s| scene(s, "session-feed")),
    ("session-review-feedback", |s| {
        scene(s, "session-review-feedback")
    }),
    ("session-feed-colours", |s| scene(s, "session-feed-colours")),
    ("session-feed-current", |s| scene(s, "session-feed-current")),
    ("session-feed-empty", |s| scene(s, "session-feed-empty")),
    ("session-feed-many", |s| scene(s, "session-feed-many")),
    ("messages", |s| scene(s, "messages")),
    ("archive", |s| scene(s, "archive")),
    ("overview", |s| scene(s, "overview")),
    ("overview-empty", |s| scene(s, "overview-empty")),
    ("overview-blocked", |s| scene(s, "overview-blocked")),
    ("overview-long", |s| scene(s, "overview-long")),
    ("overview-stale", |s| scene(s, "overview-stale")),
    ("overview-cached", |s| scene(s, "overview-cached")),
    ("worktrees", |s| scene(s, "worktrees")),
    ("sidebar-worktrees", |s| scene(s, "sidebar-worktrees")),
    ("sidebar-worktrees-many", |s| {
        scene(s, "sidebar-worktrees-many")
    }),
    ("sidebar-worktrees-projects", |s| {
        scene(s, "sidebar-worktrees-projects")
    }),
    ("sidebar-worktrees-vercel", |s| {
        scene(s, "sidebar-worktrees-vercel")
    }),
    ("worktrees-empty", |s| scene(s, "worktrees-empty")),
    ("worktrees-loading", |s| scene(s, "worktrees-loading")),
    ("worktrees-error", |s| scene(s, "worktrees-error")),
    ("provider-error", |s| scene(s, "provider-error")),
    ("provider-timeout", |s| provider_failure(s, false)),
    ("provider-rejection", |s| provider_failure(s, true)),
    ("models-empty", |s| scene(s, "models-empty")),
    ("usage-loading", |s| scene(s, "usage-loading")),
    ("files", |s| scene(s, "files")),
    ("files-loading", |s| scene(s, "files-loading")),
    ("changes-empty", |s| scene(s, "changes-empty")),
    ("approval", |s| scene(s, "approval")),
    ("questions", |s| scene(s, "questions")),
    ("image-preview", |s| scene(s, "image-preview")),
    ("review-error", |s| scene(s, "review-error")),
    ("providers", |s| scene(s, "providers")),
    ("notifications", |s| scene(s, "notifications")),
    ("storage", |s| scene(s, "storage")),
    ("about", |s| scene(s, "about")),
    ("about-checking", |s| scene(s, "about-checking")),
    ("about-current", |s| scene(s, "about-current")),
    ("about-update", |s| scene(s, "about-update")),
    ("about-release", |s| scene(s, "about-release")),
    ("about-ahead", |s| scene(s, "about-ahead")),
    ("about-error", |s| scene(s, "about-error")),
    ("vercel-about", |s| scene(s, "vercel-about")),
    ("vercel-about-update", |s| scene(s, "vercel-about-update")),
    ("models", |s| scene(s, "models")),
    ("models-settings", |s| {
        super::models::scene(s, "models-settings")
    }),
    ("launcher", |s| scene(s, "launcher")),
    ("session", |s| scene(s, "session")),
    ("fork", |s| {
        history(s, caelum_core::application::HistoryAction::Fork)
    }),
    ("edit-branch", |s| {
        history(s, caelum_core::application::HistoryAction::Edit)
    }),
    ("rewind-checkpoint", |s| {
        history(s, caelum_core::application::HistoryAction::Rewind)
    }),
    ("rename-session", |s| {
        history(s, caelum_core::application::HistoryAction::Fork);
        if let Some(index) = s.live.as_ref().and_then(|l| l.app.selected) {
            crate::commands::rename(s, index);
        }
    }),
    ("timeline", |s| scene(s, "timeline")),
    ("usage", |s| scene(s, "usage")),
    ("stats", |s| scene(s, "stats")),
    ("stats-empty", |s| scene(s, "stats-empty")),
    ("stats-loading", |s| scene(s, "stats-loading")),
    ("stats-error", |s| scene(s, "stats-error")),
    ("timeline-empty", |s| scene(s, "timeline-empty")),
    ("working", |s| scene(s, "working")),
    ("thinking", |s| scene(s, "thinking")),
    ("retrying", |s| scene(s, "retrying")),
    ("stopping", |s| scene(s, "stopping")),
    ("commands", |s| scene(s, "commands")),
    ("shells", |s| scene(s, "shells")),
    ("empty", |s| scene(s, "empty")),
    ("error", |s| scene(s, "error")),
    ("discussion", |s| scene(s, "discussion")),
    ("review-loading", |s| scene(s, "review-loading")),
    ("tools", |s| scene(s, "tools")),
    ("review", |s| scene(s, "review")),
    ("checks", |s| scene(s, "checks")),
    ("mentions", |s| scene(s, "mentions")),
    ("images", |s| scene(s, "images")),
    ("crowded", |s| scene(s, "crowded")),
    ("inspector", |s| s.inspector.set_open(true)),
    ("compact", |s| {
        s.sidebar.set_open(false);
        s.inspector.set_open(true);
        s.inspector.set_width(300.0);
    }),
    ("settings", |s| {
        if s.live.is_some() {
            panels::show(s, panels::Screen::Settings);
        }
    }),
];

fn history(state: &mut Workspace, action: caelum_core::application::HistoryAction) {
    use caelum_core::application::{Command, Event, Provider, Reply, Scope};
    use serde_json::json;
    let l = state.live.as_mut().unwrap();
    let path = l.app.projects.first().unwrap().path.clone();
    let index = l.app.new_session(Scope {
        project: path,
        provider: Provider::OpenCode,
    });
    l.app.sessions[index].remote_id = Some("ses_fork_preview".into());
    l.sync_editors();
    if !l.app.open_history_action(index, action, None) {
        return;
    }
    let id = l
        .app
        .commands()
        .into_iter()
        .find(|(_, c)| matches!(c,Command::History(k,None) if k.id=="ses_fork_preview"))
        .unwrap()
        .0;
    l.app.apply(Event::Completed(
        id,
        Ok(Reply::History(caelum_protocol::SessionHistoryPage {
            data: [
                "Sketch the project sidebar",
                "Keep worktrees grouped with their project",
                "Show how to recover a moved conversation",
            ]
            .iter()
            .enumerate()
            .map(|(i, text)| json!({"id":format!("msg_preview_{i}"),"type":"user","text":text}))
            .collect(),
            next_cursor: Some("older".into()),
            is_working: Some(false),
            model: None,
            effort: None,
            agent: None,
        })),
    ));
    l.commands.owner = Some(index);
    l.commands.search.focused = true;
    l.panels.overlay = Some(panels::Screen::Fork);
    if action == caelum_core::application::HistoryAction::Rewind {
        let boundary = l.app.fork_picker().unwrap().points[1].boundary.clone();
        l.app.fork_at(&boundary);
        let id = l
            .app
            .commands()
            .into_iter()
            .find(|(_, c)| matches!(c, Command::CheckpointFiles(..)))
            .unwrap()
            .0;
        l.app.apply(Event::Completed(
            id,
            Ok(Reply::CheckpointFiles(vec![
                "src/sidebar.rs".into(),
                "src/worktrees.rs".into(),
            ])),
        ));
    }
}

fn completed_shell(state: &mut Workspace, output: bool) {
    use caelum_core::application::Event;
    use serde_json::json;

    scene(state, if output { "shell-output" } else { "shells" });
    let l = state.live.as_mut().unwrap();
    l.apply(Event::OpenCode(caelum_protocol::OpenCodeEvent {
        project_path: l.app.sessions[0].scope.project.clone(),
        event_type: "session.synthetic".into(),
        id: Some("evt_shell_finished".into()), created: Some(2000), location: None,
        data: json!({
            "sessionID": "ses_fixture", "description": "cargo test --workspace",
            "metadata": {"source": "shell", "shellID": "shell_fixture", "state": "completed", "exit": 0},
            "text": "<shell id=\"shell_fixture\" state=\"completed\" command=\"cargo test --workspace\">\nCompiling workspace…\nFinished test profile\ntest application::queue_recovery ... ok\ntest application::draft_café ... ok\n\ntest result: ok. 162 passed; 0 failed\n</shell>"
        }),
    }));
    l.expanded
        .insert((0, "opencode-shell-shell_fixture".into()));
}

fn provider_failure(s: &mut Workspace, rejected: bool) {
    use caelum_core::application::Provider;
    use serde_json::json;

    let l = s.live.as_mut().unwrap();
    let session = &mut l.app.sessions[0];
    session.scope.provider = Provider::OpenCode;
    session.conversation = Default::default();
    session.conversation.history(
        Provider::OpenCode,
        &[json!({"id":"prompt", "type":"user", "text":"Review the workspace changes."})],
        false,
    );
    let error = if rejected {
        json!({"type":"provider.invalid-request", "message":"Unsupported feature",
            "response":{"body":json!({"error":{"code":"subscription_sharing_unsupported_capability"}}).to_string()}})
    } else {
        json!({"type":"provider.transport", "status":200, "message":"Timed out waiting for response data"})
    };
    session.conversation.apply(
        Provider::OpenCode,
        "session.execution.failed",
        &json!({"error":error}),
    );
    session.draft = "Keep the next prompt editable.".into();
    l.sync_editors();
}

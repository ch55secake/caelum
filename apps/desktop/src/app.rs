use crate::{Workspace, evaluation, live, on_key, panels, state, view};
use rune::AppSpec;

pub(super) fn spec() -> AppSpec<Workspace> {
    AppSpec {
        min_size: Some((800.0, 620.0)),
        families: ("system-ui", "ui-monospace"),
        poll: Some(state::poll),
        setup: Some(live::setup),
        on_key: Some(on_key),
        scenes: &[
            ("composer-options", |s| {
                evaluation::scene(s, "composer-options")
            }),
            ("composer-empty", |s| evaluation::scene(s, "composer-empty")),
            ("composer-multiline", |s| {
                evaluation::scene(s, "composer-multiline")
            }),
            ("composer-queued", |s| {
                evaluation::scene(s, "composer-queued")
            }),
            ("composer-queue-error", |s| {
                evaluation::scene(s, "composer-queue-error")
            }),
            ("composer-attachments", |s| {
                evaluation::scene(s, "composer-attachments")
            }),
            ("composer-long", |s| evaluation::scene(s, "composer-long")),
            ("composer-attachment-status", |s| {
                evaluation::scene(s, "composer-attachment-status")
            }),
            ("composer-attachment-preview", |s| {
                evaluation::scene(s, "composer-attachment-preview")
            }),
            ("composer-image-preview", |s| {
                evaluation::scene(s, "composer-image-preview")
            }),
            ("files-tabs", |s| evaluation::scene(s, "files-tabs")),
            ("tools-collapsed", |s| {
                evaluation::scene(s, "tools-collapsed")
            }),
            ("tool-running", |s| evaluation::scene(s, "tool-running")),
            ("tool-failure", |s| evaluation::scene(s, "tool-failure")),
            ("tool-output-long", |s| {
                evaluation::scene(s, "tool-output-long")
            }),
            ("vercel-tool-output-long", |s| {
                evaluation::scene(s, "vercel-tool-output-long")
            }),
            ("tool-search", |s| evaluation::scene(s, "tool-search")),
            ("vercel-tools", |s| evaluation::scene(s, "vercel-tools")),
            ("compaction", |s| evaluation::scene(s, "compaction")),
            ("compaction-running", |s| {
                evaluation::scene(s, "compaction-running")
            }),
            ("compaction-empty", |s| {
                evaluation::scene(s, "compaction-empty")
            }),
            ("compaction-error", |s| {
                evaluation::scene(s, "compaction-error")
            }),
            ("compaction-interrupted", |s| {
                evaluation::scene(s, "compaction-interrupted")
            }),
            ("vercel-compaction", |s| {
                evaluation::scene(s, "vercel-compaction")
            }),
            ("patch-short", |s| evaluation::scene(s, "patch-short")),
            ("patch-long", |s| evaluation::scene(s, "patch-long")),
            ("patch-multi", |s| evaluation::scene(s, "patch-multi")),
            ("vercel-patch-short", |s| {
                evaluation::scene(s, "vercel-patch-short")
            }),
            ("vercel-patch-long", |s| {
                evaluation::scene(s, "vercel-patch-long")
            }),
            ("vercel-patch-multi", |s| {
                evaluation::scene(s, "vercel-patch-multi")
            }),
            ("merge-ready", |s| evaluation::scene(s, "merge-ready")),
            ("merge-blocked", |s| evaluation::scene(s, "merge-blocked")),
            ("merge-pending", |s| evaluation::scene(s, "merge-pending")),
            ("merge-conflicts", |s| {
                evaluation::scene(s, "merge-conflicts")
            }),
            ("merge-behind", |s| evaluation::scene(s, "merge-behind")),
            ("merge-draft", |s| evaluation::scene(s, "merge-draft")),
            ("merge-merged", |s| evaluation::scene(s, "merge-merged")),
            ("merge-closed", |s| evaluation::scene(s, "merge-closed")),
            ("worktree-create", |s| {
                evaluation::scene(s, "worktree-create")
            }),
            ("shell-output", |s| evaluation::scene(s, "shell-output")),
            ("shells-many", |s| evaluation::scene(s, "shells-many")),
            ("shells-collapsed", |s| {
                evaluation::scene(s, "shells-collapsed")
            }),
            ("vercel", |s| evaluation::scene(s, "vercel")),
            ("vercel-settings", |s| {
                evaluation::scene(s, "vercel-settings")
            }),
            ("sidebar-projects", |s| {
                evaluation::scene(s, "sidebar-projects")
            }),
            ("session-feed", |s| evaluation::scene(s, "session-feed")),
            ("session-feed-colours", |s| {
                evaluation::scene(s, "session-feed-colours")
            }),
            ("session-feed-current", |s| {
                evaluation::scene(s, "session-feed-current")
            }),
            ("session-feed-empty", |s| {
                evaluation::scene(s, "session-feed-empty")
            }),
            ("session-feed-many", |s| {
                evaluation::scene(s, "session-feed-many")
            }),
            ("messages", |s| evaluation::scene(s, "messages")),
            ("archive", |s| evaluation::scene(s, "archive")),
            ("overview", |s| evaluation::scene(s, "overview")),
            ("overview-empty", |s| evaluation::scene(s, "overview-empty")),
            ("overview-blocked", |s| {
                evaluation::scene(s, "overview-blocked")
            }),
            ("worktrees", |s| evaluation::scene(s, "worktrees")),
            ("sidebar-worktrees", |s| {
                evaluation::scene(s, "sidebar-worktrees")
            }),
            ("sidebar-worktrees-many", |s| {
                evaluation::scene(s, "sidebar-worktrees-many")
            }),
            ("sidebar-worktrees-projects", |s| {
                evaluation::scene(s, "sidebar-worktrees-projects")
            }),
            ("sidebar-worktrees-vercel", |s| {
                evaluation::scene(s, "sidebar-worktrees-vercel")
            }),
            ("worktrees-empty", |s| {
                evaluation::scene(s, "worktrees-empty")
            }),
            ("worktrees-loading", |s| {
                evaluation::scene(s, "worktrees-loading")
            }),
            ("worktrees-error", |s| {
                evaluation::scene(s, "worktrees-error")
            }),
            ("provider-error", |s| evaluation::scene(s, "provider-error")),
            ("models-empty", |s| evaluation::scene(s, "models-empty")),
            ("usage-loading", |s| evaluation::scene(s, "usage-loading")),
            ("files", |s| evaluation::scene(s, "files")),
            ("files-loading", |s| evaluation::scene(s, "files-loading")),
            ("changes-empty", |s| evaluation::scene(s, "changes-empty")),
            ("approval", |s| evaluation::scene(s, "approval")),
            ("questions", |s| evaluation::scene(s, "questions")),
            ("image-preview", |s| evaluation::scene(s, "image-preview")),
            ("review-error", |s| evaluation::scene(s, "review-error")),
            ("providers", |s| evaluation::scene(s, "providers")),
            ("notifications", |s| evaluation::scene(s, "notifications")),
            ("storage", |s| evaluation::scene(s, "storage")),
            ("about", |s| evaluation::scene(s, "about")),
            ("about-checking", |s| evaluation::scene(s, "about-checking")),
            ("about-current", |s| evaluation::scene(s, "about-current")),
            ("about-update", |s| evaluation::scene(s, "about-update")),
            ("about-release", |s| evaluation::scene(s, "about-release")),
            ("about-ahead", |s| evaluation::scene(s, "about-ahead")),
            ("about-error", |s| evaluation::scene(s, "about-error")),
            ("vercel-about", |s| evaluation::scene(s, "vercel-about")),
            ("vercel-about-update", |s| {
                evaluation::scene(s, "vercel-about-update")
            }),
            ("models", |s| evaluation::scene(s, "models")),
            ("launcher", |s| evaluation::scene(s, "launcher")),
            ("session", |s| evaluation::scene(s, "session")),
            ("timeline", |s| evaluation::scene(s, "timeline")),
            ("usage", |s| evaluation::scene(s, "usage")),
            ("stats", |s| evaluation::scene(s, "stats")),
            ("stats-empty", |s| evaluation::scene(s, "stats-empty")),
            ("stats-loading", |s| evaluation::scene(s, "stats-loading")),
            ("stats-error", |s| evaluation::scene(s, "stats-error")),
            ("timeline-empty", |s| evaluation::scene(s, "timeline-empty")),
            ("working", |s| evaluation::scene(s, "working")),
            ("thinking", |s| evaluation::scene(s, "thinking")),
            ("retrying", |s| evaluation::scene(s, "retrying")),
            ("stopping", |s| evaluation::scene(s, "stopping")),
            ("commands", |s| evaluation::scene(s, "commands")),
            ("shells", |s| evaluation::scene(s, "shells")),
            ("empty", |s| evaluation::scene(s, "empty")),
            ("error", |s| evaluation::scene(s, "error")),
            ("discussion", |s| evaluation::scene(s, "discussion")),
            ("review-loading", |s| evaluation::scene(s, "review-loading")),
            ("tools", |s| evaluation::scene(s, "tools")),
            ("review", |s| evaluation::scene(s, "review")),
            ("checks", |s| evaluation::scene(s, "checks")),
            ("mentions", |s| evaluation::scene(s, "mentions")),
            ("images", |s| evaluation::scene(s, "images")),
            ("crowded", |s| evaluation::scene(s, "crowded")),
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
        ],
        ..AppSpec::new("Caelum", (1280.0, 1000.0), Workspace::new, view)
    }
}

//! Native preview registrations, shared by the app and interaction harness.
use super::scene;
use crate::{Workspace, panels};

type Scene = (&'static str, fn(&mut Workspace));

pub(crate) const SCENES: &[Scene] = &[
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
    ("shells-many", |s| scene(s, "shells-many")),
    ("shells-collapsed", |s| scene(s, "shells-collapsed")),
    ("vercel", |s| scene(s, "vercel")),
    ("vercel-settings", |s| scene(s, "vercel-settings")),
    ("sidebar-projects", |s| scene(s, "sidebar-projects")),
    ("session-feed", |s| scene(s, "session-feed")),
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
    ("launcher", |s| scene(s, "launcher")),
    ("session", |s| scene(s, "session")),
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

#!/usr/bin/env python3
"""Capture the real native views with deterministic design data."""
from pathlib import Path
import json
import subprocess
import sys

root = Path(__file__).resolve().parents[3]
output = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else root / "apps/desktop/preview-results"
output.mkdir(parents=True, exist_ok=True)
subprocess.run(["cargo", "build", "--locked", "-p", "caelum-desktop"], check=True, cwd=root)
binary = root / "target/debug/caelum-desktop"
scenes = ("", "inspector", "compact", "settings", "providers", "provider-error", "notifications", "storage", "about", "models", "models-empty", "launcher", "session", "timeline", "timeline-empty", "usage", "usage-loading", "stats", "stats-empty", "stats-loading", "stats-error", "commands", "shells", "working", "thinking", "retrying", "stopping", "empty", "error", "review-loading", "review-error", "discussion", "tools", "review", "checks", "mentions", "images", "image-preview", "files", "files-loading", "changes-empty", "approval", "questions", "crowded", "worktrees", "worktrees-loading", "worktrees-error")
interactions = {
    "session-feed-filter": ("session-feed", [{"t": 0.25, "key": "session-project-filter-trigger"}]),
    "session-feed-search": ("session-feed", [{"t": 0.25, "key": "session-search"}, {"t": 0.4, "paste": "focus"}]),
    "session-feed-project": ("session-feed", [{"t": 0.25, "key": "session-project-filter-trigger"}, {"t": 0.5, "key": "option2"}]),
    "composer-focus": ("", [{"t": 0.25, "key": "live-draft-0"}]),
    "check-details": ("merge-ready", [{"t": 0.25, "key": "review-checks-toggle"}]),
    "worktree-base": ("worktree-create", [{"t": 0.25, "key": "worktree-base-trigger"}]),
    "user-copy": ("messages", [{"t": 0.25, "type": "move", "key": "copy-user-0-opencode-formatted"}]),
    "merge-method": ("checks", [{"t": 0.15, "type": "scroll", "key": "review-overview-scroll", "dy": -130, "dur": 0.1}, {"t": 0.5, "key": "merge-method-trigger"}]),
    "provider-dropdown": ("providers", [{"t": 0.25, "key": "Default effort-trigger"}]),
    "model-hover": ("models", [{"t": 0.25, "type": "move", "key": "model-favorite-1"}]),
    "model-focus": ("models", [{"t": 0.25, "type": "key", "name": "Tab"}]),
    "settings-hover": ("settings", [{"t": 0.25, "type": "move", "key": "settings-providers"}]),
    "worktrees-form": ("worktree-create", []),
    "composer-agent": ("composer-options", [{"t": 0.25, "key": "agent-trigger"}]),
    "composer-effort": ("composer-options", [{"t": 0.25, "key": "effort-trigger"}]),
    "composer-queue-expanded": ("composer-queued", [{"t": 0.25, "key": "toggle-queued-messages"}]),
    "composer-grow": ("composer-empty", [{"t": 0.15, "key": "live-draft-0"}, {"t": 0.25, "paste": "Keep the conversation visible above the editor.\nLet the draft grow naturally as I write.\nPreserve selection and undo history.\n\nReview the captured file snapshots before sending.\nUse the same syntax colours as the file viewer.\nKeep the toolbar quiet and compact.\n"}]),
    "composer-preview": ("composer-attachment-status", [{"t": 0.25, "key": "composer-attachment-source"}]),
    "composer-remove": ("composer-attachments", [{"t": 0.25, "key": "remove-attachment-source"}]),
    "context-session": ("", [{"t": 0.25, "type": "secondary-click", "key": "live-session-1"}]),
    "context-editor": ("", [{"t": 0.15, "key": "live-draft-0"}, {"t": 0.3, "type": "key", "name": "a", "cmd": True}, {"t": 0.45, "type": "secondary-click", "key": "live-draft-0"}]),
    "context-file": ("files", [{"t": 0.25, "type": "secondary-click", "key": "file-tree-Cargo.toml"}]),
    "session-hover": ("session", [{"t": 0.25, "type": "move", "key": "archive-session"}]),
    "stats-details": ("stats", [{"t": 0.25, "type": "scroll", "key": "command-results", "dy": -420, "dur": 0.2}]),
    "stats-projects": ("stats", [{"t": 0.25, "key": "stats-project-trigger"}]),
    "timeline-search": ("timeline", [{"t": 0.25, "key": "timeline-search"}, {"t": 0.3, "paste": "draft"}]),
    "review-navigation": ("review", [{"t": 0.25, "key": "review-nav-toggle"}]),
    "shells-hide": ("shells-many", [{"t": 0.25, "key": "toggle-background-shells"}]),
    "shells-reopen": ("shells-collapsed", [{"t": 0.25, "key": "toggle-background-shells"}]),
    "vercel-select": ("settings", [{"t": 0.25, "key": "family-vercel"}]),
    "vercel-providers": ("vercel-settings", [{"t": 0.25, "key": "settings-providers"}]),
    "vercel-notifications": ("vercel-settings", [{"t": 0.25, "key": "settings-notifications"}]),
    "patch-collapsed": ("vercel-patch-multi", [{"t": 0.25, "key": "tool-0-opencode-tool-patch"}]),
    "patch-expanded": ("vercel-patch-long", [{"t": 0.8, "key": "expand-patch-0-opencode-tool-patch"}]),
    "compaction-summary": ("compaction", [{"t": 0.25, "key": "compaction-0-opencode-compact"}]),
    "tool-error-expanded": ("tool-failure", [{"t": 0.25, "key": "tool-0-opencode-tool-run"}, {"t": 0.5, "type": "move", "x": 0, "y": 0}]),
    "tool-output-full": ("tool-output-long", [{"t": 0.25, "key": "tool-output-0-opencode-tool-run-Output-expand"}]),
}
scenes += ("worktree-create", "shell-output", "shells-many", "messages", "archive", "overview", "overview-empty", "overview-blocked")
scenes += ("composer-options", "files-tabs", "tools-collapsed", "merge-ready", "merge-blocked", "merge-pending", "merge-conflicts", "merge-behind", "merge-draft", "merge-merged", "merge-closed")
scenes += ("shells-collapsed", "vercel", "vercel-settings", "sidebar-projects")
scenes += ("session-feed", "session-feed-current", "session-feed-empty", "session-feed-many")
scenes += ("sidebar-worktrees", "sidebar-worktrees-many", "sidebar-worktrees-projects", "sidebar-worktrees-vercel", "worktrees-empty")
scenes += ("patch-short", "patch-long", "patch-multi", "vercel-patch-short", "vercel-patch-long", "vercel-patch-multi")
scenes += ("about-checking", "about-current", "about-update", "about-release", "about-ahead", "about-error", "vercel-about", "vercel-about-update")
scenes += ("tool-running", "tool-failure", "compaction", "compaction-running", "compaction-empty", "compaction-error", "compaction-interrupted", "vercel-compaction")
scenes += ("tool-output-long", "vercel-tool-output-long", "tool-search", "vercel-tools")
scenes += ("composer-empty", "composer-multiline", "composer-queued", "composer-queue-error", "composer-attachments")
scenes += ("composer-long", "composer-attachment-status", "composer-attachment-preview", "composer-image-preview")
for theme in ("dark", "light"):
    for compact in (False, True):
      for label in (*scenes, *interactions):
        scene, events = interactions.get(label, (label, None))
        name = f"rune-{theme}" + (f"-{label}" if label else "") + ("-small" if compact else "")
        script = output / f"{name}-input.json"
        if events:
            script.write_text(json.dumps(events))
        command = [
            str(binary), "--design-fixture", "--theme", theme,
            "--scale", "1", "--at", "1.8" if label == "patch-expanded" else "1", "--size",
            "800x620" if compact or scene in ("compact", "crowded") else "1280x1000",
            *(["--scene", scene] if scene else []),
            "--snap", str(output / f"{name}.png"),
            "--tree", str(output / f"{name}.json"),
            *(["--script", str(script)] if events else []),
        ]
        result = subprocess.run(command, cwd=root, capture_output=True, text=True)
        (output / f"{name}.log").write_text(result.stdout + result.stderr)
        if result.returncode:
            raise RuntimeError(f"{name}: {result.stdout}{result.stderr}")
print(f"Captured {(len(scenes) + len(interactions)) * 4} native states in {output}")

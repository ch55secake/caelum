#!/usr/bin/env python3
"""Capture the real native views with deterministic design data."""
from pathlib import Path
import argparse
from concurrent.futures import ThreadPoolExecutor
import json
import os
import subprocess
import time

root = Path(__file__).resolve().parents[3]
scenes = ("", "inspector", "compact", "settings", "providers", "provider-error", "notifications", "storage", "about", "models", "models-empty", "launcher", "session", "timeline", "timeline-empty", "usage", "usage-loading", "stats", "stats-empty", "stats-loading", "stats-error", "commands", "shells", "working", "thinking", "retrying", "stopping", "empty", "error", "review-loading", "review-error", "discussion", "tools", "review", "checks", "mentions", "images", "image-preview", "files", "files-loading", "changes-empty", "approval", "questions", "crowded", "worktrees", "worktrees-loading", "worktrees-error")
interactions = {
    "vm-tool-output": ("remote-trace", []),
    "vm-reasoning-summary": ("remote-trace", [{"t": 0.2, "key": "vm-tool-checkout-example-test-call"}, {"t": 0.45, "key": "vm-summary-checkout-example-summary"}]),
    "vm-first-effort": ("remote-model-draft", [{"t": 0.1, "key": "vm-provider-trigger"}, {"t": 0.22, "key": "option0"}, {"t": 0.34, "key": "vm-model-trigger"}, {"t": 0.46, "key": "vm-model-item-example/reasoner"}, {"t": 0.58, "key": "vm-effort-trigger"}, {"t": 0.7, "key": "option3"}, {"t": 0.82, "key": "vm-effort-trigger"}]),
    "vm-first-model": ("remote-model-draft", [{"t": 0.2, "key": "vm-model-trigger"}, {"t": 0.4, "key": "vm-model-item-example/fast"}, {"t": 0.65, "key": "vm-model-trigger"}]),
    "vm-session-launcher": ("remote-machine", [{"t": 0.1, "type": "key", "name": "Escape"}, {"t": 0.3, "type": "key", "name": "n", "cmd": True, "shift": True}, {"t": 0.5, "key": "launch-provider-codex"}]),
    "vm-session-sidebar": ("remote-machine", [{"t": 0.1, "type": "key", "name": "Escape"}, {"t": 0.3, "type": "key", "name": "n", "cmd": True}]),
    "remote-provider": ("remote-machine", [{"t": 0.1, "type": "key", "name": "Escape"}, {"t": 0.35, "key": "vm-provider-trigger"}]),
    "remote-sessions": ("remote-machine", [{"t": 0.1, "type": "key", "name": "Escape"}, {"t": 0.25, "key": "vm-threads"}, {"t": 0.45, "key": "new-vm-session"}, {"t": 0.7, "key": "vm-threads"}]),
    "remote-create-ready": ("remote-create", []),
    "remote-remove": ("remote-working", [{"t": 0.25, "key": "remove-remote-checkout-example"}]),
    "home-continue-on-vm": ("home-draft", [{"t": 0.25, "key": "continue-on-vm"}]),
    "remote-handoff-new": ("remote-handoff", [{"t": 0.25, "key": "handoff-new-vm"}]),
    "question-invalid": ("questions", [{"t": 0.25, "key": "question-fixture-target-0"}, {"t": 0.5, "key": "question-submit-fixture"}]),
    "question-pending": ("questions", [{"t": 0.25, "key": "question-submit-fixture"}]),
    "session-feed-filter": ("session-feed", [{"t": 0.25, "key": "session-project-filter-trigger"}]),
    "session-feed-search": ("session-feed", [{"t": 0.25, "key": "session-search"}, {"t": 0.4, "paste": "focus"}]),
    "session-feed-project": ("session-feed", [{"t": 0.25, "key": "session-project-filter-trigger"}, {"t": 0.5, "key": "option2"}]),
    "composer-focus": ("", [{"t": 0.25, "key": "live-draft-0"}]),
    "check-details": ("merge-ready", [{"t": 0.25, "key": "review-checks-toggle"}]),
    "worktree-base": ("worktree-create", [{"t": 0.25, "key": "worktree-base-trigger"}]),
    "user-copy": ("messages", [{"t": 0.25, "type": "move", "key": "copy-user-0-opencode-formatted"}]),
    "merge-method": ("checks", [{"t": 0.25, "key": "merge-method-trigger"}]),
    "auto-merge-enabling": ("merge-blocked", [{"t": 0.25, "key": "set-auto-merge"}]),
    "auto-merge-disabling": ("merge-auto-enabled", [{"t": 0.25, "key": "set-auto-merge"}]),
    "overview-long-checks": ("overview-long", [{"t": 0.25, "key": "review-checks-toggle"}]),
    "workspace-menu": ("", [{"t": 0.25, "key": "workspace-trigger"}]),
    "overview-inspector": ("overview", [{"t": 0.25, "key": "workspace-trigger"}, {"t": 0.5, "key": "workspace-item0"}]),
    "provider-dropdown": ("providers", [{"t": 0.25, "key": "Default effort-trigger"}]),
    "model-hover": ("models", [{"t": 0.25, "type": "move", "key": "model-favorite-1"}]),
    "model-focus": ("models", [{"t": 0.25, "type": "key", "name": "Tab"}]),
    "models-connection": ("models", [{"t": 0.25, "key": "model-filter-connection-opencode-go"}]),
    "models-favorites": ("models", [{"t": 0.25, "key": "model-filter-favorites"}]),
    "models-search": ("models", [{"t": 0.25, "key": "model-search"}, {"t": 0.4, "paste": "moonshot go"}]),
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
    "shells-waiting-session": ("shells-many", [{"t": 0.25, "key": "live-session-1"}, {"t": 0.5, "type": "move", "x": 0, "y": 0}]),
    "shells-hide": ("shells-many", [{"t": 0.25, "key": "toggle-background-shells"}]),
    "shells-reopen": ("shells-collapsed", [{"t": 0.25, "key": "toggle-background-shells"}]),
    "composer-shell-commands": ("shells-many", [{"t": 0.15, "key": "live-draft-0"}, {"t": 0.25, "type": "key", "name": "a", "cmd": True}, {"t": 0.4, "paste": "/"}]),
    "vercel-select": ("settings", [{"t": 0.25, "key": "family-vercel"}]),
    "vercel-providers": ("vercel-settings", [{"t": 0.25, "key": "settings-providers"}]),
    "vercel-notifications": ("vercel-settings", [{"t": 0.25, "key": "settings-notifications"}]),
    "patch-collapsed": ("vercel-patch-multi", [{"t": 0.25, "key": "tool-0-opencode-tool-patch"}]),
    "patch-expanded": ("vercel-patch-long", [{"t": 0.8, "key": "expand-patch-0-opencode-tool-patch"}]),
    "compaction-summary": ("compaction", [{"t": 0.25, "key": "compaction-0-opencode-compact"}]),
    "tool-error-expanded": ("tool-failure", [{"t": 0.25, "key": "tool-0-opencode-tool-run"}, {"t": 0.5, "type": "move", "x": 0, "y": 0}]),
    "tool-output-full": ("tool-output-long", [{"t": 0.25, "key": "tool-output-0-opencode-tool-run-Output-expand"}]),
}
scenes += ("worktree-create", "shell-output", "shells-many", "messages", "archive", "overview", "overview-empty", "overview-blocked", "overview-long", "overview-stale")
scenes += ("composer-options", "files-tabs", "tools-collapsed", "merge-ready", "merge-blocked", "merge-pending", "merge-conflicts", "merge-behind", "merge-draft", "merge-merged", "merge-closed")
scenes += ("merge-auto-enabled", "merge-auto-error")
scenes += ("overview-cached",)
scenes += ("provider-timeout", "provider-rejection")
scenes += ("shells-collapsed", "vercel", "vercel-settings", "sidebar-projects")
scenes += ("shell-completed", "shell-output-completed")
scenes += ("subagent-running", "subagent-completed", "subagent-failed")
scenes += ("session-feed", "session-feed-current", "session-feed-empty", "session-feed-many", "session-feed-colours", "session-review-feedback")
scenes += ("sidebar-worktrees", "sidebar-worktrees-many", "sidebar-worktrees-projects", "sidebar-worktrees-vercel", "worktrees-empty")
scenes += ("patch-short", "patch-long", "patch-multi", "vercel-patch-short", "vercel-patch-long", "vercel-patch-multi")
scenes += ("about-checking", "about-current", "about-update", "about-release", "about-ahead", "about-error", "vercel-about", "vercel-about-update")
scenes += ("tool-running", "tool-failure", "compaction", "compaction-running", "compaction-empty", "compaction-error", "compaction-interrupted", "vercel-compaction")
scenes += ("tool-output-long", "vercel-tool-output-long", "tool-search", "vercel-tools")
scenes += ("composer-empty", "composer-multiline", "composer-queued", "composer-queue-error", "composer-attachments")
scenes += ("composer-long", "composer-attachment-status", "composer-attachment-preview", "composer-image-preview")
scenes += ("models-settings",)
scenes += ("fork",)
scenes += ("remote-working", "remote-creating", "remote-setup-failed", "remote-stopped")
scenes += ("remote-machine", "remote-changes", "remote-picker", "remote-settings", "remote-create")
scenes += ("native-connected", "native-offline")
scenes += ("edit-branch", "rewind-checkpoint", "rename-session")
scenes += ("remote-handoff",)
scenes += ("home", "home-empty", "home-draft", "home-projects")
scenes += ("update-downloading", "update-verifying", "update-preparing", "update-cancelling", "update-failed", "update-success")


def capture_plan(binary, output):
    plan = []
    for theme in ("dark", "light"):
        for compact in (False, True):
            for label in (*scenes, *interactions):
                scene, events = interactions.get(label, (label, None))
                name = f"rune-{theme}" + (f"-{label}" if label else "") + ("-small" if compact else "")
                script = output / f"{name}-input.json"
                command = [
                    str(binary), "--design-fixture", "--theme", theme,
                    "--scale", "1", "--at", "1.8" if label == "patch-expanded" else "1", "--size",
                    "800x620" if compact or scene in ("compact", "crowded") else "1280x1000",
                    *(["--scene", scene] if scene else []),
                    "--snap", str(output / f"{name}.png"),
                    "--tree", str(output / f"{name}.json"),
                    *(["--script", str(script)] if events else []),
                ]
                plan.append((name, command, events))
    return plan


def capture_one(item, output, timeout):
    name, command, events = item
    started = time.perf_counter()
    log = ""
    error = None
    try:
        # A stale successful artifact must never hide a missing output on a rerun.
        for suffix in (".png", ".json", ".log", "-input.json"):
            (output / f"{name}{suffix}").unlink(missing_ok=True)
        if events:
            (output / f"{name}-input.json").write_text(json.dumps(events))
        result = subprocess.run(command, cwd=root, capture_output=True, text=True, timeout=timeout)
        log = result.stdout + result.stderr
        result.check_returncode()
        with (output / f"{name}.png").open("rb") as image:
            if image.read(8) != b"\x89PNG\r\n\x1a\n":
                raise ValueError("Missing or invalid PNG")
        json.loads((output / f"{name}.json").read_text())
    except subprocess.TimeoutExpired as failure:
        log = (failure.stdout or b"").decode(errors="replace") + (failure.stderr or b"").decode(errors="replace")
        error = f"Capture exceeded {timeout:g}s"
    except (OSError, ValueError, subprocess.CalledProcessError) as failure:
        error = str(failure)
    (output / f"{name}.log").write_text(log + (f"\n{error}\n" if error else ""))
    return {"name": name, "elapsed_seconds": time.perf_counter() - started, "error": error}


def capture_all(plan, output, workers, timeout=60):
    if not 1 <= workers <= 4:
        raise ValueError("workers must be between 1 and 4")
    names = [item[0] for item in plan]
    if len(names) != len(set(names)):
        raise ValueError("Capture names must be unique")
    output.mkdir(parents=True, exist_ok=True)
    manifest = output / "manifest.json"
    manifest.unlink(missing_ok=True)
    started = time.perf_counter()
    # Each child has its own input, image, tree and log. map preserves manifest order.
    with ThreadPoolExecutor(max_workers=workers) as pool:
        results = list(pool.map(lambda item: capture_one(item, output, timeout), plan))
    failed = [result["name"] for result in results if result["error"]]
    manifest.write_text(json.dumps({
        "workers": workers,
        "elapsed_seconds": time.perf_counter() - started,
        "expected": len(plan),
        "completed": len(results) - len(failed),
        "captures": results,
    }, indent=2) + "\n")
    if failed:
        raise RuntimeError(f"{len(failed)} captures failed (see individual logs): {', '.join(failed)}")
    print(f"Captured {len(results)} native states with {workers} workers in {output}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", nargs="?", type=Path, default=root / "apps/desktop/preview-results")
    parser.add_argument("--workers", type=int, choices=range(1, 5), default=min(4, os.cpu_count() or 1))
    args = parser.parse_args()
    subprocess.run(["cargo", "build", "--locked", "-p", "caelum-desktop"], check=True, cwd=root)
    output = args.output.resolve()
    capture_all(capture_plan(root / "target/debug/caelum-desktop", output), output, args.workers)


if __name__ == "__main__":
    main()

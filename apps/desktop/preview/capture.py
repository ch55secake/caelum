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
    "provider-dropdown": ("providers", [{"t": 0.25, "key": "Default effort-trigger"}]),
    "model-hover": ("models", [{"t": 0.25, "type": "move", "key": "model-favorite-1"}]),
    "model-focus": ("models", [{"t": 0.25, "type": "key", "name": "Tab"}]),
    "settings-hover": ("settings", [{"t": 0.25, "type": "move", "key": "settings-providers"}]),
    "worktrees-form": ("worktrees", [{"t": 0.25, "type": "scroll", "key": "worktrees-scroll", "dy": -480, "dur": 0.2}]),
    "composer-provider": ("", [{"t": 0.25, "key": "provider-trigger"}]),
    "composer-effort": ("", [{"t": 0.25, "key": "effort-trigger"}]),
    "session-hover": ("session", [{"t": 0.25, "type": "move", "key": "archive-session"}]),
    "stats-details": ("stats", [{"t": 0.25, "type": "scroll", "key": "command-results", "dy": -420, "dur": 0.2}]),
    "stats-projects": ("stats", [{"t": 0.25, "key": "stats-project-filter"}]),
    "timeline-search": ("timeline", [{"t": 0.25, "key": "timeline-search"}, {"t": 0.3, "paste": "draft"}]),
    "review-navigation": ("review", [{"t": 0.25, "key": "review-nav-toggle"}]),
}
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
            "--scale", "1", "--at", "1", "--size",
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

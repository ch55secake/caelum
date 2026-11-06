#!/usr/bin/env python3
"""Capture the real native views with deterministic design data."""
from pathlib import Path
import subprocess
import sys

root = Path(__file__).resolve().parents[3]
output = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else root / "apps/desktop/preview-results"
output.mkdir(parents=True, exist_ok=True)
subprocess.run(["cargo", "build", "--locked", "-p", "caelum-desktop"], check=True, cwd=root)
binary = root / "target/debug/caelum-desktop"
for theme in ("dark", "light"):
    for scene in ("", "inspector", "compact", "settings"):
        name = f"rune-{theme}" + (f"-{scene}" if scene else "")
        subprocess.run([
            str(binary), "--design-fixture", "--theme", theme,
            "--scale", "1", "--at", "1", "--size",
            "800x620" if scene == "compact" else "1280x1000",
            *(["--scene", scene] if scene else []),
            "--snap", str(output / f"{name}.png"),
            "--tree", str(output / f"{name}.json"),
        ], check=True, cwd=root)

#!/usr/bin/env python3
"""Exercise the packaged CLI and LaunchServices against an isolated native workspace."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time


def wait_for(check, description):
    deadline = time.monotonic() + 20
    while time.monotonic() < deadline:
        value = check()
        if value:
            return value
        time.sleep(0.1)
    raise RuntimeError(f"Timed out: {description}")


def pids(executable):
    lines = subprocess.check_output(["ps", "-axo", "pid=,comm="], text=True).splitlines()
    return [int(pid) for line in lines for pid, command in [line.strip().split(None, 1)]
            if command == str(executable)]


def application(pid, action):
    return subprocess.check_output([
        "osascript", "-l", "JavaScript", "-e",
        f'ObjC.import("AppKit"); $.NSRunningApplication.runningApplicationWithProcessIdentifier({pid}).{action};',
    ], text=True).strip()


def create_projects(root):
    projects = [root / "first café #1", root / "second & 2%+?"]
    for project in projects:
        project.mkdir()
    # Match the canonical paths saved by Project::open. Filesystems can return
    # a different Unicode normalization than the string used to create a path.
    return [project.resolve(strict=True) for project in projects]


def run(app, cli, root):
    executable = app / "Contents/MacOS/caelum-desktop"
    if pids(executable):
        raise RuntimeError("The supplied app is already running; use a separate build for this check")
    home = root / "home"
    home.mkdir()
    bin_dir = root / "bin"
    bin_dir.mkdir()
    state = home / "workspace.json"
    log = root / "app.log"
    projects = create_projects(root)
    # Keep this check offline, including on a developer's authenticated machine.
    for provider in ("codex", "opencode"):
        path = bin_dir / provider
        path.write_text("#!/bin/sh\nexit 1\n")
        path.chmod(0o755)
    # Verify the CLI's actual launch arguments, then target the supplied test bundle
    # without replacing the user's installed app or global launch environment.
    launch = bin_dir / "open"
    launch.write_text('''#!/bin/sh
set -eu
test "$#" -eq 3
test "$1" = -a
test "$2" = Caelum
exec /usr/bin/open -a "$CAELUM_SMOKE_APP" \
  --env "HOME=$CAELUM_SMOKE_HOME" --env "CAELUM_STATE=$CAELUM_SMOKE_STATE" \
  --env "XDG_STATE_HOME=$CAELUM_SMOKE_HOME/state" --env "PATH=$PATH" \
  --stdout "$CAELUM_SMOKE_LOG" --stderr "$CAELUM_SMOKE_LOG" "$3"
''')
    launch.chmod(0o755)
    sessions = [dict(scope=dict(project=str(project), provider="codex"),
                     remote_id=None, title=f"Draft {i}", options={}, draft=f"Preserved draft {i}",
                     attachments=[], failed_prompt=None, pending_submission=None,
                     queue=[], archived=False, reading=-1.0) for i, project in enumerate(projects)]
    state.write_text(json.dumps(dict(version=1, projects=[dict(name=p.name, path=str(p)) for p in projects],
                                    sessions=sessions, selected=0, preferences={},
                                    remembered={str(p): i for i, p in enumerate(projects)}, project_roots={})))
    environment = {**os.environ, "PATH": f"{bin_dir}:/usr/bin:/bin:/usr/sbin:/sbin",
                   "CAELUM_SMOKE_APP": str(app), "CAELUM_SMOKE_HOME": str(home),
                   "CAELUM_SMOKE_STATE": str(state), "CAELUM_SMOKE_LOG": str(log)}

    def selected(index):
        snapshot = json.loads(state.read_text())
        return snapshot if snapshot["selected"] == index else None

    def open_project(index):
        subprocess.run([str(cli), str(projects[index])], env=environment, check=True)
        snapshot = wait_for(lambda: selected(index), f"project {index} URL delivery")
        assert [s["draft"] for s in snapshot["sessions"]] == [s["draft"] for s in sessions]

    def quit_app(pid):
        application(pid, "terminate")
        wait_for(lambda: pid not in pids(executable), "graceful application shutdown")

    pid = None
    try:
        open_project(1)  # Cold launch must select the URL, not the saved selection.
        pid = wait_for(lambda: next(iter(pids(executable)), None), "application launch")
        open_project(0)
        assert pids(executable) == [pid], "A warm URL launched another process"
        application(pid, "hide")
        wait_for(lambda: application(pid, "isHidden") == "true", "hide application")
        open_project(1)
        wait_for(lambda: application(pid, "isHidden") == "false", "URL reopens hidden application")
        assert pids(executable) == [pid]
        quit_app(pid)
        pid = None
        open_project(0)  # Restart against the flushed snapshot.
        pid = wait_for(lambda: next(iter(pids(executable)), None), "application relaunch")
        quit_app(pid)
        pid = None
        assert [s["draft"] for s in json.loads(state.read_text())["sessions"]] == [s["draft"] for s in sessions]
        print("Verified packaged CLI cold/warm URLs, hidden-app reopening, draft restoration and graceful quit")
    finally:
        # Only this supplied bundle's process is eligible for cleanup.
        for owned in pids(executable):
            quit_app(owned)
        if log.exists():
            print(log.read_text())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--app", type=Path, required=True)
    parser.add_argument("--cli", type=Path, required=True)
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="caelum-launch-") as temporary:
        run(args.app.resolve(), args.cli.resolve(), Path(temporary).resolve())


if __name__ == "__main__":
    main()

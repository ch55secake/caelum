#!/usr/bin/env python3
"""Sample release-app RSS and interval CPU without attributing shared providers."""

import argparse
import datetime
import gzip
import json
import platform
import plistlib
import re
import statistics
import subprocess
import time
from pathlib import Path


def output(*args):
    return subprocess.check_output(args, text=True).strip()


def cpu_seconds(value):
    result = 0.0
    for part in value.split(":"):
        result = result * 60 + float(part)
    return result


def processes():
    result = {}
    for line in output("ps", "-axo", "pid=,ppid=,rss=,time=,comm=").splitlines():
        pid, parent, rss, cpu, command = line.strip().split(None, 4)
        result[int(pid)] = {
            "pid": int(pid), "parent": int(parent), "rssKiB": int(rss),
            "cpuSeconds": cpu_seconds(cpu), "executable": command,
        }
    return result


def coalition(pid):
    # WebKit XPC helpers have PPID 1. LaunchServices' resource coalition, rather
    # than the process name or launch time, associates them with the right app.
    text = output("lsappinfo", "info", "-only", "LSApplicationMemberCoalitionIDKey", "-app", str(pid))
    match = re.search(r'"LSApplicationMemberCoalitionIDKey"=(\d+)', text)
    return int(match[1]) if match else None


def classify(snapshot, pid, app_coalition, coalitions):
    owned = {pid}
    while True:
        children = {p for p, value in snapshot.items() if value["parent"] in owned}
        if children <= owned:
            break
        owned |= children
    result = {}
    for key, value in snapshot.items():
        executable = value["executable"]
        if key == pid:
            result[key] = "native"
        elif "com.apple.WebKit." in executable:
            if app_coalition is not None and coalitions.get(key) == app_coalition:
                result[key] = "webkit"
        elif key in owned:
            result[key] = "provider" if re.search(r"(?:^|/)(?:codex|opencode)(?:$|[-/])", executable) else "child"
    return result


def artifact(path):
    files = sorted(p for p in path.rglob("*") if p.is_file()) if path.is_dir() else [path]
    web = [p for p in files if p.suffix in (".js", ".css", ".html")]
    return {
        "path": str(path), "bytes": sum(p.stat().st_size for p in files),
        "files": len(files),
        "webGzipBytes": sum(len(gzip.compress(p.read_bytes(), mtime=0)) for p in web) if web else None,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    target = parser.add_mutually_exclusive_group(required=True)
    target.add_argument("--app", type=Path, help="Launch a fresh instance using LaunchServices")
    target.add_argument("--pid", type=int, help="Attach to an already running release app")
    parser.add_argument("--label", required=True)
    parser.add_argument("--duration", type=float, default=30)
    parser.add_argument("--interval", type=float, default=1)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--artifact", type=Path, action="append", default=[])
    parser.add_argument("--project", type=Path, help="Project to open in the new app instance")
    parser.add_argument("--quit", action="store_true", help="Quit the app launched by this command after sampling")
    args = parser.parse_args()
    if platform.system() != "Darwin":
        parser.error("The native sampler requires macOS")
    if not 0 < args.interval <= args.duration <= 3600:
        parser.error("Use 0 < interval <= duration <= 3600 seconds")
    if (args.quit or args.project) and not args.app:
        parser.error("--quit and --project require --app")
    launched = time.monotonic()
    pid = args.pid
    if args.app:
        app = args.app.resolve()
        with (app / "Contents/Info.plist").open("rb") as file:
            metadata = plistlib.load(file)
        executable = str(app / "Contents/MacOS" / metadata["CFBundleExecutable"])
        previous = set(processes())
        command = ["open", "-n", str(app)]
        if args.project:
            command.extend(["--args", str(args.project.resolve())])
        subprocess.run(command, check=True)
        while time.monotonic() - launched < 15:
            found = [p for p, value in processes().items() if p not in previous and value["executable"] == executable]
            if found:
                pid = found[0]
                break
            time.sleep(0.1)
        if pid is None:
            raise RuntimeError("Launched app process was not found")
    app_coalition = coalition(pid)
    if app_coalition is None:
        raise RuntimeError("App has no LaunchServices coalition; launch the .app with open -n")
    samples = []
    coalitions = {}
    previous = {}
    previous_time = None
    started = time.monotonic()
    try:
        while time.monotonic() - started < args.duration:
            tick = time.monotonic()
            snapshot = processes()
            if pid not in snapshot:
                raise RuntimeError("App exited before sampling finished")
            for key, value in snapshot.items():
                if "com.apple.WebKit." in value["executable"] and key not in coalitions:
                    coalitions[key] = coalition(key)
            groups = classify(snapshot, pid, app_coalition, coalitions)
            rows = []
            for key, group in groups.items():
                value = snapshot[key]
                before = previous.get(key)
                cpu = None
                if before and before["executable"] == value["executable"] and previous_time is not None:
                    cpu = max(0, value["cpuSeconds"] - before["cpuSeconds"]) * 100 / (tick - previous_time)
                rows.append({**value, "group": group, "cpuPercent": cpu})
            samples.append({"seconds": tick - launched, "processes": rows})
            previous, previous_time = snapshot, tick
            time.sleep(max(0, args.interval - (time.monotonic() - tick)))
    finally:
        if args.quit:
            subprocess.run([
                "osascript", "-l", "JavaScript", "-e",
                'ObjC.import("AppKit"); '
                f'$.NSRunningApplication.runningApplicationWithProcessIdentifier({pid}).terminate;',
            ], check=True, stdout=subprocess.DEVNULL)
    summary = {}
    for group in ("native", "webkit", "provider", "child"):
        rss = [sum(p["rssKiB"] for p in sample["processes"] if p["group"] == group) for sample in samples]
        cpu = [sum(p["cpuPercent"] or 0 for p in sample["processes"] if p["group"] == group) for sample in samples[1:]]
        summary[group] = {
            "rssMiBMedian": statistics.median(rss) / 1024, "rssMiBPeak": max(rss) / 1024,
            "cpuPercentMean": statistics.mean(cpu) if cpu else None,
        }
    report = {
        "label": args.label, "recordedAt": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "revision": output("git", "rev-parse", "HEAD"), "dirty": bool(output("git", "status", "--porcelain")),
        "os": platform.platform(), "machine": output("sysctl", "-n", "hw.model"),
        "memoryBytes": int(output("sysctl", "-n", "hw.memsize")),
        "appPid": pid, "coalition": app_coalition, "summary": summary,
        "artifacts": [artifact(path) for path in args.artifact],
        "samples": samples,
        "unattributedProviders": [
            {"pid": key, "executable": value["executable"]}
            for key, value in previous.items()
            if key not in groups and re.search(r"(?:^|/)(?:codex|opencode)(?:$|[-/])", value["executable"])
        ],
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({"output": str(args.output), "summary": summary}, indent=2))


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""Summarize each repeat without hiding its range or unsupported measurements."""

import argparse
import json
from pathlib import Path


def summarize(report):
    rows = []
    for sample in report["samples"]:
        commands = list(sample["commands"].values())
        events = list(sample["events"].values())
        metrics = {m["name"]: m["value"] for m in sample.get("chromium") or []}
        rows.append({
            "scenario": sample["name"],
            "elapsedMs": round(sample["elapsedMs"], 2),
            "navigationMs": round(sample["navigationMs"], 2),
            "commits": sample["renders"]["count"],
            "renderMs": round(sample["renders"]["total"], 2),
            "renderP95Ms": sample["renders"]["p95"],
            "frameGapP95Ms": sample["frameGapMs"]["p95"],
            "longTasks": sample["longTasks"]["count"] if sample["longTasks"] is not None else None,
            "invokes": sum(c["count"] for c in commands),
            "invokeRequestBytes": sum(c["requestBytes"] for c in commands),
            "invokeResponseBytes": sum(c["responseBytes"] for c in commands),
            "events": sum(e["count"] for e in events),
            "eventBytes": sum(e["bytes"] for e in events),
            "domNodes": sample["domNodes"], "transcriptEntries": sample["transcriptEntries"],
            "heapMiB": round(metrics["JSHeapUsedSize"] / 1024**2, 2) if "JSHeapUsedSize" in metrics else None,
            "outputLatencyMs": sample.get("outputLatencyMs"),
        })
    return {key: value for key, value in report.items() if key != "samples"} | {"samples": rows}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("--native", type=Path)
    args = parser.parse_args()
    directory = args.directory
    reports = [summarize(json.loads(path.read_text())) for path in sorted(directory.rglob("metrics.json"))]
    if not reports:
        raise SystemExit(f"No metrics.json files under {directory}")
    native = []
    if args.native:
        for path in sorted(args.native.glob("*.json")):
            report = json.loads(path.read_text())
            native.append({
                key: report[key] for key in (
                    "label", "recordedAt", "revision", "dirty", "os", "machine", "memoryBytes", "summary", "artifacts",
                )
            } | {"unattributedProviderCount": len(report["unattributedProviders"])})
    print(json.dumps({"frontend": reports, "native": native}, indent=2))

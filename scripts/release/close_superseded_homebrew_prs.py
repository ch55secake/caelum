#!/usr/bin/env python3
"""Close older versioned Caelum update PRs in the Homebrew tap."""

from __future__ import annotations

import json
import re
import subprocess
import sys
from collections.abc import Callable


REPOSITORY = "ch55secake/homebrew-tap"
BRANCH_PATTERN = re.compile(r"automation/caelum-v(\d+)\.(\d+)\.(\d+)")
VERSION_PATTERN = re.compile(r"v?(\d+)\.(\d+)\.(\d+)")


def version(value: str) -> tuple[int, int, int]:
    match = VERSION_PATTERN.fullmatch(value)
    if match is None:
        raise ValueError(f"invalid stable Caelum version: {value}")
    return tuple(map(int, match.groups()))


def gh(*args: str) -> str:
    return subprocess.check_output(["gh", *args], text=True).strip()


def close_superseded_prs(release_tag: str, run_gh: Callable[..., str] = gh) -> list[int]:
    current = version(release_tag)
    output = run_gh(
        "pr",
        "list",
        "--repo",
        REPOSITORY,
        "--state",
        "open",
        "--limit",
        "1000",
        "--json",
        "number,headRefName",
    )
    pull_requests = json.loads(output)
    closed = []
    for pull_request in pull_requests:
        match = BRANCH_PATTERN.fullmatch(pull_request["headRefName"])
        if match is None or tuple(map(int, match.groups())) >= current:
            continue
        number = int(pull_request["number"])
        run_gh(
            "pr",
            "close",
            str(number),
            "--repo",
            REPOSITORY,
            "--delete-branch",
            "--comment",
            f"Superseded by Caelum {release_tag}.",
        )
        closed.append(number)
    return closed


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit(f"usage: {sys.argv[0]} RELEASE_TAG")
    closed = close_superseded_prs(sys.argv[1])
    print(f"Closed {len(closed)} superseded Caelum Homebrew PR(s).")


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""Rust size and capability-folder guardrails, inspired by anomalyco/rune.

Existing exceptions may shrink or disappear; CI compares them with the PR base.
Only tracked and non-ignored new Rust files under apps/ and crates/ are measured.
"""
import argparse
import os
from pathlib import Path, PurePosixPath
import re
import subprocess
import sys
from collections import Counter, defaultdict

from rust import is_test, measure

ROOT = Path(__file__).resolve().parents[2]
EXCEPTIONS = "scripts/structure/exceptions.txt"
SOURCE_SOFT, SOURCE_HARD, TEST_HARD = 800, 1500, 1000
FOLDER_SOFT, FOLDER_HARD = 40, 60


def parse_exceptions(text):
    result = {}
    for line in text.splitlines():
        if not line.strip() or line.startswith("#"):
            continue
        kind, path, ceiling = line.split()
        key = (kind, path)
        if kind not in {"source", "tests", "folder", "family"} or key in result:
            raise ValueError(f"invalid or duplicate exception: {line}")
        result[key] = int(ceiling)
        if result[key] <= 0:
            raise ValueError(f"invalid ceiling: {line}")
    return result


def ratchet(previous, current):
    return [f"exception may not be added or raised: {kind} {path} ({value})"
            for (kind, path), value in current.items()
            if (kind, path) not in previous or value > previous[(kind, path)]]


def folder_sizes(paths):
    folders = defaultdict(list)
    subdirs = defaultdict(set)
    for path in map(PurePosixPath, paths):
        for parent in path.parents:
            if parent.parent != parent:
                subdirs[str(parent.parent)].add(parent.name)
        if is_test(str(path)) or path.parent.name in {"examples", "benches", "bin"}:
            continue
        folders[str(path.parent)].append(path.stem)
    result = {}
    for folder, stems in folders.items():
        if len(stems) > FOLDER_SOFT:
            result[("folder", folder)] = len(stems)
        families = Counter(re.split(r"[_-]", stem, maxsplit=1)[0]
                           for stem in stems if "_" in stem or "-" in stem)
        for name, count in families.items():
            has_base = name in stems or name in subdirs[folder]
            if count >= (2 if has_base else 3):
                result[("family", f"{folder}/{name}_*")] = count
    return result


def inventory(root, paths):
    result = folder_sizes(paths)
    for path in paths:
        size = measure(path, (root / path).read_text())
        if size.source > SOURCE_SOFT:
            result[("source", path)] = size.source
        if size.tests > TEST_HARD:
            result[("tests", path)] = size.tests
    return result


def hard_limit(kind):
    return {"source": SOURCE_HARD, "tests": TEST_HARD,
            "folder": FOLDER_HARD, "family": 0}[kind]


def diagnostics(sizes, exceptions):
    rows = []
    for (kind, path), count in sorted(sizes.items()):
        limit = exceptions.get((kind, path), hard_limit(kind))
        level = "error" if count > limit else "warning"
        detail = {"source": "non-test lines", "tests": "test lines",
                  "folder": "source files", "family": "prefixed siblings"}[kind]
        hint = "group this capability into a folder" if kind in {"folder", "family"} else "extract a focused module"
        rows.append((level, path, f"{count} {detail}; ceiling {limit}: {hint}"))
    return rows


def tighten(exceptions, sizes):
    return {key: min(ceiling, sizes[key]) for key, ceiling in exceptions.items()
            if sizes.get(key, 0) > hard_limit(key[0])}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base", default="HEAD", help="revision whose exception ceilings cannot be raised")
    parser.add_argument("--update-exceptions", action="store_true", help="lower ceilings and remove resolved exceptions")
    args = parser.parse_args()
    paths = subprocess.check_output(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z", "apps", "crates"],
        cwd=ROOT,
    ).decode().split("\0")
    paths = sorted({path for path in paths if path.endswith(".rs") and (ROOT / path).is_file()})
    sizes = inventory(ROOT, paths)
    exceptions = parse_exceptions((ROOT / EXCEPTIONS).read_text())
    baseline = subprocess.run(["git", "show", f"{args.base}:{EXCEPTIONS}"], cwd=ROOT,
                              capture_output=True, text=True)
    errors = ratchet(parse_exceptions(baseline.stdout), exceptions) if baseline.returncode == 0 else []
    # A missing baseline is valid only when first introducing the checker.
    revision = subprocess.run(["git", "rev-parse", "--verify", args.base], cwd=ROOT, capture_output=True)
    if revision.returncode:
        errors.append(f"cannot resolve baseline revision: {args.base}")
    if args.update_exceptions and not errors:
        exceptions = tighten(exceptions, sizes)
        lines = ["# kind path ceiling; existing violations only, ceilings may only decrease."]
        lines.extend(f"{kind} {path} {ceiling}" for (kind, path), ceiling in sorted(exceptions.items()))
        (ROOT / EXCEPTIONS).write_text("\n".join(lines) + "\n")
    for key, ceiling in exceptions.items():
        if sizes.get(key, 0) < ceiling:
            errors.append(f"lower or remove resolved exception: {' '.join(key)}; run --update-exceptions")
    for error in errors:
        print(f"ERROR {error}")
    rows = diagnostics(sizes, exceptions)
    for level, path, message in rows:
        print(f"{level.upper():7} {path}: {message}")
        if os.environ.get("GITHUB_ACTIONS") == "true":
            print(f"::{level} file={path}::{message}")
    failures = len(errors) + sum(level == "error" for level, _, _ in rows)
    print(f"Structure: {len(paths)} Rust files, {failures} errors, {len(rows) - failures + len(errors)} warnings")
    return bool(failures)


if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
"""Create an immutable release tag when a version bump reaches main."""

from __future__ import annotations

import json
import os
import subprocess
import tomllib
from pathlib import Path


def git(*args: str) -> str:
    return subprocess.check_output(["git", *args], text=True).strip()


def project_versions(revision: str) -> dict[str, str]:
    cargo = tomllib.loads(git("show", f"{revision}:Cargo.toml"))
    app = json.loads(git("show", f"{revision}:apps/desktop/src-tauri/tauri.conf.json"))
    frontend = json.loads(git("show", f"{revision}:apps/desktop/package.json"))
    return {
        "Cargo workspace": cargo["workspace"]["package"]["version"],
        "Tauri app": app["version"],
        "frontend package": frontend["version"],
    }


def workspace_lock_versions(revision: str) -> dict[str, str]:
    lockfile = tomllib.loads(git("show", f"{revision}:Cargo.lock"))
    return {
        package["name"]: package["version"]
        for package in lockfile["package"]
        if package["name"].startswith("caelum-") and "source" not in package
    }


def set_outputs(tag: str, should_release: bool) -> None:
    with Path(os.environ["GITHUB_OUTPUT"]).open("a", encoding="utf-8") as output:
        output.write(f"release_tag={tag}\n")
        output.write(f"should_release={str(should_release).lower()}\n")


def remote_tag_commit(tag: str) -> str | None:
    tag_ref = f"refs/tags/{tag}"
    result = subprocess.run(
        ["git", "ls-remote", "origin", tag_ref, f"{tag_ref}^{{}}"],
        check=True,
        capture_output=True,
        text=True,
    )
    refs = dict(line.split("\t", maxsplit=1)[::-1] for line in result.stdout.splitlines())
    return refs.get(f"{tag_ref}^{{}}") or refs.get(tag_ref)


def main() -> None:
    ref = os.environ["GITHUB_REF"]
    if ref.startswith("refs/tags/"):
        tag = os.environ.get("GITHUB_REF_NAME", ref.removeprefix("refs/tags/"))
        if not tag.startswith("v"):
            raise SystemExit(f"Unexpected release tag: {tag}")
        set_outputs(tag, True)
        return

    if ref != "refs/heads/main":
        raise SystemExit(f"Expected a main branch push or version tag, got {ref}")

    current = project_versions("HEAD")
    before = os.environ.get("BEFORE_SHA", "")
    previous = (
        None
        if not before or set(before) == {"0"}
        else project_versions(before)
    )
    if previous == current:
        print("Project version is unchanged; skipping automatic release.")
        set_outputs("", False)
        return

    if len(set(current.values())) != 1:
        raise SystemExit(
            "Cargo workspace, Tauri app, and frontend package versions must match: "
            + ", ".join(f"{name}={version}" for name, version in current.items())
        )

    version = next(iter(current.values()))
    lock_versions = workspace_lock_versions("HEAD")
    if not lock_versions or any(
        lock_version != version for lock_version in lock_versions.values()
    ):
        raise SystemExit(
            "Cargo.lock must be refreshed for the current workspace version "
            f"{version}; found {lock_versions}"
        )

    tag = f"v{version}"
    commit = os.environ["GITHUB_SHA"]
    existing_commit = remote_tag_commit(tag)
    if existing_commit is not None and existing_commit != commit:
        raise SystemExit(
            f"Release tag {tag} already points to {existing_commit}; refusing to move it."
        )

    if existing_commit is None:
        subprocess.run(
            [
                "git",
                "-c",
                "user.name=github-actions[bot]",
                "-c",
                "user.email=41898282+github-actions[bot]@users.noreply.github.com",
                "tag",
                "-a",
                tag,
                commit,
                "-m",
                f"Release {tag}",
            ],
            check=True,
        )
        subprocess.run(["git", "push", "origin", f"refs/tags/{tag}"], check=True)
        print(f"Created release tag {tag} on {commit}.")
    else:
        print(f"Reusing release tag {tag} on {commit}.")

    set_outputs(tag, True)


if __name__ == "__main__":
    main()

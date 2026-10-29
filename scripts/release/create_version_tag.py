#!/usr/bin/env python3
"""Bump, tag, and publish a patch release for each merge to main."""

from __future__ import annotations

import base64
import json
import os
import re
import subprocess
import tomllib
from pathlib import Path

VERSION_FILES = (
    "Cargo.toml",
    "Cargo.lock",
    "apps/desktop/package.json",
    "apps/desktop/src-tauri/tauri.conf.json",
)
BOT_NAME = "github-actions[bot]"
BOT_EMAIL = "41898282+github-actions[bot]@users.noreply.github.com"


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


def workspace_lock_versions_from_text(lockfile: str) -> dict[str, str]:
    packages = tomllib.loads(lockfile)["package"]
    return {
        package["name"]: package["version"]
        for package in packages
        if package["name"].startswith("caelum-") and "source" not in package
    }


def workspace_lock_versions(revision: str) -> dict[str, str]:
    lockfile = git("show", f"{revision}:Cargo.lock")
    return workspace_lock_versions_from_text(lockfile)


def validate_version(revision: str) -> str:
    versions = project_versions(revision)
    if len(set(versions.values())) != 1:
        raise SystemExit(
            "Cargo workspace, Tauri app, and frontend package versions must match: "
            + ", ".join(f"{name}={version}" for name, version in versions.items())
        )

    version = next(iter(versions.values()))
    lock_versions = workspace_lock_versions(revision)
    if not lock_versions or any(lock_version != version for lock_version in lock_versions.values()):
        raise SystemExit(
            "Cargo.lock must be refreshed for the current workspace version "
            f"{version}; found {lock_versions}"
        )
    return version


def bump_patch_version(version: str) -> str:
    match = re.fullmatch(r"(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)", version)
    if match is None:
        raise SystemExit(f"Cannot automatically patch-bump non-stable SemVer version {version!r}.")
    major, minor, patch = (int(part) for part in match.groups())
    return f"{major}.{minor}.{patch + 1}"


def update_version_files(root: Path, version: str) -> None:
    cargo_path = root / "Cargo.toml"
    cargo, count = re.subn(
        r'(?m)^(version\s*=\s*)"[^"]+"$',
        rf'\g<1>"{version}"',
        cargo_path.read_text(encoding="utf-8"),
        count=1,
    )
    if count != 1:
        raise SystemExit("Could not find the Cargo workspace version to update.")
    cargo_path.write_text(cargo, encoding="utf-8")

    for relative_path in (
        "apps/desktop/package.json",
        "apps/desktop/src-tauri/tauri.conf.json",
    ):
        path = root / relative_path
        document = json.loads(path.read_text(encoding="utf-8"))
        document["version"] = version
        path.write_text(json.dumps(document, indent=2) + "\n", encoding="utf-8")

    lock_path = root / "Cargo.lock"
    lockfile = lock_path.read_text(encoding="utf-8")
    local_packages = set(workspace_lock_versions_from_text(lockfile))
    updated_packages: set[str] = set()
    sections = re.split(r"(?=^\[\[package\]\]\s*$)", lockfile, flags=re.MULTILINE)
    for index, section in enumerate(sections):
        name_match = re.search(r'(?m)^name = "([^"]+)"$', section)
        if name_match is None or name_match.group(1) not in local_packages:
            continue
        name = name_match.group(1)
        sections[index], count = re.subn(
            r'(?m)^version = "[^"]+"$',
            f'version = "{version}"',
            section,
            count=1,
        )
        if count != 1:
            raise SystemExit(f"Could not update the Cargo.lock entry for {name}.")
        updated_packages.add(name)
    if updated_packages != local_packages:
        missing = ", ".join(sorted(local_packages - updated_packages))
        raise SystemExit(f"Could not update all Caelum Cargo.lock entries: {missing}")
    lockfile = "".join(sections)
    tomllib.loads(lockfile)
    lock_path.write_text(lockfile, encoding="utf-8")


def set_outputs(tag: str, should_release: bool, commit: str) -> None:
    with Path(os.environ["GITHUB_OUTPUT"]).open("a", encoding="utf-8") as output:
        output.write(f"release_tag={tag}\n")
        output.write(f"release_commit={commit}\n")
        output.write(f"should_release={str(should_release).lower()}\n")


def is_automatic_release_commit(revision: str) -> bool:
    author_name = git("show", "-s", "--format=%an", revision)
    author_email = git("show", "-s", "--format=%ae", revision)
    message = git("show", "-s", "--format=%B", revision)
    return (
        author_name == BOT_NAME
        and author_email == BOT_EMAIL
        and any(line.startswith("Release source: ") for line in message.splitlines())
    )


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


def tag_push_args(tag: str, token: str | None) -> list[str]:
    args = ["git"]
    if token:
        encoded_token = base64.b64encode(f"x-access-token:{token}".encode()).decode()
        auth_header = f"http.https://github.com/.extraheader=AUTHORIZATION: basic {encoded_token}"
        args.extend(
            [
                "-c",
                "http.https://github.com/.extraheader=",
                "-c",
                auth_header,
            ]
        )
    args.extend(["push", "origin", f"refs/tags/{tag}"])
    return args


def automatic_release_commit(source_commit: str, root: Path) -> tuple[str, str]:
    for attempt in range(5):
        subprocess.run(
            ["git", "fetch", "origin", "+refs/heads/main:refs/remotes/origin/main"],
            check=True,
        )
        existing = subprocess.run(
            [
                "git",
                "log",
                "origin/main",
                f"--grep=^Release source: {source_commit}$",
                "--format=%H",
                "-n",
                "1",
            ],
            check=True,
            capture_output=True,
            text=True,
        ).stdout.strip()
        if existing:
            return validate_version(existing), existing

        base_commit = git("rev-parse", "origin/main")
        subprocess.run(["git", "checkout", "--detach", base_commit], check=True)
        version = bump_patch_version(validate_version("HEAD"))
        update_version_files(root, version)
        subprocess.run(["git", "add", *VERSION_FILES], check=True)
        subprocess.run(
            [
                "git",
                "-c",
                f"user.name={BOT_NAME}",
                "-c",
                f"user.email={BOT_EMAIL}",
                "commit",
                "-m",
                f"chore(release): bump version to {version}",
                "-m",
                f"Release source: {source_commit}",
            ],
            check=True,
        )
        commit = git("rev-parse", "HEAD")
        push = subprocess.run(
            ["git", "push", "origin", f"{commit}:refs/heads/main"],
            capture_output=True,
            text=True,
        )
        if push.returncode == 0:
            print(f"Created automatic release version commit {commit} ({version}).")
            return version, commit

        print(
            f"Could not push release version commit on attempt {attempt + 1}; "
            "fetching main and retrying."
        )
        if attempt == 4:
            raise SystemExit(push.stderr.strip() or "Could not push the automatic release commit.")

    raise AssertionError("unreachable")


def create_tag(tag: str, commit: str) -> None:
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
                f"user.name={BOT_NAME}",
                "-c",
                f"user.email={BOT_EMAIL}",
                "tag",
                "-a",
                tag,
                commit,
                "-m",
                f"Release {tag}",
            ],
            check=True,
        )
        subprocess.run(tag_push_args(tag, os.environ.get("GITHUB_TOKEN")), check=True)
        print(f"Created release tag {tag} on {commit}.")
    else:
        print(f"Reusing release tag {tag} on {commit}.")


def main() -> None:
    ref = os.environ["GITHUB_REF"]
    commit = os.environ["GITHUB_SHA"]
    if ref.startswith("refs/tags/"):
        tag = os.environ.get("GITHUB_REF_NAME", ref.removeprefix("refs/tags/"))
        if not tag.startswith("v"):
            raise SystemExit(f"Unexpected release tag: {tag}")
        set_outputs(tag, True, commit)
        return

    if ref != "refs/heads/main":
        raise SystemExit(f"Expected a main branch push or version tag, got {ref}")

    if is_automatic_release_commit(commit):
        print(f"Skipping duplicate release workflow for automated version commit {commit}.")
        set_outputs("", False, commit)
        return

    current = project_versions("HEAD")
    before = os.environ.get("BEFORE_SHA", "")
    previous = None if not before or set(before) == {"0"} else project_versions(before)
    if previous == current:
        version, commit = automatic_release_commit(commit, Path.cwd())
    else:
        version = validate_version("HEAD")

    tag = f"v{version}"
    create_tag(tag, commit)
    set_outputs(tag, True, commit)


if __name__ == "__main__":
    main()

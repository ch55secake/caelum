#!/usr/bin/env python3
"""Publish complete releases through a recoverable draft; preserve published assets."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import subprocess
import tempfile
from pathlib import Path


def asset_names(tag: str) -> set[str]:
    return {
        f"Caelum-{tag}-aarch64.dmg",
        f"Caelum-{tag}-x86_64.dmg",
        f"caelum-cli-{tag}-aarch64-apple-darwin.tar.gz",
        f"caelum-cli-{tag}-x86_64-apple-darwin.tar.gz",
    }


def gh(*args: str) -> str:
    return subprocess.check_output(["gh", *args], text=True).strip()


def release_for_tag(repository: str, tag: str) -> dict | None:
    # REST's tag endpoint cannot find drafts. Resolve the pending tag through
    # GraphQL, then use REST for asset digests and the publication endpoint.
    owner, name = repository.split("/", maxsplit=1)
    result = json.loads(gh(
        "api", "graphql", "-f",
        "query=query($owner:String!,$name:String!,$tag:String!){"
        "repository(owner:$owner,name:$name){release(tagName:$tag){databaseId}}}",
        "-f", f"owner={owner}", "-f", f"name={name}", "-f", f"tag={tag}",
    ))
    release = result["data"]["repository"]["release"]
    if release is None:
        return None
    return json.loads(gh("api", f"repos/{repository}/releases/{release['databaseId']}"))


def parse_checksums(text: str, tag: str) -> dict[str, str]:
    checksums = {}
    for line in text.splitlines():
        match = re.fullmatch(r"([0-9a-f]{64})  (?:\./)?([^/]+)", line)
        if not match or match[2] in checksums:
            raise SystemExit(f"Invalid or duplicate release checksum: {line!r}")
        checksums[match[2]] = match[1]
    if set(checksums) != asset_names(tag):
        raise SystemExit("Release checksums must contain both architecture DMGs and CLI archives.")
    return checksums


def checksum(path: Path) -> str:
    with path.open("rb") as file:
        return hashlib.file_digest(file, "sha256").hexdigest()


def prepare_assets(directory: Path, tag: str) -> dict[str, str]:
    names = asset_names(tag)
    if {path.name for path in directory.iterdir()} - {"SHA256SUMS"} != names:
        raise SystemExit("Expected exactly two DMGs and two CLI archives for this tag.")
    if any(not (directory / name).is_file() or (directory / name).stat().st_size == 0 for name in names):
        raise SystemExit("Release assets must be nonempty files.")
    checksums = {name: checksum(directory / name) for name in sorted(names)}
    (directory / "SHA256SUMS").write_text(
        "".join(f"{digest}  {name}\n" for name, digest in checksums.items()), encoding="utf-8",
    )
    return checksums


def verify_remote_assets(repository: str, tag: str, release: dict, expected: dict | None = None) -> None:
    assets = {asset["name"]: asset for asset in release["assets"]}
    if len(assets) != len(release["assets"]) or set(assets) != asset_names(tag) | {"SHA256SUMS"}:
        raise SystemExit("GitHub release has missing, duplicate, or unexpected assets.")
    if any(asset["state"] != "uploaded" or asset["size"] <= 0 for asset in assets.values()):
        raise SystemExit("GitHub release assets are not fully uploaded.")
    with tempfile.TemporaryDirectory() as temp:
        directory = Path(temp)
        gh("release", "download", tag, "--repo", repository, "--pattern", "SHA256SUMS", "--dir", temp)
        checksums = parse_checksums((directory / "SHA256SUMS").read_text(), tag)
        if expected is not None and checksums != expected:
            raise SystemExit("Uploaded checksums do not match the verified build artifacts.")
        for name, digest in checksums.items():
            # GitHub hashes release assets at upload. Older assets without a
            # digest are downloaded and hashed instead of being assumed valid.
            remote_digest = assets[name].get("digest")
            if not remote_digest:
                gh("release", "download", tag, "--repo", repository, "--pattern", name, "--dir", temp)
                remote_digest = f"sha256:{checksum(directory / name)}"
            if remote_digest != f"sha256:{digest}":
                raise SystemExit(f"Published asset checksum mismatch: {name}")


def installation_notes(repository: str, tag: str) -> str:
    base = f"https://github.com/{repository}/releases/download/{tag}"
    return f"""## Install on macOS

| Mac | Desktop app | Standalone CLI |
| --- | --- | --- |
| Apple Silicon (M-series) | [DMG]({base}/Caelum-{tag}-aarch64.dmg) | [CLI archive]({base}/caelum-cli-{tag}-aarch64-apple-darwin.tar.gz) |
| Intel | [DMG]({base}/Caelum-{tag}-x86_64.dmg) | [CLI archive]({base}/caelum-cli-{tag}-x86_64-apple-darwin.tar.gz) |

Open the DMG and drag **Caelum.app** into **Applications**. For the CLI, extract the archive and put `caelum` in a directory on your `PATH`; run `caelum --version` to check the installation.

### First launch

These builds are **ad-hoc signed and not notarized by Apple**. After attempting to open Caelum, use **System Settings → Privacy & Security → Open Anyway** if macOS blocks it. See [Apple's instructions](https://support.apple.com/en-us/102445). No Apple Developer account is needed to install or use Caelum.

### Verify the download

Download [SHA256SUMS]({base}/SHA256SUMS) beside the selected asset, then run:

```sh
shasum -a 256 --ignore-missing -c SHA256SUMS
```

Homebrew installation and tap-update availability are described in the [release guide](https://github.com/{repository}/blob/main/docs/releasing.md#install).

"""


def publish(repository: str, tag: str, directory: Path) -> None:
    release = release_for_tag(repository, tag)
    if release is not None and not release["draft"]:
        verify_remote_assets(repository, tag, release)
        print(f"Reusing published release {tag}; its assets and checksums are unchanged.")
        return

    checksums = prepare_assets(directory, tag)
    if release is None:
        with tempfile.TemporaryDirectory() as temp:
            notes = Path(temp) / "notes.md"
            notes.write_text(installation_notes(repository, tag), encoding="utf-8")
            gh(
                "release", "create", tag, "--repo", repository, "--verify-tag", "--draft",
                "--title", tag, "--generate-notes", "--notes-file", str(notes),
                *(["--prerelease"] if "-" in tag else []),
            )

    # Only drafts can be repaired. Published releases always take the read-only
    # path above, including recovery after publication but before tap updates.
    release = release_for_tag(repository, tag)
    if release is None or not release["draft"]:
        raise SystemExit("Release changed while preparing it; rerun to inspect the published assets.")
    for asset in release["assets"]:
        if asset["name"] not in asset_names(tag) | {"SHA256SUMS"}:
            raise SystemExit(f"Unexpected draft asset: {asset['name']}")
    gh("release", "upload", tag, "--repo", repository, "--clobber", *(
        str(directory / name) for name in sorted(asset_names(tag) | {"SHA256SUMS"})
    ))
    release = release_for_tag(repository, tag)
    if release is None or not release["draft"]:
        raise SystemExit("Release changed during upload; rerun to inspect its assets.")
    verify_remote_assets(repository, tag, release, checksums)
    gh(
        "api", "--method", "PATCH", f"repos/{repository}/releases/{release['id']}",
        "-F", "draft=false", "-F", f"prerelease={str('-' in tag).lower()}",
        "-f", f"make_latest={'false' if '-' in tag else 'legacy'}",
    )
    print(f"Published complete release {tag}.")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--assets", type=Path, default=Path("dist"))
    args = parser.parse_args()
    tag = os.environ["RELEASE_TAG"]
    repository = os.environ["GITHUB_REPOSITORY"]
    if args.check:
        release = release_for_tag(repository, tag)
        published = release is not None and not release["draft"]
        if published:
            verify_remote_assets(repository, tag, release)
        with Path(os.environ["GITHUB_OUTPUT"]).open("a", encoding="utf-8") as output:
            output.write(f"published={str(published).lower()}\n")
    else:
        publish(repository, tag, args.assets)


if __name__ == "__main__":
    main()

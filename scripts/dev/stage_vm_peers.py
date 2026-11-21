#!/usr/bin/env python3
"""Stage the two headless Linux peers from a trusted Caelum CI build for a Dev bundle."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import tomllib
import zipfile

ROOT = Path(__file__).resolve().parents[2]
REPO = "caelurium/caelum"
VERSION = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["package"]["version"]


def api(path):
    return json.loads(subprocess.check_output(["gh", "api", f"repos/{REPO}/{path}"]))


def stage(run_id, revision):
    run = api(f"actions/runs/{run_id}")
    if (run["head_sha"] != revision or run["head_repository"]["full_name"] != REPO
            or run["path"] != ".github/workflows/ci.yml"):
        raise SystemExit("Expected this repository's CI run for the specified commit.")
    subprocess.run(["git", "merge-base", "--is-ancestor", revision, "HEAD"], cwd=ROOT, check=True)
    artifacts = {a["name"]: a for a in api(f"actions/runs/{run_id}/artifacts")["artifacts"]}
    peers = ROOT / "target" / "vm-peers" / VERSION
    peers.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=peers) as tmp:
        tmp = Path(tmp)
        for arch, machine in (("x86_64", 62), ("aarch64", 183)):
            artifact = artifacts[f"release-linux-{arch}"]
            archive = tmp / "image.zip"
            with archive.open("wb") as out:
                subprocess.run(["gh", "api", f"repos/{REPO}/actions/artifacts/{artifact['id']}/zip"], stdout=out, check=True)
            with archive.open("rb") as f:
                if artifact["digest"] != f"sha256:{hashlib.file_digest(f, 'sha256').hexdigest()}":
                    raise SystemExit("CI archive digest mismatch.")
            image = tmp / "image"
            with zipfile.ZipFile(archive) as z, z.open(f"Caelum-v{VERSION}-{arch}.AppImage") as f, image.open("wb") as out:
                shutil.copyfileobj(f, out)
            blob = image.read_bytes()
            offset = blob.find(b"hsqs")
            while offset >= 0 and blob[offset + 28:offset + 32] != b"\x04\x00\x00\x00":
                offset = blob.find(b"hsqs", offset + 1)
            if offset < 0:
                raise SystemExit("No SquashFS payload in AppImage.")
            binary = tmp / arch
            with binary.open("wb") as out:
                subprocess.run(["unsquashfs", "-cat", "-o", str(offset), image, "usr/bin/caelum"], stdout=out, check=True)
            with binary.open("rb") as f:
                header = f.read(20)
            if header[:6] != b"\x7fELF\x02\x01" or int.from_bytes(header[18:20], "little") != machine:
                raise SystemExit(f"Not an {arch} ELF executable.")
        for arch in ("x86_64", "aarch64"):
            (tmp / arch).replace(peers / arch)
    print(peers)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("run", type=int)
    parser.add_argument("revision", help="full Git commit built by that CI run")
    args = parser.parse_args()
    stage(args.run, args.revision)

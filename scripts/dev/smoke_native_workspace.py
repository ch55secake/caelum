#!/usr/bin/env python3
"""Run the production OpenSSH client, headless peer and provider adapters in an isolated checkout."""
import argparse
import asyncio
import hashlib
import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import tempfile
import tomllib
from contextlib import nullcontext

import asyncssh  # Only needed on the fixture host.
from native_responses import model


ROOT = Path(__file__).resolve().parents[2]
REPO = "example/project"
VERSION = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["package"]["version"]
CLI = ROOT / "target/debug/caelum"
BOOT = (ROOT / "crates/caelum-core/src/remote/native/bootstrap.sh").read_text()


async def run(home, real_opencode, real_codex, api, calls):
    project = home / "Boat's checkout"
    project.mkdir()
    binary = home / ".local/bin"
    binary.mkdir(parents=True)
    remote = {"HOME": str(home), "PATH": f"{binary}:/usr/bin:/bin", "LANG": "C",
              "XDG_DATA_HOME": str(home / "data"), "XDG_CONFIG_HOME": str(home / "config"),
              "XDG_CACHE_HOME": str(home / "cache"), "XDG_STATE_HOME": str(home / "state")}
    peer = CLI
    bundle = home / "bundle"
    bundle.mkdir()
    with peer.open("rb") as f:
        digest = hashlib.file_digest(f, "sha256").hexdigest()
    manifest = {}
    for arch in ("aarch64", "x86_64"):
        (bundle / arch).symlink_to(peer)
        manifest[arch] = [digest, peer.stat().st_size]
    (bundle / "manifest.json").write_text(json.dumps(manifest))
    (binary / "uname").write_text('#!/bin/sh\n[ "$1" != -m ] || { echo x86_64; exit; }\nexec /usr/bin/uname "$@"\n')
    (binary / "uname").chmod(0o700)
    if not Path("/usr/bin/sha256sum").exists():
        (binary / "sha256sum").write_text('#!/bin/sh\nexec /usr/bin/shasum -a 256 "$@"\n')
        (binary / "sha256sum").chmod(0o700)
    for name in ("opencode", "codex"):
        source = ROOT / f"crates/caelum-core/src/{name}/tests/fixtures"
        installed = real_opencode if name == "opencode" else real_codex
        if installed:
            (binary / name).symlink_to(Path(installed).resolve())
            continue
        elif name == "opencode":
            shutil.copy2(source / "managed_server.py", binary / name)
        else:
            shutil.copy2(source / "server.py", binary / "codex.py")
            (binary / name).write_text('#!/bin/sh\n[ "$1" = login ] && exit 0\nexec python3 "$HOME/.local/bin/codex.py"\n')
        (binary / name).chmod(0o700)

    if real_opencode:
        remote["CAELUM_TEST_KEY"] = "test-key"
        (project / "opencode.json").write_text(json.dumps({
            "$schema": "https://opencode.ai/config.json", "model": "fixture/checkout",
            "providers": {"fixture": {
                "package": "@opencode/ai/providers/openai-compatible/responses",
                "env": ["CAELUM_TEST_KEY"], "settings": {"baseURL": api},
                "models": {"checkout": {"name": "Checkout fixture"}},
            }},
        }))
    if real_codex:
        config = home / ".codex"
        config.mkdir()
        (config / "config.toml").write_text(f'''model_provider = "fixture"
model = "gpt-5"
cli_auth_credentials_store = "file"
[model_providers.fixture]
name = "SSH fixture"
base_url = "{api}"
wire_api = "responses"
env_key = "CAELUM_TEST_KEY"
''')
        remote.update(CODEX_HOME=str(config), CAELUM_TEST_KEY="test-key")
        subprocess.run([binary / "codex", "login", "--with-api-key"], env=remote, input=b"test-key\n", check=True)

    def git(*args):
        subprocess.run(["git", *args], cwd=project, env=remote, check=True, stdout=subprocess.DEVNULL)

    git("init", "-b", "feat/native")
    git("remote", "add", "origin", "https://github.com/example/project.git")
    (project / "note.txt").write_text("base\n")
    git("add", "note.txt")
    git("-c", "user.name=Fixture", "-c", "user.email=fixture@example.org", "commit", "-m", "fixture")
    (project / "note.txt").write_text("VM edit\n")
    (project / "ssh-smoke").touch()
    (project / "background-shells.json").write_text(json.dumps({"data": [
        {"id": "sh_example", "command": "check", "status": "running", "time": {"started": 1}, "metadata": {"sessionID": "ses_fake"}}
    ]}))

    refused = subprocess.run([peer, "workspace-peer", "attach", "another/project"], cwd=project, env=remote, capture_output=True)
    assert refused.returncode and "Error" in json.loads(refused.stdout[4:]), "wrong Git origin accepted"
    login = asyncssh.generate_private_key("ssh-ed25519")
    login.write_private_key(home / "client")
    (home / "client").chmod(0o600)
    login.write_public_key(home / "client.pub")
    phases = ["native_new_session_over_openssh", "native_resume_after_vm_stop"] if real_opencode or real_codex else ["native_workspace_over_openssh"]
    for phase in phases:
        previous = len(calls)
        await exercise(home, project, remote, peer, phase)
        if phase == "native_new_session_over_openssh":
            assert len(calls) == 1 if real_codex else len(calls) >= 1
        elif phase == "native_resume_after_vm_stop":
            assert len(calls) == previous, "resuming history contacted the model again"


async def exercise(home, project, remote, peer, phase):
    host = asyncssh.generate_private_key("ssh-ed25519")

    async def attach(channel):
        # This loopback fixture accepts only the exact release bootstrap from this checkout.
        args = shlex.split(channel.command or "")
        assets = json.loads((home / "bundle/manifest.json").read_text())
        flags = [str(v) for arch in ("x86_64", "aarch64") for v in assets[arch]]
        if args != ["sh", "-c", BOOT, "sh", VERSION, REPO, str(project), *flags]:
            channel.exit(1)
            return
        child = await asyncio.create_subprocess_exec(
            *args, cwd=home, env=remote, stdin=asyncio.subprocess.PIPE, stdout=asyncio.subprocess.PIPE,
        )
        async def input_stream():
            try:
                while data := await channel.stdin.read(65536):
                    child.stdin.write(data)
                    await child.stdin.drain()
            finally:
                child.stdin.close()

        incoming = asyncio.create_task(input_stream())
        try:
            while data := await child.stdout.read(65536):
                channel.stdout.write(data)
                await channel.stdout.drain()
            channel.exit(await child.wait())
        finally:
            incoming.cancel()
            await asyncio.gather(incoming, return_exceptions=True)
            if child.returncode is None:
                child.kill()
                await child.wait()

    server = await asyncssh.create_server(
        asyncssh.SSHServer, "127.0.0.1", 0, server_host_keys=[host],
        authorized_client_keys=str(home / "client.pub"), process_factory=attach, encoding=None,
    )
    port = server.sockets[0].getsockname()[1]
    (home / "endpoint.json").write_text(json.dumps({
        "ok": True, "type": "sshkey", "sshEndpoint": f"127.0.0.1:{port}",
        "sshUser": "user", "hostKey": host.export_public_key().decode().strip(),
    }))
    # Hold a handle to the real detached worker so teardown can also verify orderly shutdown.
    worker = await asyncio.create_subprocess_exec(peer, "workspace-peer", "serve", REPO, env=remote, cwd=project)
    try:
        async with asyncio.timeout(10):
            while not list((home / ".local/state/caelum/peers").glob("*.sock")):
                if worker.returncode is not None:
                    raise RuntimeError("workspace peer failed to start")
                await asyncio.sleep(0.02)
        test = await asyncio.create_subprocess_exec(
            "cargo", "test", "--locked", "-p", "caelum-core", phase, "--", "--ignored",
            cwd=ROOT, env={**os.environ, "CAELUM_NATIVE_SSH_FIXTURE": str(home),
                           "CAELUM_NATIVE_SSH_ROOT": str(project),
                           "CAELUM_NATIVE_SMOKE_PROVIDER": "codex" if "CODEX_HOME" in remote else "opencode"},
        )
        if await test.wait():
            raise RuntimeError("native workspace smoke failed")
    finally:
        server.close()
        await server.wait_closed()
        if worker.returncode is None:
            worker.terminate()
        try:
            if await asyncio.wait_for(worker.wait(), 15):
                raise RuntimeError("workspace peer did not shut down cleanly")
        finally:
            if worker.returncode is None:
                worker.kill()
                await worker.wait()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    providers = parser.add_mutually_exclusive_group()
    providers.add_argument("--real-opencode", metavar="BINARY", help="stream and cold-resume a real OpenCode session using a loopback Responses fixture")
    providers.add_argument("--real-codex", metavar="BINARY", help="stream and cold-resume a real Codex thread using a loopback Responses fixture")
    args = parser.parse_args()
    subprocess.run(["cargo", "build", "--locked", "-p", "caelum-cli"], check=True, cwd=ROOT)
    # sockaddr_un is only 104 bytes on macOS; some TMPDIRs alone occupy most of that.
    parent = Path(tempfile.gettempdir()).resolve()
    with tempfile.TemporaryDirectory(prefix="cw-", dir=parent if len(os.fsencode(parent)) < 34 else "/tmp") as temp:
        with model() if args.real_codex or args.real_opencode else nullcontext((None, [])) as (api, calls):
            asyncio.run(run(Path(temp).resolve(), args.real_opencode, args.real_codex, api, calls))

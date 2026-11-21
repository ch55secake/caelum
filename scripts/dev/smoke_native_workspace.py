#!/usr/bin/env python3
"""Run the production OpenSSH client, headless peer and provider adapters in an isolated checkout."""
import argparse
import asyncio
import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import tempfile
import tomllib

import asyncssh  # Only needed on the fixture host.


ROOT = Path(__file__).resolve().parents[2]
REPO = "example/project"
VERSION = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["package"]["version"]
CLI = ROOT / "target/debug/caelum"
BOOT = (ROOT / "crates/caelum-core/src/remote/native/bootstrap.sh").read_text()


async def run(home, real_opencode):
    project = home / "project"
    project.mkdir()
    binary = home / ".local/bin"
    binary.mkdir(parents=True)
    remote = {"HOME": str(home), "PATH": f"{binary}:/usr/bin:/bin", "LANG": "C",
              "XDG_DATA_HOME": str(home / "data"), "XDG_CONFIG_HOME": str(home / "config"),
              "XDG_CACHE_HOME": str(home / "cache"), "XDG_STATE_HOME": str(home / "state")}
    peer = home / f".local/lib/caelum-peer/v{VERSION}/caelum"
    peer.parent.mkdir(parents=True)
    shutil.copy2(CLI, peer)
    for name in ("opencode", "codex"):
        source = ROOT / f"crates/caelum-core/src/{name}/tests/fixtures"
        if name == "opencode" and real_opencode:
            (binary / name).symlink_to(Path(real_opencode).resolve())
            continue
        elif name == "opencode":
            shutil.copy2(source / "managed_server.py", binary / name)
        else:
            shutil.copy2(source / "server.py", binary / "codex.py")
            (binary / name).write_text('#!/bin/sh\n[ "$1" = login ] && exit 0\nexec python3 "$HOME/.local/bin/codex.py"\n')
        (binary / name).chmod(0o700)

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

    refused = subprocess.run([peer, "workspace-peer", "attach", "another/project"], env=remote, capture_output=True)
    assert refused.returncode and "Error" in json.loads(refused.stdout[4:]), "wrong Git origin accepted"
    login = asyncssh.generate_private_key("ssh-ed25519")
    login.write_private_key(home / "client")
    (home / "client").chmod(0o600)
    login.write_public_key(home / "client.pub")
    phases = ["native_workspace_over_openssh"] if not real_opencode else ["native_new_session_over_openssh", "native_resume_after_vm_stop"]
    for phase in phases:
        await exercise(home, remote, peer, phase)


async def exercise(home, remote, peer, phase):
    host = asyncssh.generate_private_key("ssh-ed25519")

    async def attach(channel):
        # This loopback fixture accepts only the exact release bootstrap from this checkout.
        args = shlex.split(channel.command or "")
        if args != ["sh", "-c", BOOT, "sh", VERSION, REPO]:
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
    worker = await asyncio.create_subprocess_exec(peer, "workspace-peer", "serve", REPO, env=remote, cwd=home)
    try:
        async with asyncio.timeout(10):
            while not list((home / ".local/state/caelum/peers").glob("*.sock")):
                if worker.returncode is not None:
                    raise RuntimeError("workspace peer failed to start")
                await asyncio.sleep(0.02)
        test = await asyncio.create_subprocess_exec(
            "cargo", "test", "--locked", "-p", "caelum-core", phase, "--", "--ignored",
            cwd=ROOT, env={**os.environ, "CAELUM_NATIVE_SSH_FIXTURE": str(home)},
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
    parser.add_argument("--real-opencode", metavar="BINARY", help="create and resume a real OpenCode session without model inference")
    args = parser.parse_args()
    subprocess.run(["cargo", "build", "--locked", "-p", "caelum-cli"], check=True, cwd=ROOT)
    # sockaddr_un is only 104 bytes on macOS; some TMPDIRs alone occupy most of that.
    parent = Path(tempfile.gettempdir()).resolve()
    with tempfile.TemporaryDirectory(prefix="cw-", dir=parent if len(os.fsencode(parent)) < 34 else "/tmp") as temp:
        asyncio.run(run(Path(temp).resolve(), args.real_opencode))

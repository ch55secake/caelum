#!/usr/bin/env python3
"""Stage, audit and package native Linux desktop binaries on the glibc 2.35 baseline."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import struct
import subprocess
import tempfile
import tomllib
import urllib.request
from pathlib import Path

TOOL_VERSION = "1.9.1"
TOOL_SHA256 = {
    "aarch64": "f0837e7448a0c1e4e650a93bb3e85802546e60654ef287576f46c71c126a9158",
    "x86_64": "ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0",
}
MACHINES = {"x86_64": 62, "aarch64": 183}
SYSTEM = {"libc.so.6", "libm.so.6", "libdl.so.2", "libpthread.so.0", "librt.so.1", "libresolv.so.2", "libutil.so.1"}
# GPU loader/driver libraries belong to the host; never override its graphics stack.
HOST_GRAPHICS = re.compile(r"lib(?:vulkan|GL|EGL|GLX|GLdispatch|drm|gbm|nvidia).*\.so")
DLOPEN = (
    "libX11.so.6", "libX11-xcb.so.1", "libXcursor.so.1", "libXi.so.6",
    "libXrandr.so.2", "libXfixes.so.3", "libxcb.so.1", "libxkbcommon.so.0",
    "libxkbcommon-x11.so.0", "libwayland-client.so.0", "libfontconfig.so.1",
)


def command(*args: str | Path, **kwargs) -> str:
    return subprocess.check_output([str(arg) for arg in args], text=True, **kwargs).strip()


def dependencies(text: str) -> dict[str, Path]:
    if "not found" in text:
        raise SystemExit(f"Unresolved Linux libraries:\n{text}")
    return {match[1]: Path(match[2]) for match in re.finditer(r"^\s*(\S+) => (/\S+) \(", text, re.MULTILINE)}


def host_library(name: str) -> bool:
    return name in SYSTEM or name.startswith("ld-linux-") or bool(HOST_GRAPHICS.match(name))


def audit_elf(path: Path, arch: str) -> None:
    with path.open("rb") as file:
        header = file.read(20)
    if header[:6] != b"\x7fELF\x02\x01" or struct.unpack_from("<H", header, 18)[0] != MACHINES[arch]:
        raise SystemExit(f"Expected a native {arch} ELF: {path.name}")
    versions = command("readelf", "--version-info", path)
    required = [tuple(map(int, value.split("."))) for value in re.findall(r"GLIBC_(\d+\.\d+(?:\.\d+)?)", versions)]
    if required and max(required) > (2, 35):
        raise SystemExit(f"{path.name} requires GLIBC_{'.'.join(map(str, max(required)))}; build on Ubuntu 22.04")
    dynamic = command("readelf", "--dynamic", "--program-headers", path)
    if "/nix/store/" in dynamic:
        raise SystemExit(f"Machine-specific Nix dependency in {path.name}; package in the baseline distribution")


def bundle_libraries(binary: Path, destination: Path, arch: str) -> None:
    cache = command("ldconfig", "-p")
    available = {match[1]: Path(match[2]) for match in re.finditer(r"^\s*(\S+) \([^\n]+\) => (/\S+)$", cache, re.MULTILINE)}
    libraries = dependencies(command("ldd", binary))
    for name in DLOPEN:
        if name not in available:
            raise SystemExit(f"Install the build dependency {name} before packaging")
        libraries[name] = available[name]
        libraries.update(dependencies(command("ldd", available[name])))
    destination.mkdir(parents=True)
    notices = destination.parent / "share/doc/caelum/system-libraries"
    notices.mkdir(parents=True)
    for name, source in sorted(libraries.items()):
        if host_library(name):
            continue
        if "/nix/store/" in str(source):
            raise SystemExit(f"Cannot redistribute a Nix-linked library: {name}")
        target = destination / name
        shutil.copy2(source, target)
        audit_elf(target, arch)
        subprocess.run(["patchelf", "--set-rpath", "$ORIGIN", str(target)], check=True)
        # Keep Debian's licensing/source references alongside every redistributed library.
        package = library_package(source)
        copyright_file = Path("/usr/share/doc") / package / "copyright"
        if not copyright_file.is_file():
            raise SystemExit(f"Missing copyright notice for {name} ({package})")
        shutil.copy2(copyright_file, notices / f"{package}.copyright")
    # RUNPATH is confined to these ELFs. Provider/git/gh subprocesses inherit no
    # AppImage LD_LIBRARY_PATH override that could break their system libraries.
    subprocess.run(["patchelf", "--set-rpath", "$ORIGIN/../lib", str(binary)], check=True)


def library_package(source: Path) -> str:
    # Ubuntu's merged-/usr symlinks and dpkg's recorded paths need not agree.
    candidates = [source, source.resolve()]
    candidates.extend(Path("/usr") / path.relative_to("/") for path in list(candidates) if str(path).startswith("/lib/"))
    candidates.extend(Path("/") / path.relative_to("/usr") for path in list(candidates) if str(path).startswith("/usr/lib/"))
    for path in candidates:
        result = subprocess.run(["dpkg-query", "-S", str(path)], capture_output=True, text=True)
        if result.returncode == 0:
            return result.stdout.split(": ", 1)[0].split(":", 1)[0]
    raise SystemExit(f"Cannot identify the distribution package for {source.name}")


def clear_runtime_metadata(runtime: bytearray) -> bytearray:
    """Reuse the pinned tool's runtime without inheriting its update URL/signature."""
    header = struct.unpack_from("<HHIQQQIHHHHHH", runtime, 16)
    offset, size, count, names_index = header[5], header[10], header[11], header[12]
    sections = [struct.unpack_from("<IIQQQQIIQQ", runtime, offset + index * size) for index in range(count)]
    names = sections[names_index]
    strings = runtime[names[4]:names[4] + names[5]]
    for section in sections:
        name = strings[section[0]:].split(b"\0", 1)[0]
        if name in (b".upd_info", b".sha256_sig", b".sig_key", b".digest_md5"):
            start, length = section[4:6]
            if start + length > len(runtime):
                raise SystemExit("Invalid AppImage runtime section")
            runtime[start:start + length] = b"\0" * length
    return runtime


def appimage_tool(directory: Path, arch: str) -> tuple[Path, Path]:
    tool = directory / "appimagetool.AppImage"
    url = f"https://github.com/AppImage/appimagetool/releases/download/{TOOL_VERSION}/appimagetool-{arch}.AppImage"
    with urllib.request.urlopen(url, timeout=60) as response, tool.open("wb") as file:
        shutil.copyfileobj(response, file)
    if hashlib.sha256(tool.read_bytes()).hexdigest() != TOOL_SHA256[arch]:
        raise SystemExit("appimagetool checksum mismatch")
    tool.chmod(0o755)
    offset = int(command(tool, "--appimage-offset"))
    runtime = directory / "runtime"
    runtime.write_bytes(clear_runtime_metadata(bytearray(tool.read_bytes()[:offset])))
    return tool, runtime


def stage(root: Path, app: Path, target: str, tag: str) -> Path:
    arch = target.split("-", 1)[0]
    binary = app / "usr/bin/caelum-desktop"
    binary.parent.mkdir(parents=True)
    shutil.copy2(root / "target" / target / "release/caelum-desktop", binary)
    audit_elf(binary, arch)
    bundle_libraries(binary, app / "usr/lib", arch)
    # The VM extracts only this headless executable; it never launches the GUI.
    peer = app / "usr/bin/caelum"
    shutil.copy2(root / "target" / target / "release/caelum", peer)
    audit_elf(peer, arch)
    assets = root / "apps/desktop/assets"
    for source, name in [(assets / "linux/AppRun", "AppRun"), (assets / "linux/dev.caelum.desktop.desktop", "dev.caelum.desktop.desktop"), (assets / "linux/dev.caelum.desktop.png", "dev.caelum.desktop.png")]:
        shutil.copy2(source, app / name)
    (app / "AppRun").chmod(0o755)
    notices = app / "usr/share/doc/caelum"
    shutil.copy2(root / "LICENSE", notices / "LICENSE")
    shutil.copy2(root / "apps/desktop/THIRD_PARTY_NOTICES.md", notices / "THIRD_PARTY_NOTICES.md")
    (notices / "release.json").write_text(json.dumps({"version": tag[1:], "revision": command("git", "-C", root, "rev-parse", "HEAD"), "target": target, "glibc": "2.35"}, indent=2) + "\n")
    subprocess.run(["desktop-file-validate", str(app / "dev.caelum.desktop.desktop")], check=True)
    if command(binary, "--version") != f"Caelum {tag[1:]}":
        raise SystemExit("Packaged binary version does not match release")
    return binary


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--target", choices=[f"{arch}-unknown-linux-gnu" for arch in MACHINES], required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    root, output = args.root.resolve(), args.output.resolve()
    version = tomllib.loads((root / "Cargo.toml").read_text())["workspace"]["package"]["version"]
    tag = os.environ.get("RELEASE_TAG") or f"v{version}"
    if tag != f"v{version}":
        raise SystemExit(f"Tag {tag} must match project version v{version}")
    arch = args.target.split("-", 1)[0]
    output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="caelum-appimage-", dir=os.environ.get("RUNNER_TEMP")) as temporary:
        directory = Path(temporary)
        app = directory / "Caelum.AppDir"
        stage(root, app, args.target, tag)
        tool, runtime = appimage_tool(directory, arch)
        image = output / f"Caelum-{tag}-{arch}.AppImage"
        subprocess.run([str(tool), "--no-appstream", "--runtime-file", str(runtime), str(app), str(image)], check=True, env={**os.environ, "ARCH": arch, "VERSION": version, "APPIMAGE_EXTRACT_AND_RUN": "1"})
        if command(image, "--appimage-extract-and-run", "--version") != f"Caelum {version}":
            raise SystemExit("AppImage startup/version verification failed")
        # Exercise exactly the extraction used by the VM, outside the desktop library bundle.
        command(image, "--appimage-extract", "usr/bin/caelum", cwd=directory)
        if command(directory / "squashfs-root/usr/bin/caelum", "--version") != f"caelum {version}":
            raise SystemExit("Extracted workspace peer version does not match the desktop")
        print(image)


if __name__ == "__main__":
    main()

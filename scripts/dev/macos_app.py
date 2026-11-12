#!/usr/bin/env python3
"""Run the debug executable as a registered, signed macOS app, retaining terminal I/O."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import plistlib
import shutil
import subprocess
import sys
import tomllib


def stage(root: Path, binary: Path) -> Path:
    app = binary.parent / "bundle" / "macos" / "Caelum Dev.app"
    executable = app / "Contents" / "MacOS" / "caelum-desktop"
    resources = app / "Contents" / "Resources"
    executable.parent.mkdir(parents=True, exist_ok=True)
    resources.mkdir(parents=True, exist_ok=True)
    # Replacing the inode also permits rebuilding while an older process exits.
    temporary = executable.with_suffix(".new")
    shutil.copy2(binary, temporary)
    temporary.replace(executable)
    shutil.copy2(root / "apps/desktop/assets/icon.icns", resources / "icon.icns")
    version = tomllib.loads((root / "Cargo.toml").read_text())["workspace"]["package"]["version"]
    checkout = hashlib.sha256(os.fsencode(root.resolve())).hexdigest()[:12]
    data = dict(
        CFBundleName="Caelum Dev", CFBundleDisplayName="Caelum Dev",
        CFBundleIdentifier=f"dev.caelum.desktop.dev.{checkout}",
        CFBundleExecutable="caelum-desktop", CFBundlePackageType="APPL",
        CFBundleShortVersionString=version, CFBundleVersion=version,
        CFBundleIconFile="icon.icns", NSHighResolutionCapable=True,
        LSMinimumSystemVersion="13.0", NSPrincipalClass="NSApplication",
        LSApplicationCategoryType="public.app-category.developer-tools",
    )
    (app / "Contents/Info.plist").write_bytes(plistlib.dumps(data))
    return app


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--build-only", action="store_true")
    parser.add_argument("args", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    if sys.platform != "darwin":
        parser.error("the native desktop requires macOS")
    root = Path(__file__).resolve().parents[2]
    result = subprocess.run(
        ["cargo", "build", "--locked", "-p", "caelum-desktop", "--message-format=json-render-diagnostics"],
        cwd=root, text=True, stdout=subprocess.PIPE, check=True,
    )
    artifacts = [json.loads(line) for line in result.stdout.splitlines() if line.startswith("{")]
    binary = next(Path(item["executable"]) for item in artifacts
                  if item.get("reason") == "compiler-artifact" and item.get("executable")
                  and item["target"]["name"] == "caelum-desktop")
    app = stage(root, binary)
    subprocess.run(["codesign", "--force", "--sign", "-", "--timestamp=none", str(app)], check=True)
    subprocess.run(["codesign", "--verify", "--deep", "--strict", str(app)], check=True)
    subprocess.run([
        "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister",
        "-f", str(app),
    ], check=True)
    if args.build_only:
        print(app)
        return
    executable = str(app / "Contents/MacOS/caelum-desktop")
    forwarded = args.args[1:] if args.args[:1] == ["--"] else args.args
    os.execv(executable, [executable, *forwarded])


if __name__ == "__main__":
    main()

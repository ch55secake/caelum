#!/usr/bin/env bash
set -euo pipefail
root="$(git rev-parse --show-toplevel)"
TARGET="${TARGET:-$(rustc -vV | awk '/^host:/ { print $2 }')}"
case "$TARGET" in
  aarch64-apple-darwin|x86_64-apple-darwin) ;;
  *) echo "Unsupported desktop target: $TARGET" >&2; exit 1 ;;
esac
export MACOSX_DEPLOYMENT_TARGET=13.0
cargo build --locked --release -p caelum-desktop --target "$TARGET" --manifest-path "$root/Cargo.toml"
output="${1:-$root/target/$TARGET/release/bundle/macos}"
mkdir -p "$output"
output="$(cd "$output" && pwd)"
stage="$(mktemp -d "${TMPDIR:-/tmp}/caelum-app.XXXXXX")"
trap 'rm -rf "$stage"' EXIT
app="$stage/Caelum.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
binary="$app/Contents/MacOS/caelum-desktop"
cp "$root/target/$TARGET/release/caelum-desktop" "$binary"
# shellcheck source=scripts/release/macos_libraries.sh
source "$root/scripts/release/macos_libraries.sh"
system_libraries "$binary"
cp "$root/apps/desktop/assets/icon.icns" "$app/Contents/Resources/icon.icns"
python3 - "$root" "$app" <<'PY'
import plistlib, subprocess, sys, tomllib
from pathlib import Path
root, app = map(Path, sys.argv[1:])
version = tomllib.loads((root / 'Cargo.toml').read_text())['workspace']['package']['version']
revision = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip()
data = dict(CFBundleName='Caelum', CFBundleDisplayName='Caelum', CFBundleIdentifier='dev.caelum.desktop',
            CFBundleExecutable='caelum-desktop', CFBundlePackageType='APPL', CFBundleShortVersionString=version,
            CFBundleVersion=version, CFBundleIconFile='icon.icns', NSHighResolutionCapable=True,
            LSMinimumSystemVersion='13.0', LSApplicationCategoryType='public.app-category.developer-tools',
            NSPrincipalClass='NSApplication', CaelumRevision=revision,
            CFBundleURLTypes=[dict(CFBundleURLName='Caelum project', CFBundleURLSchemes=['caelum'])])
(app / 'Contents/Info.plist').write_bytes(plistlib.dumps(data))
PY
codesign --force --sign - --timestamp=none "$app"
codesign --verify --deep --strict "$app"
plutil -lint "$app/Contents/Info.plist"
# Replace only the generated bundle, so deleted resources cannot survive a rebuild.
rm -rf "$output/Caelum.app"
ditto "$app" "$output/Caelum.app"
echo "$output/Caelum.app"

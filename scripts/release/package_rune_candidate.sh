#!/usr/bin/env bash
set -euo pipefail

# Run from the repository root with the pinned candidate toolchain available.
root="$(git rev-parse --show-toplevel)"
revision="$(git rev-parse --short HEAD)"
output="${1:-$root/apps/rune-prototype/target/evaluation/$revision}"
mkdir -p "$output"
output="$(cd "$output" && pwd)"
archive="$output/Caelum-Rune-Evaluation-$(uname -m).zip"
if [[ -e "$archive" ]]; then
  echo "Archive already exists: $archive (choose a new output directory)" >&2
  exit 1
fi
cargo build --locked --release --manifest-path "$root/apps/rune-prototype/Cargo.toml"
stage="$(mktemp -d "${TMPDIR:-/tmp}/caelum-rune-package.XXXXXX")"
trap 'rm -rf "$stage"' EXIT
app="$stage/Caelum Rune Evaluation.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
binary="$app/Contents/MacOS/caelum-rune"
cp "$root/apps/rune-prototype/target/release/caelum-rune-prototype" "$binary"
# Nix links its copy of Apple's libiconv; evaluation builds use the macOS copy.
while IFS= read -r library; do
  case "$library" in
    /nix/store/*/lib/libiconv.2.dylib)
      install_name_tool -change "$library" /usr/lib/libiconv.2.dylib "$binary"
      ;;
    /System/Library/*|/usr/lib/*) ;;
    *)
      echo "Unsupported non-system dependency: $library" >&2
      exit 1
      ;;
  esac
done < <(otool -L "$binary" | awk 'NR > 1 { print $1 }')
cp "$root/apps/desktop/src-tauri/icons/icon.icns" "$app/Contents/Resources/icon.icns"
python3 - "$root" "$app" "$revision" <<'PY'
import plistlib, sys, tomllib
from pathlib import Path
root, app, revision = map(Path, sys.argv[1:])
version = tomllib.loads((root / 'Cargo.toml').read_text())['workspace']['package']['version']
data = dict(CFBundleName='Caelum Rune Evaluation', CFBundleDisplayName='Caelum Rune Evaluation',
            CFBundleIdentifier='dev.caelum.rune-evaluation', CFBundleExecutable='caelum-rune',
            CFBundlePackageType='APPL', CFBundleShortVersionString=version,
            CFBundleVersion=version, CFBundleIconFile='icon.icns',
            NSHighResolutionCapable=True, LSMinimumSystemVersion='13.0',
            NSPrincipalClass='NSApplication', CaelumRevision=str(revision))
(app / 'Contents/Info.plist').write_bytes(plistlib.dumps(data))
PY
codesign --force --sign - --timestamp=none "$app"
codesign --verify --deep --strict "$app"
plutil -lint "$app/Contents/Info.plist"
ditto -c -k --sequesterRsrc --keepParent "$app" "$archive"
ditto "$app" "$output/Caelum Rune Evaluation.app"
echo "$archive"

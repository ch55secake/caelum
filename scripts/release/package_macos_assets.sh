#!/usr/bin/env bash
set -euo pipefail

: "${TARGET:?TARGET must be set to the Rust target triple}"
: "${ARCH:?ARCH must be set to the architecture name}"
: "${RELEASE_TAG:?RELEASE_TAG must be set to the release tag}"
: "${RUNNER_TEMP:?RUNNER_TEMP must be set by GitHub Actions}"

version="${RELEASE_TAG#v}"
export MACOSX_DEPLOYMENT_TARGET=13.0
dist="$RUNNER_TEMP/release"
case "$ARCH:$TARGET" in
  aarch64:aarch64-apple-darwin) machine=arm64 ;;
  x86_64:x86_64-apple-darwin) machine=x86_64 ;;
  *) echo "Architecture and Rust target do not match" >&2; exit 1 ;;
esac
scratch="$(mktemp -d "$RUNNER_TEMP/caelum-package.XXXXXX")"
mounted=false
cleanup() {
  if [[ "$mounted" == true ]]; then hdiutil detach "$scratch/mount" >/dev/null; fi
  rm -rf "$scratch"
}
trap cleanup EXIT
mkdir -p "$dist" "$scratch/cli" "$scratch/mount" "$scratch/extracted"

verify_binary() {
  test "$(lipo -archs "$1")" = "$machine"
  codesign --verify --strict --verbose=2 "$1"
  codesign --display --verbose=4 "$1" 2>&1 | grep '^Signature=adhoc$' >/dev/null
}
verify_app() {
  local app="$1" executable
  test "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$app/Contents/Info.plist")" = "$version"
  test "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$app/Contents/Info.plist")" = "dev.caelum.desktop"
  executable="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$app/Contents/Info.plist")"
  codesign --verify --deep --strict --verbose=2 "$app"
  verify_binary "$app/Contents/MacOS/$executable"
  test "$("$app/Contents/MacOS/$executable" --version)" = "Caelum $version"
  test "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleURLTypes:0:CFBundleURLSchemes:0' "$app/Contents/Info.plist")" = "caelum"
}

app_bundle="target/$TARGET/release/bundle/macos/Caelum.app"
bash scripts/release/package_macos_app.sh
verify_app "$app_bundle"

cargo build --locked --release -p caelum-cli --target "$TARGET"
cp "target/$TARGET/release/caelum" "$scratch/cli/caelum"
source scripts/release/macos_libraries.sh
system_libraries "$scratch/cli/caelum"
codesign --force --sign - --timestamp=none "$scratch/cli/caelum"
COPYFILE_DISABLE=1 tar -C "$scratch/cli" -czf "$dist/caelum-cli-v${version}-${TARGET}.tar.gz" caelum
tar -xzf "$dist/caelum-cli-v${version}-${TARGET}.tar.gz" -C "$scratch/extracted"
verify_binary "$scratch/extracted/caelum"
test "$("$scratch/extracted/caelum" --version)" = "caelum $version"
"$scratch/extracted/caelum" --help >/dev/null

mkdir -p "$scratch/image"
ditto "$app_bundle" "$scratch/image/Caelum.app"
ln -s /Applications "$scratch/image/Applications"
dmg="$dist/Caelum-v${version}-${ARCH}.dmg"
hdiutil create -ov -volname Caelum -srcfolder "$scratch/image" -format UDZO "$dmg"
hdiutil verify "$dmg"
hdiutil attach "$dmg" -readonly -nobrowse -mountpoint "$scratch/mount"
mounted=true
verify_app "$scratch/mount/Caelum.app"
diff -qr "$app_bundle" "$scratch/mount/Caelum.app"
python3 scripts/release/smoke_macos_app.py --app "$app_bundle" --cli "$scratch/extracted/caelum"

echo "Verified $ARCH app, DMG, and standalone CLI for $RELEASE_TAG"

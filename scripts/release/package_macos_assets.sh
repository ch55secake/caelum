#!/usr/bin/env bash
set -euo pipefail

: "${TARGET:?TARGET must be set to the Rust target triple}"
: "${ARCH:?ARCH must be set to the architecture name}"
: "${RELEASE_TAG:?RELEASE_TAG must be set to the release tag}"
: "${RUNNER_TEMP:?RUNNER_TEMP must be set by GitHub Actions}"

version="${RELEASE_TAG#v}"
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
}

app_bundle="target/$TARGET/release/bundle/macos/Caelum.app"
verify_app "$app_bundle"

cp "target/$TARGET/release/caelum" "$scratch/cli/caelum"
codesign --force --sign - --timestamp=none "$scratch/cli/caelum"
COPYFILE_DISABLE=1 tar -C "$scratch/cli" -czf "$dist/caelum-cli-v${version}-${TARGET}.tar.gz" caelum
tar -xzf "$dist/caelum-cli-v${version}-${TARGET}.tar.gz" -C "$scratch/extracted"
verify_binary "$scratch/extracted/caelum"
test "$("$scratch/extracted/caelum" --version)" = "caelum $version"
"$scratch/extracted/caelum" --help >/dev/null

shopt -s nullglob
dmgs=("target/$TARGET/release/bundle/dmg/"*.dmg)
test "${#dmgs[@]}" -eq 1
dmg="$dist/Caelum-v${version}-${ARCH}.dmg"
cp "${dmgs[0]}" "$dmg"
hdiutil verify "$dmg"
hdiutil attach "$dmg" -readonly -nobrowse -mountpoint "$scratch/mount"
mounted=true
verify_app "$scratch/mount/Caelum.app"
diff -qr "$app_bundle" "$scratch/mount/Caelum.app"

echo "Verified $ARCH app, DMG, and standalone CLI for $RELEASE_TAG"

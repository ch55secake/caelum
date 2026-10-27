#!/usr/bin/env bash
set -euo pipefail

: "${TARGET:?TARGET must be set to the Rust target triple}"
: "${ARCH:?ARCH must be set to the architecture name}"
: "${RELEASE_TAG:?RELEASE_TAG must be set to the release tag}"
: "${RUNNER_TEMP:?RUNNER_TEMP must be set by GitHub Actions}"

version="${RELEASE_TAG#v}"
dist="$RUNNER_TEMP/release"
mkdir -p "$dist/cli"

cp "target/$TARGET/release/caelum" "$dist/cli/caelum"
COPYFILE_DISABLE=1 tar -C "$dist/cli" -czf "$dist/caelum-cli-v${version}-${TARGET}.tar.gz" caelum

app_bundle="target/$TARGET/release/bundle/macos/Caelum.app"
test -d "$app_bundle"

dmg_path="$(find "target/$TARGET/release/bundle/dmg" -maxdepth 1 -type f -name '*.dmg' -print -quit)"
test -n "$dmg_path"
cp "$dmg_path" "$dist/Caelum-v${version}-${ARCH}.dmg"

rm -rf "$dist/cli"

#!/usr/bin/env bash
set -euo pipefail
root="$(git rev-parse --show-toplevel)"
TARGET="${TARGET:-$(rustc -vV | awk '/^host:/ { print $2 }')}"
case "$TARGET" in
  aarch64-unknown-linux-gnu|x86_64-unknown-linux-gnu) ;;
  *) echo "Unsupported Linux desktop target: $TARGET" >&2; exit 1 ;;
esac
if [[ "${TARGET%%-*}" != "$(uname -m)" ]]; then
  echo "Package and verify Linux releases on a native $TARGET host" >&2
  exit 1
fi
output="${RUNNER_TEMP:-$root/target}/release"
mkdir -p "$output"
cargo build --locked --release --timings -p caelum-desktop --target "$TARGET" --manifest-path "$root/Cargo.toml"
python3 "$root/scripts/release/linux_appimage.py" --root "$root" --target "$TARGET" --output "$output"

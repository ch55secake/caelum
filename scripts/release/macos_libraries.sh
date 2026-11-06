#!/usr/bin/env bash
# Normalize the known Nix libiconv link, rejecting dependencies that cannot ship.
system_libraries() {
  local binary="$1" library
  while IFS= read -r library; do
    case "$library" in
      /nix/store/*/lib/libiconv.2.dylib)
        install_name_tool -change "$library" /usr/lib/libiconv.2.dylib "$binary" ;;
      /System/Library/*|/usr/lib/*) ;;
      *) echo "Unsupported non-system dependency: $library" >&2; return 1 ;;
    esac
  done < <(otool -L "$binary" | awk 'NR > 1 { print $1 }')
}

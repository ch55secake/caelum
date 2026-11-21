# Invoked over authenticated SSH with the desktop release version and verified repository slug.
set -eu
umask 077
version=$1
repo=$2
dir="$HOME/.local/lib/caelum-peer/v$version"
peer="$dir/caelum"
if [ ! -x "$peer" ]; then
    arch=$(uname -m)
    case "$arch" in x86_64|aarch64) ;; *) exit 65 ;; esac
    mkdir -p "$dir"
    stage=$(mktemp -d "$dir/.install.XXXXXXXX")
    trap 'rm -rf "$stage"' EXIT
    cd "$stage"
    asset="Caelum-v$version-$arch.AppImage"
    base="https://github.com/caelurium/caelum/releases/download/v$version"
    fetch() { curl --fail --silent --show-error --location --proto '=https' --proto-redir '=https' --max-time 90 "$1" -o "$2"; }
    fetch "$base/SHA256SUMS" sums || exit 66
    awk -v f="$asset" '($2==f || $2=="./"f) && length($1)==64 && $1~/^[0-9a-f]+$/ {print $1 "  " f; n++} END {if(n!=1) exit 1}' sums > expected || exit 66
    fetch "$base/$asset" "$asset" || exit 66
    sha256sum --check --status expected || exit 66
    chmod 700 "$asset"
    "./$asset" --appimage-extract 'usr/bin/caelum' >/dev/null || exit 66
    [ -x squashfs-root/usr/bin/caelum ] || exit 66
    mv squashfs-root/usr/bin/caelum "$peer"
    cd "$HOME"
    rm -rf "$stage"
    trap - EXIT
fi
exec "$peer" workspace-peer attach "$repo"

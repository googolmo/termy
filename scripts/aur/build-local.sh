#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

usage() {
  cat <<EOF
Usage: $0 <render|build|install> [version]

Renders scripts/aur/PKGBUILD.template into target/aur/termy-bin-<version>-<arch>,
downloads the release sources needed for checksum substitution, and optionally
builds or installs the package.

Environment:
  MAKEPKG_FLAGS  Extra makepkg flags for build/install modes.
                 Default: --nodeps --force --noconfirm --skippgpcheck -C
  PACMAN_FLAGS   Extra pacman flags for install mode.
EOF
}

die() {
  echo "Error: $*" >&2
  exit 1
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || die "'$1' is required"
}

read_version_from_cargo_toml() {
  awk '
    /^\[package\]$/ { in_package = 1; next }
    /^\[/ && in_package { exit }
    in_package && $1 == "version" {
      gsub(/"/, "", $3)
      print $3
      exit
    }
  ' "$REPO_ROOT/crates/desktop_app/Cargo.toml"
}

mode="${1:-}"
version="${2:-}"

case "$mode" in
  render|build|install) ;;
  -h|--help|"")
    usage
    [[ -n "$mode" ]] || exit 1
    exit 0
    ;;
  *)
    usage >&2
    die "unknown mode: $mode"
    ;;
esac

if [[ -z "$version" ]]; then
  version="$(read_version_from_cargo_toml)"
  [[ -n "$version" ]] || die "could not read version from crates/desktop_app/Cargo.toml"
fi

carch="$(uname -m)"
[[ "$carch" == "x86_64" ]] || die "AUR package template currently supports x86_64 only (got: $carch)"

require_cmd b2sum
require_cmd curl
if [[ "$mode" != "render" ]]; then
  require_cmd makepkg
fi
if [[ "$mode" == "install" ]]; then
  require_cmd pacman
  if [[ "$EUID" -ne 0 ]]; then
    require_cmd sudo
  fi
fi

aur_dir="$REPO_ROOT/target/aur/termy-bin-${version}-${carch}"
mkdir -p "$aur_dir"

desktop_path="$aur_dir/termy.desktop"
license_path="$aur_dir/LICENSE"
icon_path="$aur_dir/termy_icon.png"
tarball_path="$aur_dir/termy-${version}-${carch}.tar.gz"
pkgbuild_path="$aur_dir/PKGBUILD"

cp "$SCRIPT_DIR/termy.desktop" "$desktop_path"
curl -fsSL -o "$license_path" "https://raw.githubusercontent.com/lassejlv/termy/v${version}/LICENSE"
curl -fsSL -o "$icon_path" "https://raw.githubusercontent.com/lassejlv/termy/v${version}/assets/termy_icon.png"
curl -fsSL -o "$tarball_path" \
  "https://github.com/lassejlv/termy/releases/download/v${version}/Termy-v${version}-linux-${carch}.tar.gz"

desktop_sum="$(b2sum "$desktop_path" | cut -d' ' -f1)"
license_sum="$(b2sum "$license_path" | cut -d' ' -f1)"
icon_sum="$(b2sum "$icon_path" | cut -d' ' -f1)"
tarball_sum="$(b2sum "$tarball_path" | cut -d' ' -f1)"

sed -e "s|@PKGVER@|$version|g" \
    -e "s|@TARBALL_B2SUM@|$tarball_sum|g" \
    -e "s|@DESKTOP_B2SUM@|$desktop_sum|g" \
    -e "s|@LICENSE_B2SUM@|$license_sum|g" \
    -e "s|@ICON_B2SUM@|$icon_sum|g" \
    "$SCRIPT_DIR/PKGBUILD.template" > "$pkgbuild_path"

echo "Rendered $pkgbuild_path"

if [[ "$mode" == "render" ]]; then
  exit 0
fi

read -r -a makepkg_flags <<< "${MAKEPKG_FLAGS:---nodeps --force --noconfirm --skippgpcheck -C}"
(cd "$aur_dir" && makepkg "${makepkg_flags[@]}")

pkgfile="$(find "$aur_dir" -maxdepth 1 -type f -name "termy-bin-${version}-*.pkg.tar.*" | sort -V | tail -n1)"
[[ -n "$pkgfile" ]] || die "package artifact not found in $aur_dir"

echo "Built $pkgfile"

if [[ "$mode" != "install" ]]; then
  exit 0
fi

pacman_cmd=(pacman -U)
if [[ -n "${PACMAN_FLAGS:-}" ]]; then
  read -r -a pacman_flags <<< "$PACMAN_FLAGS"
  pacman_cmd+=("${pacman_flags[@]}")
fi
pacman_cmd+=("$pkgfile")

if [[ "$EUID" -eq 0 ]]; then
  "${pacman_cmd[@]}"
else
  sudo "${pacman_cmd[@]}"
fi

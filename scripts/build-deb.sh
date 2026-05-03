#!/usr/bin/env bash
# Build a binary .deb for Rhino Player (Debian, Ubuntu, and derivatives).
# Prerequisites: Rust toolchain, `cargo build --release` deps (see README),
# and `dpkg-deb` (package `dpkg`).
#
# Output: releases/rhino-player_<version>-<rev>_<arch>.deb  (override with OUTPUT=)
# Package revision for rebuilds: DEB_REV=2 ./scripts/build-deb.sh
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

VERSION="$(grep -m1 '^version\s*=' Cargo.toml | sed -E 's/^version\s*=\s*"([^"]+)".*/\1/')"
DEB_REV="${DEB_REV:-1}"
FULL_VER="${VERSION}-${DEB_REV}"

DEB_ARCH="${DEB_ARCH:-}"
if [[ -z "$DEB_ARCH" ]]; then
  case "$(uname -m)" in
    x86_64) DEB_ARCH=amd64 ;;
    aarch64 | arm64) DEB_ARCH=arm64 ;;
    *)
      echo "Unsupported machine $(uname -m); set DEB_ARCH manually (e.g. armhf)." >&2
      exit 1
      ;;
  esac
fi

# Empty rustc-wrapper overrides a global sccache entry when the daemon is down.
cargo build --release --config 'build.rustc-wrapper=""'

STAGE="$(mktemp -d)"
cleanup() { rm -rf "$STAGE"; }
trap cleanup EXIT

INST="$STAGE/usr"
install -d -m 0755 "$INST/bin" "$INST/share/rhino-player/vs" \
  "$INST/share/applications" "$INST/share/metainfo" "$INST/share/icons" \
  "$INST/share/man/man1"

install -m 0755 "$REPO_ROOT/target/release/rhino-player" "$INST/bin/"
install -m 0644 "$REPO_ROOT/data/vs"/*.vpy "$INST/share/rhino-player/vs/"
cp -a "$REPO_ROOT/data/icons/hicolor" "$INST/share/icons/"
install -m 0644 "$REPO_ROOT/data/applications/ch.rhino.RhinoPlayer.desktop" \
  "$INST/share/applications/"
install -m 0644 "$REPO_ROOT/data/metainfo/ch.rhino.RhinoPlayer.metainfo.xml" \
  "$INST/share/metainfo/"
gzip -9 -n -c "$REPO_ROOT/doc/rhino-player.1" >"$INST/share/man/man1/rhino-player.1.gz"
chmod 0644 "$INST/share/man/man1/rhino-player.1.gz"

DEBIAN="$STAGE/DEBIAN"
install -d -m 0755 "$DEBIAN"

cat >"$DEBIAN/control" <<EOF
Package: rhino-player
Version: $FULL_VER
Section: video
Priority: optional
Architecture: $DEB_ARCH
Depends: libgtk-4-1, libadwaita-1-0, libmpv2 | libmpv1, libc6
Maintainer: Peter Adrianov <adrianov@users.noreply.github.com>
Description: GTK/libadwaita media player backed by mpv
 Rhino Player plays local video and audio with mpv, a GTK 4 shell, and
 optional smooth-motion (VapourSynth) support when installed separately.
Homepage: https://github.com/adrianov/rhino-player
EOF

cat >"$DEBIAN/postinst" <<'EOS'
#!/bin/sh
set -e
if [ "$1" = "configure" ]; then
  if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -f -t /usr/share/icons/hicolor 2>/dev/null || true
  fi
  if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database -q /usr/share/applications 2>/dev/null || true
  fi
  if command -v mandb >/dev/null 2>&1; then
    mandb -pq /usr/share/man 2>/dev/null || true
  fi
fi
exit 0
EOS
chmod 0755 "$DEBIAN/postinst"

OUT_DIR="${OUTPUT:-$REPO_ROOT/releases}"
mkdir -p "$OUT_DIR"
DEB_FILE="$OUT_DIR/rhino-player_${FULL_VER}_${DEB_ARCH}.deb"
dpkg-deb --build --root-owner-group "$STAGE" "$DEB_FILE"

echo "Built $DEB_FILE"
echo "Install: cd \"$OUT_DIR\" && sudo apt install ./$(basename "$DEB_FILE")"

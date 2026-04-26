#!/usr/bin/env bash
# System-wide install: binary, desktop, icons, metainfo, bundled VapourSynth scripts
# (see src/paths.rs: ../share/rhino-player/vs/ next to the binary).
#
# Usage: sudo ./data/install-system-wide.sh [/path/to/rhino-player]
# Default: REPO_ROOT/target/release/rhino-player
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DATA="$REPO_ROOT/data"
DEFAULT_BIN="$REPO_ROOT/target/release/rhino-player"
BIN="${1:-"$DEFAULT_BIN"}"
PREFIX="${PREFIX:-/usr/local}"
DEST_BIN="$PREFIX/bin/rhino-player"
SHARE_RHINO="$PREFIX/share/rhino-player"
APP_SHARE="$PREFIX/share"

if ! [[ -f "$BIN" && -x "$BIN" ]]; then
  echo "Build release first: cargo build --release" >&2
  echo "Or: $0 /path/to/rhino-player" >&2
  exit 1
fi
if [[ "${EUID:-}" -ne 0 ]]; then
  echo "Run as root, e.g.: sudo $0" >&2
  exit 1
fi

install -d -m 0755 "$PREFIX/bin"
install -d -m 0755 "$SHARE_RHINO/vs"
install -m 0755 "$BIN" "$DEST_BIN"
install -m 0644 "$DATA/vs"/*.vpy "$SHARE_RHINO/vs/"

install -d -m 0755 "$APP_SHARE/applications" "$APP_SHARE/metainfo" "$APP_SHARE/icons"
cp -a "$DATA/icons/hicolor" "$APP_SHARE/icons/"

# Exec: full path so launch works even if /usr/local/bin is not first on a weird PATH
if [[ "$DEST_BIN" == *" "* || "$DEST_BIN" == *"'"* ]]; then
  RHI_EXELINE="\"$DEST_BIN\" %F"
else
  RHI_EXELINE="$DEST_BIN %F"
fi
export RHI_EXELINE
tmp="$(mktemp)"
awk '
  /^Exec=/ { print "Exec=" ENVIRON["RHI_EXELINE"]; next }
  { print }
' "$DATA/applications/ch.rhino.RhinoPlayer.desktop" >"$tmp"
install -m 0644 "$tmp" "$APP_SHARE/applications/ch.rhino.RhinoPlayer.desktop"
rm -f "$tmp"
unset RHI_EXELINE

if [[ -f "$DATA/metainfo/ch.rhino.RhinoPlayer.metainfo.xml" ]]; then
  install -m 0644 "$DATA/metainfo/ch.rhino.RhinoPlayer.metainfo.xml" \
    "$APP_SHARE/metainfo/ch.rhino.RhinoPlayer.metainfo.xml"
fi

if command -v gtk-update-icon-cache &>/dev/null; then
  gtk-update-icon-cache -f -t "$APP_SHARE/icons/hicolor" 2>/dev/null || true
fi
if command -v update-desktop-database &>/dev/null; then
  update-desktop-database "$APP_SHARE/applications" 2>/dev/null || true
fi

# Register as default for common video types (per user who invoked sudo)
VIDEO_MIMES=(
  video/mp4 video/mpeg video/mp2t video/x-matroska video/webm video/quicktime
  video/x-msvideo video/x-avi video/ogg
  video/3gpp video/3gpp2 video/x-flv video/x-m4v video/dv
)
if command -v xdg-mime &>/dev/null && [[ -n "${SUDO_USER:-}" ]]; then
  SUDO_HOME="$(getent passwd "$SUDO_USER" | cut -d: -f6)"
  if [[ -n "$SUDO_HOME" ]]; then
    for m in "${VIDEO_MIMES[@]}"; do
      sudo -u "$SUDO_USER" env HOME="$SUDO_HOME" xdg-mime default ch.rhino.RhinoPlayer.desktop "$m" 2>/dev/null || true
    done
    echo "Set ch.rhino.RhinoPlayer as default for common video/* types (user: $SUDO_USER)."
  fi
else
  echo "To set as default for video files, run (as your user):"
  echo "  xdg-mime default ch.rhino.RhinoPlayer.desktop video/mp4  # and other types as needed"
fi

echo "Installed $DEST_BIN and $SHARE_RHINO/vs/, desktop + icons + metainfo under $APP_SHARE"

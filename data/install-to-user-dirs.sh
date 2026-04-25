#!/usr/bin/env bash
# Install hicolor icons + desktop file under ~/.local/share so GNOME/KDE (taskbar, alt+tab) can
# resolve Icon= from the ch.rhino.RhinoPlayer id. Run once per machine, or after moving the binary.
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DATA="$REPO_ROOT/data"
LOCAL="${XDG_DATA_HOME:-$HOME/.local/share}"
DEFAULT_BIN="$REPO_ROOT/target/release/rhino-player"
if [[ -x "$REPO_ROOT/target/debug/rhino-player" ]]; then
  DEFAULT_BIN="$REPO_ROOT/target/debug/rhino-player"
fi
BIN="${1:-"$DEFAULT_BIN"}"
if ! [[ -f "$BIN" ]]; then
  echo "Usage: $0 [/absolute/path/to/rhino-player]" >&2
  echo "  Default tried: $DEFAULT_BIN (not found)" >&2
  exit 1
fi
# Exec line: quote path if it has spaces (Freedesktop entry spec).
if [[ "$BIN" == *" "* || "$BIN" == *"'"* ]]; then
  export RHI_EXELINE="\"$BIN\" %F"
else
  export RHI_EXELINE="$BIN %F"
fi
mkdir -p "$LOCAL/icons" "$LOCAL/applications"
cp -a "$DATA/icons/hicolor" "$LOCAL/icons/"
tmp="$(mktemp)"
awk '
  /^Exec=/ { print "Exec=" ENVIRON["RHI_EXELINE"]; next }
  { print }
' "$DATA/applications/ch.rhino.RhinoPlayer.desktop" >"$tmp"
install -m 0644 "$tmp" "$LOCAL/applications/ch.rhino.RhinoPlayer.desktop"
rm -f "$tmp"
unset RHI_EXELINE
if command -v gtk-update-icon-cache &>/dev/null; then
  gtk-update-icon-cache -f -t "$LOCAL/icons/hicolor" 2>/dev/null || true
fi
if command -v update-desktop-database &>/dev/null; then
  update-desktop-database "$LOCAL/applications" 2>/dev/null || true
fi
echo "Installed desktop + icons under $LOCAL (Exec=$BIN)"
echo "If the taskbar still shows a generic icon, log out and back in, or restart GNOME Shell once."

#!/usr/bin/env bash
# Regenerate hicolor PNGs from a full-size design export. See data/icons/README.md.
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# Default: prepared design export in repo (any square-ish PNG with intended transparency). You can also pass
# ch.rhino.RhinoPlayer-master-1024.png to re-bake hicolor from an already transparent master.
DEFAULT_SRC="${REPO_ROOT}/data/icons/source/ch.rhino.RhinoPlayer-source.png"
SRC="${1:-"$DEFAULT_SRC"}"
# Optional inner inset for the 1024px master only (0.00–0.12): art is scaled to (1-2*inset)*1024
# then centered on 1024 (GNOME / dock breathing room if you want a hair of margin). Default 0 = full-bleed.
INSET="${2:-0}"

if ! [[ -f "$SRC" ]]; then
  echo "Usage: $0 [/path/to/source.png] [inset 0-0.12]" >&2
  echo "  Default source: $DEFAULT_SRC" >&2
  exit 1
fi
if ! command -v convert >/dev/null 2>&1; then
  echo "This script needs ImageMagick (convert). Install imagemagick (or -6.q16) etc." >&2
  exit 1
fi

TDIR="$(mktemp -d -t rhino-icon-XXXXXX)"
trap 'rm -rf "$TDIR"' EXIT

# 1) Preserve the prepared source canvas; only square-pad non-square inputs without stretching.
W=$(identify -format %w "$SRC" 2>/dev/null) || { echo "identify failed for: $SRC" >&2; exit 1; }
H=$(identify -format %h "$SRC" 2>/dev/null) || { echo "identify failed for: $SRC" >&2; exit 1; }
S=$W
if (( H > S )); then S=$H; fi
if (( S < 1 )); then
  echo "Error: source image has invalid size. Check $SRC" >&2
  exit 1
fi
convert "$SRC" -alpha on -gravity center -background none -extent "${S}x${S}" "$TDIR/square.png"

# 2) Scale square art to the full 1024 master.
# If INSET>0, scale to (1-2*inset)*1024 and center (transparent frame).
if awk -v i="$INSET" 'BEGIN { exit !(i > 0 && i < 0.2) }'; then
  INNER="$(awk -v i="$INSET" 'BEGIN { t=int(1024*(1-2*i)+0.5); if (t<32) t=32; print t }')"
  convert "$TDIR/square.png" -resize "${INNER}x${INNER}" -background none -gravity center -extent 1024x1024 -strip \
    "$TDIR/norm-1024.png"
else
  convert "$TDIR/square.png" -resize 1024x1024 -background none -gravity center -extent 1024x1024 -strip \
    "$TDIR/norm-1024.png"
fi

NORM="$TDIR/norm-1024.png"
OUT="${REPO_ROOT}/data/icons/hicolor"
SOURCE_OUT="${REPO_ROOT}/data/icons/source/ch.rhino.RhinoPlayer-master-1024.png"
convert "$NORM" -define png:compression-level=9 -define png:exclude-chunks=date,time -strip "$SOURCE_OUT"
cp "$SOURCE_OUT" "$OUT/1024x1024/apps/ch.rhino.RhinoPlayer.png"
for s in 16 22 24 32 48 64 128 256 512; do
  mkdir -p "$OUT/${s}x${s}/apps"
  convert "$NORM" -define png:compression-level=9 -define png:exclude-chunks=date,time -resize "${s}x${s}" -strip \
    "$OUT/${s}x${s}/apps/ch.rhino.RhinoPlayer.png"
done
if command -v optipng >/dev/null 2>&1; then
  # Optional extra lossless squeeze (install: optipng). Do not mutate arbitrary external source paths.
  for s in 16 22 24 32 48 64 128 256 512 1024; do
    optipng -o2 -strip all -quiet "$OUT/${s}x${s}/apps/ch.rhino.RhinoPlayer.png" 2>/dev/null || true
  done
  optipng -o2 -strip all -quiet "$SOURCE_OUT" 2>/dev/null || true
  if [[ "$(realpath "$SRC")" == "$(realpath "$DEFAULT_SRC")" ]]; then
    optipng -o2 -strip all -quiet "$DEFAULT_SRC" 2>/dev/null || true
  fi
fi
identify -format "OK: master %wx%h, inset ${INSET}\n" "$SOURCE_OUT"
echo "   Wrote: $SOURCE_OUT and hicolor/…/ch.rhino.RhinoPlayer.png (all standard sizes)"

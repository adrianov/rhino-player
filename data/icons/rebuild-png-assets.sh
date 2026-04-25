#!/usr/bin/env bash
# Regenerate hicolor PNGs from a full-size source (1024) using ImageMagick. See data/icons/README.md.
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SRC="${1:-"$REPO_ROOT/data/icons/source/ch.rhino.RhinoPlayer-master-1024.png"}"
# High fuzz: background is near-uniform off-white; tune down if the squircle’s edge is damaged.
FUZZ_PCT="${2:-8}"
NORM="/tmp/rhino-icon-normalized-$$.png"
if ! [[ -f "$SRC" ]]; then
  echo "Usage: $0 /path/to/source.png [fuzz%]" >&2
  exit 1
fi
# Near-white / white → transparent, trim, square canvas (by max side, no stretch), 1024 with safe margin.
convert "$SRC" -alpha set -channel RGBA -fuzz "${FUZZ_PCT}%" -transparent white \
  -trim +repage /tmp/rhino-trim-$$.png
W=$(identify -format %w /tmp/rhino-trim-$$.png)
H=$(identify -format %h /tmp/rhino-trim-$$.png)
S=$W
if (( H > S )); then S=$H; fi
convert /tmp/rhino-trim-$$.png -gravity center -background none -extent "${S}x${S}" /tmp/rhino-sq-$$.png
convert /tmp/rhino-sq-$$.png -resize '800x800>' -background none -gravity center -extent 1024x1024 \
  "$NORM"
rm -f /tmp/rhino-trim-$$.png /tmp/rhino-sq-$$.png
OUT="$REPO_ROOT/data/icons/hicolor"
SOURCE_OUT="$REPO_ROOT/data/icons/source/ch.rhino.RhinoPlayer-master-1024.png"
cp "$NORM" "$SOURCE_OUT"
cp "$NORM" "$OUT/1024x1024/apps/ch.rhino.RhinoPlayer.png"
for s in 16 22 24 32 48 64 128 256 512; do
  mkdir -p "$OUT/${s}x${s}/apps"
  convert "$NORM" -resize "${s}x${s}" "$OUT/${s}x${s}/apps/ch.rhino.RhinoPlayer.png"
done
rm -f "$NORM"
identify -format "OK: master %wx%h\n" "$SOURCE_OUT"
echo "   fuzz=${FUZZ_PCT}% under $OUT (and $SOURCE_OUT)"

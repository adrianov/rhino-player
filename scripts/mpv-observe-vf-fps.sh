#!/usr/bin/env bash
# Watch mpv’s reported framerate- and drop-related properties while a file plays.
# Uses the same /usr/local libmpv as Rhino when:  export LD_LIBRARY_PATH="…/usr/local/lib/x86_64-linux-gnu/…"
#
# **Note:** In mpv 0.38, `estimated-vf-fps` is derived from a *smoothed* per-frame
# duration that often **snaps to container (demux) fps** when PTS spacing still looks
# like 24p (see `calculate_frame_duration` in `player/video.c`). A line near ~**24** here
# does *not* prove the VapourSynth chain is not feeding ~60 frames per second of video.
# For what you *see* on screen, use mpv’s stats overlay: **Shift+i** (twice for details),
# and watch “Dropped” / frame time; in top/htop, sum **CPU%** of mpv and child threads.
set -euo pipefail
: "${MPV:=/usr/local/bin/mpv}"
: "${DURATION:=15}"
: "${VS:=$1}"
: "${FILE:=$2}"
if [[ -z "${VS}" || -z "${FILE}" || ! -f "$FILE" ]]; then
  echo "Usage:  LD_LIBRARY_PATH=…/usr/local/lib/…  $0 /path/to/script.vpy /path/to/video.mkv" >&2
  echo "  Optional:  MPV=…/mpv  DURATION=20  (seconds to poll)" >&2
  exit 1
fi
SOCK="/tmp/mpv-observe-$$.sock"
rm -f "$SOCK"
"$MPV" "$FILE" --no-terminal --really-quiet --length="$((DURATION + 2))" \
  --vf-add="vapoursynth:file=$VS:buffered-frames=8:concurrent-frames=auto" \
  --input-ipc-server="$SOCK" &
M=$!
for _ in $(seq 1 100); do [[ -S "$SOCK" ]] && break; sleep 0.05; done
sleep 1.5
for t in $(seq 1 "$DURATION"); do
  out="$(python3 -c "
import json, socket, sys
path = sys.argv[1]
props = [
    'container-fps', 'estimated-vf-fps', 'decoder-frame-drop-count', 'frame-drop-count',
    'display-fps', 'estimated-display-fps',
]
s = socket.socket(socket.AF_UNIX)
s.connect(path)
rf = s.makefile('rb')
out = []
for p in props:
    s.sendall((json.dumps({'command':['get_property', p]}) + '\\n').encode())
    line = rf.readline()
    o = json.loads(line.decode()) if line else {}
    out.append(f\"{p}={o.get('data')!r}\")
rf.close()
s.close()
print('  '.join(out))
" "$SOCK" 2>&1)"
  echo "t=${t}s  $out"
  sleep 1
done
kill "$M" 2>/dev/null || true
wait "$M" 2>/dev/null || true
rm -f "$SOCK"

#!/usr/bin/env bash
# Build and install **libmpv** + **mpv** to `/usr/local` with **-Dvapoursynth=enabled**, using
# distro **-dev** packages from `apt-get build-dep mpv`.
#
# **Ubuntu 24.04+** uses `/etc/apt/sources.list.d/ubuntu.sources` (DEB822). `deb-src` is enabled
# by `Types: deb deb-src` — uncommenting `sources.list` alone is not enough. This script can do
# that for you (set `MPV_APT_ENABLE_DEB_SRC=0` to disable).
#
# From your **real** rhino-player directory (not the literal `cd /path/to/...`):
#   export LD_LIBRARY_PATH="/usr/local/lib/x86_64-linux-gnu${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
#   cd /home/…/rhino-player && cargo run
#
# **mpv 0.39+** needs **VapourSynth R56+** in pkg-config. **ppa:savoury1/mpv** often ships **R55**
# (API3). This script defaults to **v0.38.0**, which only requires `vapoursynth >= 26` and matches R55.
# For **v0.39.0+**, upgrade VapourSynth to R56+ first, then: `MPV_VERSION=v0.39.0 ./scripts/…`
#
# **Meson / install:** do **not** use `sudo meson install` if your **user** and **root** Python do not
# load the same `meson` (e.g. `pip install --user meson` 1.11 for you, `python3-meson` 1.7 for root).
# Then `build.dat` from `meson setup` cannot be read by `sudo meson install`. The script uses
# **`meson install` as you** with **`DESTDIR`**, then **`sudo cp -a` into / (same pattern as distros).
set -euo pipefail
MPV_VERSION="${MPV_VERSION:-v0.38.0}"
PREFIX="${PREFIX:-/usr/local}"
WORKDIR="${WORKDIR:-/tmp/mpv-vapoursynth-build}"
: "${MPV_APT_ENABLE_DEB_SRC:=1}"
: "${MPV_MESON:=/usr/bin/meson}"

if ! command -v sudo >/dev/null 2>&1; then
  echo "sudo is required." >&2
  exit 1
fi

enable_deb_src() {
  local did=0
  if [[ "${MPV_APT_ENABLE_DEB_SRC}" != "1" ]]; then
    return 0
  fi
  for f in /etc/apt/sources.list.d/ubuntu.sources /etc/apt/sources.list.d/ubuntu-ports.sources; do
    if [[ -f "$f" ]] && grep -qE '^Types:[[:space:]]+deb[[:space:]]*$' "$f" 2>/dev/null; then
      echo "Enabling deb-src: changing Types: deb -> Types: deb deb-src in $f (backup in /var/tmp)…" >&2
      sudo cp -a "$f" "/var/tmp/$(basename "$f").rhino-apt-bak.$(date +%Y%m%d%H%M%S)"
      sudo sed -i 's/^Types:[[:space:]]*deb[[:space:]]*$/Types: deb deb-src/' "$f" || true
      did=1
    fi
  done
  if [[ -f /etc/apt/sources.list ]]; then
    if sudo sed -Ei 's/^#(deb-src)/\1/' /etc/apt/sources.list 2>/dev/null; then
      did=1
    fi
  fi
  if [[ "$did" -eq 1 ]]; then
    sudo apt-get update
  fi
}
enable_deb_src

echo "== 1) Toolchain + VapourSynth headers + apt build-dep mpv =="
sudo apt-get update
sudo apt-get install -y --no-install-recommends \
  build-essential git meson ninja-build pkg-config \
  libvapoursynth-dev yasm

if ! sudo apt-get build-dep -y mpv; then
  echo "" >&2
  echo "apt-get build-dep mpv failed." >&2
  echo "On Ubuntu 24.04+ run:  grep -E '^Types' /etc/apt/sources.list.d/ubuntu.sources" >&2
  echo "It must include **deb-src** (e.g. \"Types: deb deb-src\"). Re-run this script; or set" >&2
  echo "  MPV_APT_ENABLE_DEB_SRC=0  and install mpv build-deps by hand from “apt-get source mpv; grep Build-Depends”." >&2
  exit 1
fi

echo "== 2) mpv source ${MPV_VERSION} in ${WORKDIR} =="
rm -rf "$WORKDIR"
mkdir -p "$WORKDIR"
git clone --depth 1 --branch "$MPV_VERSION" https://github.com/mpv-player/mpv.git "$WORKDIR/mpv"

cd "$WORKDIR/mpv"
echo "== 3) Meson: libmpv + vapoursynth =="
if [[ ! -x "$MPV_MESON" ]]; then
  if MPV_MESON="$(command -v meson 2>/dev/null)" && [[ -n "$MPV_MESON" && -x "$MPV_MESON" ]]; then
    :
  else
    echo "meson not found; install meson (apt) or set MPV_MESON to the meson binary." >&2
    exit 1
  fi
fi
echo "Using meson: $MPV_MESON ($("$MPV_MESON" --version 2>&1 | head -1))" >&2
"$MPV_MESON" setup build \
  --prefix="$PREFIX" \
  -Dlibmpv=true \
  -Dbuildtype=release \
  -Dvapoursynth=enabled \
  -Dmanpage-build=disabled
# `ninja` only: avoids `meson compile` re-reading `build.dat` with a different Meson at the end.
ninja -C build
# Never `sudo meson install` here: **root** `python3` ignores `~/.local`, so a different `meson`
# (often **older** from apt) can’t read this `build.dat` (from **user** Meson, often pip).
STAGE="${WORKDIR}/mpv-install-stage"
rm -rf "$STAGE"
mkdir -p "$STAGE"
DESTDIR="$STAGE" "$MPV_MESON" install -C build
sudo cp -a "${STAGE}/." /
rm -rf "$STAGE"

echo ""
echo "== Installed. Use this libmpv with Rhino: =="
echo "export LD_LIBRARY_PATH=\"$PREFIX/lib/x86_64-linux-gnu\${LD_LIBRARY_PATH:+:\$LD_LIBRARY_PATH}\""
echo "cd your-rhino-player-clone && cargo run"
echo "Verify:  $PREFIX/bin/mpv -vf help 2>&1 | grep -E '^\s+vapoursynth\b'"

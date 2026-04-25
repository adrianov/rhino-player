#!/usr/bin/env bash
# Prepare **VapourSynth** + plugins for Rhino’s **vf=vapoursynth** path.
# The usual blocker is not Python: **libmpv** must expose the native `vapoursynth` vf. Check:
#   mpv -vf help 2>&1 | grep -E '^\s+vapoursynth\b'
set -euo pipefail

if ! command -v apt-get >/dev/null 2>&1; then
  echo "This script needs apt (Debian / Ubuntu)." >&2
  exit 1
fi

lists_vapour_vf() {
  command -v mpv >/dev/null 2>&1 && mpv -vf help 2>&1 | grep -qE '^[[:space:]]*vapoursynth[[:space:]]'
}

has_core_mv() {
  python3 -c "import vapoursynth as vs; _ = vs.core.mv" 2>/dev/null
}

echo "== 1) libmpv + mpv: must list native 'vapoursynth' vf =="
if lists_vapour_vf; then
  echo "OK: current mpv already has vf=vapoursynth."
else
  echo "This system's **libmpv** is missing the vapoursynth filter (common for Ubuntu/Debian stock mpv)."
  echo "Prebuilt Savoury PPAs often need **extra** dependencies (e.g. libplacebo360, FFmpeg 7) that may be incomplete without private repos."
  echo ""
  echo "Reliable path on Ubuntu 24.04+:"
  echo "  ./scripts/build-mpv-vapoursynth-system.sh"
  echo "  (enable deb-src first — see that script’s header — then re-run; uses meson to /usr/local and LD_LIBRARY_PATH for Rhino.)"
  echo ""
  if command -v sudo >/dev/null 2>&1 && [[ -t 0 ]]; then
    read -r -p "Try adding ppa:savoury1/mpv and installing mpv+libmpv2 anyway? [y/N] " a || a=n
    if [[ "${a:-n}" =~ ^[Yy]$ ]]; then
      sudo add-apt-repository -y ppa:savoury1/mpv
      sudo apt-get update
      if sudo apt-get install -y libmpv2 mpv vapoursynth vapoursynth-python3; then
        lists_vapour_vf && echo "OK: PPA install worked." || echo "PPA install completed but 'mpv -vf help' still has no vapoursynth — use build-mpv script." >&2
      else
        echo "apt could not install the stack. Use ./scripts/build-mpv-vapoursynth-system.sh" >&2
      fi
    fi
  else
    echo "Non-interactive: skipping optional PPA. Run ./scripts/build-mpv-vapoursynth-system.sh" >&2
  fi
  if ! lists_vapour_vf; then
    exit 1
  fi
fi

echo ""
echo "== 2) MVTools plugin (for bundled .vpy) =="
if has_core_mv; then
  echo "OK: core.mv is available."
else
  echo "Trying apt package vapoursynth-mvtools…"
  if sudo apt-get install -y vapoursynth-mvtools; then
    has_core_mv && echo "OK: mvtools installed." || echo "MVTools still missing — use vsrepo or build https://github.com/dubhater/vapoursynth-mvtools" >&2
  else
    echo "Install the mvtools .so (vsrepo or build from source); see data/vs/README.md" >&2
  fi
fi

echo "Done."

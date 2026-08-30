#!/usr/bin/env bash
# Copy MVTools + its non-system dylib dep into a Rhino lib tree and rewrite install names
# so the plugin no longer depends on Homebrew Cellar / opt paths.
#
# Usage: macos-vendor-smooth-libs.sh <dest-lib-root>
# Creates:
#   <dest>/plugins/mvtools.dylib
#   <dest>/plugins/libfftw3f.3.dylib
# with mvtools → @loader_path/libfftw3f.3.dylib
#
# Exit 0 on success; exit 1 if source plugins cannot be found (unless SKIP_MISSING=1).

set -euo pipefail

DEST_ROOT="${1:-}"
if [[ -z "$DEST_ROOT" ]]; then
  echo "usage: $0 <dest-lib-root>" >&2
  exit 2
fi

find_mvtools_src() {
  local prefix py plugins hit
  for prefix in /opt/homebrew /usr/local; do
    if [[ -f "$prefix/lib/libmvtools.dylib" ]]; then
      echo "$prefix/lib/libmvtools.dylib"
      return 0
    fi
    for base in \
      "$prefix/opt/vapoursynth-mvtools/lib" \
      "$prefix/Cellar/vapoursynth-mvtools"/*/lib; do
      [[ -d "$base" ]] || continue
      for py in "$base"/python*/site-packages/vapoursynth/plugins; do
        [[ -d "$py" ]] || continue
        for hit in "$py/mvtools.dylib" "$py/libmvtools.dylib"; do
          if [[ -f "$hit" ]]; then
            echo "$hit"
            return 0
          fi
        done
      done
    done
  done
  return 1
}

find_fftw_src() {
  local prefix
  for prefix in /opt/homebrew /usr/local; do
    if [[ -f "$prefix/opt/fftw/lib/libfftw3f.3.dylib" ]]; then
      echo "$prefix/opt/fftw/lib/libfftw3f.3.dylib"
      return 0
    fi
    if [[ -f "$prefix/lib/libfftw3f.3.dylib" ]]; then
      echo "$prefix/lib/libfftw3f.3.dylib"
      return 0
    fi
  done
  return 1
}

SRC_MV="$(find_mvtools_src || true)"
if [[ -z "$SRC_MV" ]]; then
  if [[ "${SKIP_MISSING:-0}" == "1" ]]; then
    echo "macos-vendor-smooth-libs: no Homebrew MVTools — skipped" >&2
    exit 0
  fi
  echo "macos-vendor-smooth-libs: mvtools.dylib not found (brew install vapoursynth-mvtools)" >&2
  exit 1
fi

SRC_FFTW="$(find_fftw_src || true)"
if [[ -z "$SRC_FFTW" ]]; then
  if [[ "${SKIP_MISSING:-0}" == "1" ]]; then
    echo "macos-vendor-smooth-libs: no libfftw3f — skipped" >&2
    exit 0
  fi
  echo "macos-vendor-smooth-libs: libfftw3f.3.dylib not found (brew install fftw)" >&2
  exit 1
fi

PLUGINS="$DEST_ROOT/plugins"
mkdir -p "$PLUGINS"
# Writable copies (Homebrew keg files are often mode 0444).
cp -f "$SRC_MV" "$PLUGINS/mvtools.dylib"
chmod u+w "$PLUGINS/mvtools.dylib"
cp -f "$SRC_FFTW" "$PLUGINS/libfftw3f.3.dylib"
chmod u+w "$PLUGINS/libfftw3f.3.dylib"

# Drop Homebrew's existing signature so install_name_tool has nothing to invalidate
# (we ad-hoc re-sign below regardless); avoids noisy "will invalidate the code signature".
codesign --remove-signature "$PLUGINS/libfftw3f.3.dylib" 2>/dev/null || true
codesign --remove-signature "$PLUGINS/mvtools.dylib" 2>/dev/null || true

# Drop absolute Homebrew deps on fftw; keep only @loader_path sibling.
install_name_tool -id "@loader_path/libfftw3f.3.dylib" "$PLUGINS/libfftw3f.3.dylib"
install_name_tool -id "@loader_path/mvtools.dylib" "$PLUGINS/mvtools.dylib"
# Replace every absolute libfftw3f*.dylib reference with the bundled sibling.
while IFS= read -r dep; do
  case "$dep" in
    *libfftw3f*.dylib)
      install_name_tool -change "$dep" "@loader_path/libfftw3f.3.dylib" "$PLUGINS/mvtools.dylib"
      ;;
  esac
done < <(otool -L "$PLUGINS/mvtools.dylib" | awk 'NR>1 {print $1}')

# install_name_tool invalidates signatures; ad-hoc sign so dyld will load the copies.
codesign --force -s - "$PLUGINS/libfftw3f.3.dylib" >/dev/null
codesign --force -s - "$PLUGINS/mvtools.dylib" >/dev/null

echo "macos-vendor-smooth-libs: wrote $PLUGINS/mvtools.dylib (from $SRC_MV)"

#!/usr/bin/env bash
# Populate releases/ with artifacts to attach to a GitHub Release (one OS per invocation).
#
# Linux: builds rhino-player_<semver>-<rev>_<arch>.deb into releases/
# macOS: builds Rhino Player.app, then Rhino-Player-<semver>-macos-<arch>.dmg into releases/
#
# Windows is not wired yet; see releases/README.md for a suggested future filename.
set -euo pipefail
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

case "$(uname -s)" in
  Linux)
    exec "$REPO/scripts/build-deb.sh"
    ;;
  Darwin)
    VERSION="$(grep -m1 '^version =' "$REPO/Cargo.toml" | sed -E 's/^version = "([^"]+)".*/\1/')"
    REL="$REPO/releases"
    mkdir -p "$REL"
    MAC="$(uname -m)"
    case "$MAC" in
      arm64) DMG_ARCH=arm64 ;;
      x86_64) DMG_ARCH=x86_64 ;;
      *) DMG_ARCH="$MAC" ;;
    esac
    STAGING="$(mktemp -d)"
    cleanup() { rm -rf "$STAGING"; }
    trap cleanup EXIT
    DEST_ROOT="$STAGING" "$REPO/scripts/macos-build-app-bundle.sh"
    DMG="$REL/Rhino-Player-${VERSION}-macos-${DMG_ARCH}.dmg"
    rm -f "$DMG"
    # UDZO: zlib-compressed read-only disk image with the .app at the volume root.
    hdiutil create -volname "Rhino Player" -srcfolder "$STAGING" -ov -format UDZO "$DMG"
    trap - EXIT
    cleanup
    echo "Built $DMG — attach this file on GitHub Releases."
    ;;
  *)
    echo "No automated GitHub release staging for $(uname -s) yet." >&2
    echo "Windows is planned; see $REPO/releases/README.md" >&2
    exit 1
    ;;
esac

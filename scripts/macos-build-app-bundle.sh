#!/usr/bin/env bash
# Release: Rhino Player.app with Info.plist document types + AppIcon.icns + bundled vs/ + icons.
# Requires Homebrew GTK 4 stack (same as README). Outputs dist/macos/Rhino Player.app by default.
#
# Usage: ./scripts/macos-build-app-bundle.sh

set -euo pipefail
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "This script targets macOS only." >&2
  exit 1
fi

# shellcheck source=macos-dev-env.sh
source "$REPO/scripts/macos-dev-env.sh"

VERSION="$(grep -m1 '^version =' "$REPO/Cargo.toml" | sed -E 's/.*"([^"]+)".*/\1/')"
BIN_NAME="rhino-player"
readonly APP_TITLE="Rhino Player"
APP_BUNDLE="${APP_TITLE}.app"
DEST_ROOT="${DEST_ROOT:-$REPO/dist/macos}"

APP_PATH="$DEST_ROOT/$APP_BUNDLE"
CONTENTS="$APP_PATH/Contents"

cargo build --release --config 'build.rustc-wrapper=""'

rm -rf "$APP_PATH"
mkdir -p "$CONTENTS/MacOS" "$CONTENTS/Resources/share/rhino-player/vs"

install -m 0755 "$REPO/target/release/$BIN_NAME" "$CONTENTS/MacOS/$BIN_NAME"

sed "s/@VERSION@/${VERSION}/g" "$REPO/packaging/macos/Info.plist.in" >"$CONTENTS/Info.plist"

cp -a "$REPO/data/vs/"*.vpy "$CONTENTS/Resources/share/rhino-player/vs/"

DEST_ICONS="$CONTENTS/Resources/data/icons"
mkdir -p "$DEST_ICONS"
cp -a "$REPO/data/icons/hicolor" "$DEST_ICONS/"

WORKDIR="$(mktemp -d)"
trap 'rm -rf "$WORKDIR"' EXIT
ICONSET="$WORKDIR/AppIcon.iconset"
mkdir "$ICONSET"

HICON="$REPO/data/icons/hicolor"
cp "$HICON/16x16/apps/ch.rhino.RhinoPlayer.png" "$ICONSET/icon_16x16.png"
cp "$HICON/32x32/apps/ch.rhino.RhinoPlayer.png" "$ICONSET/icon_16x16@2x.png"
cp "$HICON/32x32/apps/ch.rhino.RhinoPlayer.png" "$ICONSET/icon_32x32.png"
cp "$HICON/64x64/apps/ch.rhino.RhinoPlayer.png" "$ICONSET/icon_32x32@2x.png"
cp "$HICON/128x128/apps/ch.rhino.RhinoPlayer.png" "$ICONSET/icon_128x128.png"
cp "$HICON/256x256/apps/ch.rhino.RhinoPlayer.png" "$ICONSET/icon_128x128@2x.png"
cp "$HICON/256x256/apps/ch.rhino.RhinoPlayer.png" "$ICONSET/icon_256x256.png"
cp "$HICON/512x512/apps/ch.rhino.RhinoPlayer.png" "$ICONSET/icon_256x256@2x.png"
cp "$HICON/512x512/apps/ch.rhino.RhinoPlayer.png" "$ICONSET/icon_512x512.png"
cp "$HICON/1024x1024/apps/ch.rhino.RhinoPlayer.png" "$ICONSET/icon_512x512@2x.png"

iconutil -c icns "$ICONSET" -o "$CONTENTS/Resources/AppIcon.icns"

echo "Built $APP_PATH (version ${VERSION}). Drag to /Applications or run: open \"$APP_PATH\""
open -R "$APP_PATH"

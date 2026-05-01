#!/usr/bin/env bash
# Source before `cargo build` on macOS so pkg-config finds GTK 4 / libadwaita from Homebrew.
# Usage: source ./scripts/macos-dev-env.sh

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "macos-dev-env.sh is for macOS only." >&2
  return 1 2>/dev/null || exit 1
fi

if ! command -v brew >/dev/null 2>&1; then
  echo "Homebrew not found. Install from https://brew.sh/" >&2
  return 1 2>/dev/null || exit 1
fi

pfx="$(brew --prefix)"
export PATH="${pfx}/bin:${PATH}"
export PKG_CONFIG_PATH="${pfx}/lib/pkgconfig:${pfx}/share/pkgconfig"

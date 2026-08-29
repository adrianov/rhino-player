#!/usr/bin/env bash
# Idempotent Cloud Agent bootstrap for Rhino Player (Linux / GTK4 / libadwaita / mpv).
# System headers come from apt; the Rust toolchain is bumped to a release new enough
# for the edition-2024 crates in Cargo.lock; then the workspace is built.
set -euo pipefail

# GTK 4, libadwaita, libmpv dev headers + clang/lld linker (see .cargo/config.toml).
sudo apt-get update -qq
sudo apt-get install -y --no-install-recommends \
  libgtk-4-dev \
  libadwaita-1-dev \
  libmpv-dev \
  build-essential \
  clang \
  lld \
  pkg-config

# Some Cargo.lock dependencies require the edition-2024 feature (Cargo >= 1.85).
# Ubuntu's preinstalled toolchain is older, so ensure a recent stable is the default.
need_bump=1
if command -v cargo >/dev/null 2>&1; then
  ver="$(cargo --version | awk '{print $2}')"
  major="${ver%%.*}"; rest="${ver#*.}"; minor="${rest%%.*}"
  if [ "${major:-0}" -gt 1 ] || { [ "${major:-0}" -eq 1 ] && [ "${minor:-0}" -ge 85 ]; }; then
    need_bump=0
  fi
fi
if [ "$need_bump" -eq 1 ]; then
  rustup toolchain install stable
  rustup default stable
fi

# Build the debug binary (fast, incremental). Use `cargo build --release` for distribution.
cargo build

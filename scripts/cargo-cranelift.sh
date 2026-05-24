#!/usr/bin/env bash
# Nightly Cranelift dev builds (faster codegen than LLVM for incremental work).
# Usage: scripts/cargo-cranelift.sh build | run [-- args…]
# Alias: cargo cf build | cargo cf run

set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

if [[ "$(uname -s)" == "Darwin" && -f "$root/scripts/macos-dev-env.sh" ]]; then
  # shellcheck source=/dev/null
  source "$root/scripts/macos-dev-env.sh"
fi

toolchain="${RHINO_CRANELIFT_TOOLCHAIN:-nightly}"
rustup_home="${RUSTUP_HOME:-$HOME/.rustup}"

if ! command -v rustup >/dev/null 2>&1; then
  echo "rustup required for Cranelift (Homebrew stable cargo cannot use -Zcodegen-backend)." >&2
  echo "Install: https://rustup.rs/" >&2
  exit 1
fi

if ! rustup run "$toolchain" rustc --version >/dev/null 2>&1; then
  echo "Missing toolchain '$toolchain'. Install:" >&2
  echo "  rustup toolchain install $toolchain -c rustc-codegen-cranelift-preview" >&2
  exit 1
fi

host="$(rustup run "$toolchain" rustc --print host-tuple)"
tc_dir="$rustup_home/toolchains/${toolchain}-${host}"
cargo="$tc_dir/bin/cargo"
rustc="$tc_dir/bin/rustc"

if [[ ! -x "$cargo" || ! -x "$rustc" ]]; then
  echo "Nightly toolchain not found at $tc_dir" >&2
  exit 1
fi

export RUSTC="$rustc"
exec env RUSTC_WRAPPER= "$cargo" -Zcodegen-backend \
  --config "$root/.cargo/cranelift.toml" \
  --config 'build.rustc-wrapper=""' \
  "$@"

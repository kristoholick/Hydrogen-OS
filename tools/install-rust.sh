#!/usr/bin/env bash
# Installs Rust for the current Linux user only (~/.rustup, ~/.cargo). No sudo, nothing system-wide.
# Source: the official installer at https://sh.rustup.rs
set -euo pipefail

if ! command -v rustup >/dev/null 2>&1 && [ ! -x "$HOME/.cargo/bin/rustup" ]; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain nightly
fi

# shellcheck disable=SC1091
. "$HOME/.cargo/env"
rustup component add rust-src llvm-tools --toolchain nightly
rustup target add x86_64-unknown-none --toolchain nightly

echo "--- installed ---"
rustc --version
cargo --version
rustup target list --installed
rustup component list --installed | grep -E 'rust-src|llvm-tools'

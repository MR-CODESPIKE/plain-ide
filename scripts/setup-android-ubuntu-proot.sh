#!/usr/bin/env bash
# Run this script inside an Ubuntu 24.04 proot-distro shell on Android.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PLAIN_ROOT="$ROOT/native/plain-ecosystem"

if [ "$(id -u)" -ne 0 ]; then
  echo 'Run this inside the Ubuntu proot as root (proot-distro login ubuntu).' >&2
  exit 1
fi

apt-get update
apt-get install -y build-essential curl ca-certificates nodejs llvm-18-dev clang-18 libclang-18-dev
if ! command -v rustup >/dev/null 2>&1; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
fi
export PATH="$HOME/.cargo/bin:$HOME/.local/bin:$PATH"
if ! command -v cargo >/dev/null 2>&1; then
  echo 'Rust/Cargo installation failed.' >&2
  exit 1
fi
LLVM_CONFIG="$(command -v llvm-config-18)"
LLVM_VERSION="$("$LLVM_CONFIG" --version)"
case "$LLVM_VERSION" in 18.*) ;; *) echo "Expected LLVM 18.x, found $LLVM_VERSION" >&2; exit 1 ;; esac
mkdir -p "$HOME/.local/bin"
ln -sf "$(command -v clang-18)" "$HOME/.local/bin/clang"
export LLVM_SYS_180_PREFIX="$("$LLVM_CONFIG" --prefix)"
export LLVM_CONFIG_PATH="$LLVM_CONFIG"
export LLVM_LINK_LLVM_DYLIB=1
cd "$PLAIN_ROOT"
cargo build --release --workspace
export PLAIN_COMPILER_PATH="$PLAIN_ROOT/target/release/plain"
node "$ROOT/native/bridge/server.mjs"

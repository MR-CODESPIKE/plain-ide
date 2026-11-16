#!/data/data/com.termux/files/usr/bin/bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PLAIN_ROOT="$ROOT/native/plain-ecosystem"

for tool in rustc cargo clang llvm-config node; do
  command -v "$tool" >/dev/null 2>&1 || {
    echo "Missing $tool. In Termux run: pkg update && pkg install rust llvm clang nodejs" >&2
    exit 1
  }
done
LLVM_VERSION="$(llvm-config --version)"
case "$LLVM_VERSION" in
  18.*) ;;
  *) echo "Plain currently requires LLVM 18.x; Termux provides $LLVM_VERSION. Install a compatible LLVM 18 build before continuing." >&2; exit 1 ;;
esac

export LLVM_SYS_180_PREFIX="$(llvm-config --prefix)"
export LLVM_CONFIG_PATH="$(command -v llvm-config)"
export LLVM_LINK_LLVM_DYLIB=1
cd "$PLAIN_ROOT"
cargo build --release --workspace
export PLAIN_COMPILER_PATH="$PLAIN_ROOT/target/release/plain"
node "$ROOT/native/bridge/server.mjs"

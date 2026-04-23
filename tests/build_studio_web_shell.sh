#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT_DIR="${1:-$ROOT/target/studio-web-shell}"
WASM_PATH="$ROOT/target/wasm32-unknown-unknown/debug/studio_web.wasm"

if ! command -v wasm-bindgen >/dev/null 2>&1; then
  echo "missing wasm-bindgen CLI; install it with: cargo install wasm-bindgen-cli" >&2
  exit 1
fi

cargo build -p studio-web --target wasm32-unknown-unknown
rm -rf "$OUT_DIR"
mkdir -p "$OUT_DIR"

wasm-bindgen "$WASM_PATH" --target web --out-dir "$OUT_DIR"
cp "$ROOT/crates/studio-web/web/index.html" "$OUT_DIR/index.html"

echo "studio-web shell built at $OUT_DIR"

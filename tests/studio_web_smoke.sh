#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORKSPACE="$ROOT/tests/studio-web-smoke-workspace"
HOST_LOG="$ROOT/target/studio-web-smoke-host.log"
HOST_BIND="127.0.0.1:39180"
WATCH_SMOKE_FILE="$WORKSPACE/watch-smoke-generated.txt"

if ! command -v wasm-pack >/dev/null 2>&1; then
  echo "missing wasm-pack; install it with: cargo install wasm-pack" >&2
  exit 1
fi

if ! command -v bun >/dev/null 2>&1; then
  echo "missing bun; install it from https://bun.sh" >&2
  exit 1
fi

mkdir -p "$ROOT/target"
cargo run -p app-server-host --bin websocket-host -- --workspace "$WORKSPACE" --bind "$HOST_BIND" >"$HOST_LOG" 2>&1 &
HOST_PID=$!

cleanup() {
  kill "$HOST_PID" 2>/dev/null || true
  wait "$HOST_PID" 2>/dev/null || true
  rm -f "$WATCH_SMOKE_FILE"
}
trap cleanup EXIT

rm -f "$WATCH_SMOKE_FILE"

bun scripts/wait_for_port.ts 127.0.0.1 39180 30000 200

cd "$ROOT/crates/studio-web-wasm"
wasm-pack test --headless --chrome --features browser-smoke --test browser_smoke

wasm-pack test --headless --chrome --features browser-smoke --test browser_watch_smoke

#!/usr/bin/env bash
set -euo pipefail

OUT="${1:-bolt-demo.cast}"
DX_BIN_REAL="${DX_BIN:-}"
if [ -z "$DX_BIN_REAL" ]; then
  if command -v dx >/dev/null 2>&1; then DX_BIN_REAL="$(command -v dx)"; elif [ -x ./target/release/dx ]; then DX_BIN_REAL="./target/release/dx"; else echo "Build dx first: cargo build --release" >&2; exit 1; fi
fi

scripts/demo_bolt.sh &
APP_PID=$!
trap 'kill $APP_PID 2>/dev/null || true' EXIT

exec asciinema rec "$OUT" -c "bash -lc 'wait $APP_PID'"



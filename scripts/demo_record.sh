#!/usr/bin/env bash
# Demo recorder for dx using asciinema
# Usage:
#   scripts/demo_record.sh [output.cast]
# Env:
#   DX_BIN=/path/to/dx   # optional override; falls back to `dx` in PATH or ./target/release/dx
set -euo pipefail

# Colors
GREEN='\033[32m'; YELLOW='\033[33m'; DIM='\033[2m'; RESET='\033[0m'

msg()  { printf "%b%s%b\n" "$DIM" "$*" "$RESET"; }
ok()   { printf "%b%s%b\n" "$GREEN" "$*" "$RESET"; }
warn() { printf "%b%s%b\n" "$YELLOW" "$*" "$RESET"; }

die() { printf "Error: %s\n" "$*" >&2; exit 1; }

# Check asciinema
if ! command -v asciinema >/dev/null 2>&1; then
  die "asciinema not found. Install: macOS -> brew install asciinema, Linux -> pipx/pip install asciinema"
fi

# Resolve dx binary
DX_BIN_REAL="${DX_BIN:-}"
if [ -z "${DX_BIN_REAL}" ]; then
  if command -v dx >/dev/null 2>&1; then
    DX_BIN_REAL="$(command -v dx)"
  elif [ -x ./target/release/dx ]; then
    DX_BIN_REAL="./target/release/dx"
  else
    die "dx binary not found. Build with: cargo build --release, or set DX_BIN=/path/to/dx"
  fi
fi

OUT="${1:-demo-$(date +%Y%m%d-%H%M%S).cast}"

msg "Detected dx: $DX_BIN_REAL"
msg "Output file: $OUT"

warn "Recording will start immediately. Press q/Esc to quit dx; recording stops on exit."

# Run recording
exec asciinema rec "$OUT" -c "$DX_BIN_REAL"

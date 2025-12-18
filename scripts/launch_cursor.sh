#!/usr/bin/env bash
set -euo pipefail

# Launch Cursor editor for the given directory (default: current project)
DIR="${1:-.}"

if command -v open >/dev/null 2>&1; then
  exec open -a "Cursor" "$DIR"
elif command -v cursor >/dev/null 2>&1; then
  exec cursor "$DIR"
else
  echo "Cursor not found. Install Cursor or add it to PATH." >&2
  exit 1
fi



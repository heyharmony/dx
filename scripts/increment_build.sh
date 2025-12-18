#!/usr/bin/env bash
set -euo pipefail

# Increment build number stored in BUILD (integer). Create if missing.
ROOT_DIR="$(cd "$(dirname "$0")"/.. && pwd)"
BUILD_FILE="$ROOT_DIR/build-artifacts/BUILD"
VERSION_FILE="$ROOT_DIR/build-artifacts/VERSION.txt"
BASE_VERSION_FILE="$ROOT_DIR/build-artifacts/VERSION"
CARGO_FILE="$ROOT_DIR/Cargo.toml"

current=0
if [[ -f "$BUILD_FILE" ]]; then
  # Strip NUL and newline characters before extracting digits to avoid "nul byte found" errors
  current=$(tr -d '\000\n' < "$BUILD_FILE" | sed 's/[^0-9]//g')
  current=${current:-0}
fi
next=$((current + 1))
echo "$next" > "$BUILD_FILE"

# Compose VERSION.txt as: YYYY.X.X-bN
# Base comes from VERSION when present (e.g. 2024.1.0); otherwise default to current year .0.0
year=$(date +%Y)
base="${year}.0.0"
if [[ -f "$BASE_VERSION_FILE" ]]; then
  base=$(cat "$BASE_VERSION_FILE" | tr -d '\n' | sed 's/\s//g')
fi
echo "${base}-b${next}" > "$VERSION_FILE"

# Update version in Cargo.toml
new_version="${base}-b${next}"
if [[ -f "$CARGO_FILE" ]]; then
  # Use sed to replace the version line in Cargo.toml
  if command -v gsed >/dev/null 2>&1; then
    # Use GNU sed if available (macOS with brew install gnu-sed)
    gsed -i "s/^version = \".*\"/version = \"${new_version}\"/" "$CARGO_FILE"
  elif [[ "$(uname)" == "Darwin" ]]; then
    # Use BSD sed (default on macOS) - requires empty string for -i
    sed -i '' "s/^version = \".*\"/version = \"${new_version}\"/" "$CARGO_FILE"
  else
    # Use GNU sed (Linux) - no empty string for -i
    sed -i "s/^version = \".*\"/version = \"${new_version}\"/" "$CARGO_FILE"
  fi
fi

echo "Bumped build number to ${next}. VERSION=$(cat "$VERSION_FILE")"



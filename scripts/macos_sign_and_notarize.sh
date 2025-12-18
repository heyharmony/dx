#!/usr/bin/env bash
set -euo pipefail

# macOS signing + notarization helper for distributing the dx binary (or any CLI)
# Requirements:
# - Xcode Command Line Tools
# - Apple Developer account with Developer ID Application certificate installed in login keychain
# - Either an App Store Connect API key profile for notarytool, or Apple ID creds
#
# Usage (examples):
#   ./scripts/macos_sign_and_notarize.sh \
#     --binary ./target/release/dx \
#     --out ./dist \
#     --name dx
#
#   # With environment variables (preferred):
#   DEVELOPER_ID="Developer ID Application: YOUR NAME (TEAMID)" \
#   NOTARY_PROFILE="AC_PROFILE" \
#   ./scripts/macos_sign_and_notarize.sh --binary ./target/release/dx
#
# Notes:
# - The script signs the binary with Hardened Runtime, zips it with ditto, submits for notarization,
#   waits for result, and staples the ticket to the zip (and the binary if applicable).
# - You can also provide Apple ID credentials instead of --keychain-profile by exporting:
#     NOTARY_APPLE_ID, NOTARY_TEAM_ID, NOTARY_PASSWORD (app-specific password)

print_usage() {
  cat <<'USAGE'
Usage: macOS signing and notarization

  ./scripts/macos_sign_and_notarize.sh --binary PATH [--out DIR] [--name NAME] \
    [--entitlements PATH] [--team-id TEAMID] [--sign-id "Developer ID Application: … (TEAMID)"] \
    [--profile AC_PROFILE]

Environment (overrides flags):
  DEVELOPER_ID           Developer ID Application identity string (preferred)
  TEAM_ID                Team ID (fallback if DEVELOPER_ID not provided)
  NOTARY_PROFILE         Keychain profile configured via: xcrun notarytool store-credentials
  NOTARY_APPLE_ID        Apple ID (if no NOTARY_PROFILE)
  NOTARY_TEAM_ID         Team ID (if no NOTARY_PROFILE)
  NOTARY_PASSWORD        App-specific password (if no NOTARY_PROFILE)

Examples:
  DEVELOPER_ID="Developer ID Application: John Appleseed (ABCDE12345)" \\
  NOTARY_PROFILE="AC_PROFILE" \\
  ./scripts/macos_sign_and_notarize.sh --binary ./target/release/dx --out ./dist --name dx
USAGE
}

BIN=""
OUT_DIR=""
NAME=""
ENTITLEMENTS=""
SIGN_ID="${DEVELOPER_ID:-}"
TEAM_ID_FLAG="${TEAM_ID:-}"
PROFILE="${NOTARY_PROFILE:-}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --binary)
      BIN="$2"; shift 2 ;;
    --out)
      OUT_DIR="$2"; shift 2 ;;
    --name)
      NAME="$2"; shift 2 ;;
    --entitlements)
      ENTITLEMENTS="$2"; shift 2 ;;
    --team-id)
      TEAM_ID_FLAG="$2"; shift 2 ;;
    --sign-id)
      SIGN_ID="$2"; shift 2 ;;
    --profile)
      PROFILE="$2"; shift 2 ;;
    -h|--help)
      print_usage; exit 0 ;;
    *)
      echo "Unknown argument: $1" >&2; print_usage; exit 1 ;;
  esac
done

if [[ -z "${BIN}" ]]; then
  echo "--binary PATH is required" >&2
  print_usage
  exit 1
fi

if [[ ! -f "${BIN}" ]]; then
  echo "Binary not found: ${BIN}. Build first (e.g., cargo build --release)." >&2
  exit 1
fi

ABS_BIN=$(cd "$(dirname "${BIN}")" && pwd)/"$(basename "${BIN}")"

if [[ -z "${OUT_DIR}" ]]; then
  OUT_DIR="$(cd "$(dirname "${ABS_BIN}")" && pwd)/dist"
fi
mkdir -p "${OUT_DIR}"

if [[ -z "${NAME}" ]]; then
  NAME="$(basename "${ABS_BIN}")"
fi

ZIP_PATH="${OUT_DIR}/${NAME}.zip"

if [[ -z "${SIGN_ID}" ]]; then
  if [[ -n "${TEAM_ID_FLAG}" ]]; then
    SIGN_ID="Developer ID Application: ${USER} (${TEAM_ID_FLAG})"
  else
    echo "No signing identity provided. Set DEVELOPER_ID or pass --sign-id, or configure TEAM_ID." >&2
    exit 1
  fi
fi

echo "[sign] Identity: ${SIGN_ID}"
echo "[sign] Binary:   ${ABS_BIN}"
echo "[sign] Out dir:  ${OUT_DIR}"

# 1) Ensure executable bit
chmod +x "${ABS_BIN}"

# 2) Sign with hardened runtime
SIGN_ARGS=(--force --options runtime --timestamp --sign "${SIGN_ID}")
if [[ -n "${ENTITLEMENTS}" ]]; then
  SIGN_ARGS+=(--entitlements "${ENTITLEMENTS}")
fi

codesign "${SIGN_ARGS[@]}" "${ABS_BIN}"

# 3) Verify signature locally
codesign --verify --deep --strict --verbose=2 "${ABS_BIN}"

# 4) Create zip using ditto (preserves extended attributes)
echo "[zip] Creating: ${ZIP_PATH}"
rm -f "${ZIP_PATH}"
ditto -c -k --keepParent "${ABS_BIN}" "${ZIP_PATH}"

# 5) Submit for notarization
echo "[notary] Submitting to Apple notarization service"
if [[ -n "${PROFILE}" ]]; then
  xcrun notarytool submit "${ZIP_PATH}" --keychain-profile "${PROFILE}" --wait
else
  if [[ -z "${NOTARY_APPLE_ID:-}" || -z "${NOTARY_TEAM_ID:-}" || -z "${NOTARY_PASSWORD:-}" ]]; then
    echo "Provide NOTARY_PROFILE or NOTARY_APPLE_ID/NOTARY_TEAM_ID/NOTARY_PASSWORD env vars." >&2
    exit 1
  fi
  xcrun notarytool submit "${ZIP_PATH}" \
    --apple-id "${NOTARY_APPLE_ID}" \
    --team-id "${NOTARY_TEAM_ID}" \
    --password "${NOTARY_PASSWORD}" \
    --wait
fi

# 6) Staple ticket to the zip and the binary
echo "[staple] Stapling ticket"
xcrun stapler staple "${ZIP_PATH}" || true
xcrun stapler staple "${ABS_BIN}" || true

echo "[done] Signed, notarized, and stapled. Output: ${ZIP_PATH}"



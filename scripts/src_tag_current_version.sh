#!/usr/bin/env bash
set -euo pipefail

# Tag dx-src current HEAD with VERSION.txt and push tag

DX_SRC_ROOT=${DX_SRC_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}

cd "${DX_SRC_ROOT}"

if [ ! -f "build-artifacts/VERSION.txt" ]; then
  echo "build-artifacts/VERSION.txt not found in ${DX_SRC_ROOT}" >&2
  exit 1
fi

VERSION=$(cat build-artifacts/VERSION.txt)

if [ -z "${VERSION}" ]; then
  echo "VERSION.txt is empty" >&2
  exit 1
fi

git tag -f "${VERSION}"
git push -f origin "refs/tags/${VERSION}"

echo "Tagged and pushed ${VERSION}"



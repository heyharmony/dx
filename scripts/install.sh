#!/usr/bin/env bash
set -euo pipefail

echo "Installing dx and plugin(s)"

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

echo "Building release..."
"$ROOT_DIR/scripts/increment_build.sh" >/dev/null 2>&1 || true
cargo build --release
cargo build -p dx-overlay-cpu --release
cargo build -p dx-plugin-asciinema --release

BIN_DIR="$ROOT_DIR/target/release"
BIN_PATH="$BIN_DIR/dx"
PLUG_DST_DIR="$BIN_DIR/plugins"
CPU_PLUG_SRC="$BIN_DIR/libdx_overlay_cpu.dylib"
CPU_PLUG_DST="$PLUG_DST_DIR/libdx_overlay_cpu.dylib"
ASC_PLUG_SRC="$BIN_DIR/libdx_plugin_asciinema.dylib"
ASC_PLUG_DST="$PLUG_DST_DIR/libdx_plugin_asciinema.dylib"

mkdir -p "$PLUG_DST_DIR"
if [ -f "$CPU_PLUG_SRC" ]; then
  cp -f "$CPU_PLUG_SRC" "$CPU_PLUG_DST"
  cp -f "$CPU_PLUG_SRC" "$PLUG_DST_DIR/libdx_overlay_cpu.dxplugin"
fi
if [ -f "$ASC_PLUG_SRC" ]; then
  cp -f "$ASC_PLUG_SRC" "$ASC_PLUG_DST"
  cp -f "$ASC_PLUG_SRC" "$PLUG_DST_DIR/libdx_plugin_asciinema.dxplugin"
  echo "Placed plugin(s) next to binary: $PLUG_DST_DIR"
else
  echo "WARNING: Some plugins missing in $BIN_DIR"
fi

echo "Preparing local plugin copy next to binary..."
mkdir -p "$PLUG_DST_DIR"
# already copied above for CPU/ASC

# Install scope (default: user). Set DX_INSTALL_SCOPE=global to force global install.
SCOPE="${DX_INSTALL_SCOPE:-user}"

GLOBAL_BIN_DIR="/usr/local/bin"
GLOBAL_LIB_DIR="/usr/local/lib/dx/plugins"
USER_BIN_DIR="$HOME/.local/bin"
USER_LIB_DIR1="$HOME/.local/share/dx/plugins"
USER_LIB_DIR2="$HOME/.dx/plugins"

if [ "$SCOPE" = "global" ]; then
  if [ -d "$GLOBAL_BIN_DIR" ] && [ -w "$GLOBAL_BIN_DIR" ]; then
    mkdir -p "$GLOBAL_LIB_DIR"
    cp -f "$BIN_PATH" "$GLOBAL_BIN_DIR/dx"
    if [ -f "$CPU_PLUG_SRC" ]; then cp -f "$CPU_PLUG_SRC" "$GLOBAL_LIB_DIR/libdx_overlay_cpu.dylib"; cp -f "$CPU_PLUG_SRC" "$GLOBAL_LIB_DIR/libdx_overlay_cpu.dxplugin"; fi
    if [ -f "$ASC_PLUG_SRC" ]; then cp -f "$ASC_PLUG_SRC" "$GLOBAL_LIB_DIR/libdx_plugin_asciinema.dylib"; cp -f "$ASC_PLUG_SRC" "$GLOBAL_LIB_DIR/libdx_plugin_asciinema.dxplugin"; fi
    echo "Installed globally: $GLOBAL_BIN_DIR/dx"
    echo "Plugins at: $GLOBAL_LIB_DIR"
  else
    echo "ERROR: No write access to $GLOBAL_BIN_DIR. Re-run with sudo or use DX_INSTALL_SCOPE=user."
    exit 1
  fi
else
  mkdir -p "$USER_BIN_DIR" "$USER_LIB_DIR1" "$USER_LIB_DIR2"
  cp -f "$BIN_PATH" "$USER_BIN_DIR/dx"
  if [ -f "$CPU_PLUG_SRC" ]; then
    cp -f "$CPU_PLUG_SRC" "$USER_LIB_DIR1/libdx_overlay_cpu.dylib"; cp -f "$CPU_PLUG_SRC" "$USER_LIB_DIR1/libdx_overlay_cpu.dxplugin";
    cp -f "$CPU_PLUG_SRC" "$USER_LIB_DIR2/libdx_overlay_cpu.dylib"; cp -f "$CPU_PLUG_SRC" "$USER_LIB_DIR2/libdx_overlay_cpu.dxplugin";
  fi
  if [ -f "$ASC_PLUG_SRC" ]; then
    cp -f "$ASC_PLUG_SRC" "$USER_LIB_DIR1/libdx_plugin_asciinema.dylib"; cp -f "$ASC_PLUG_SRC" "$USER_LIB_DIR1/libdx_plugin_asciinema.dxplugin";
    cp -f "$ASC_PLUG_SRC" "$USER_LIB_DIR2/libdx_plugin_asciinema.dylib"; cp -f "$ASC_PLUG_SRC" "$USER_LIB_DIR2/libdx_plugin_asciinema.dxplugin";
  fi
  echo "Installed for current user: $USER_BIN_DIR/dx"
  case ":$PATH:" in
    *":$USER_BIN_DIR:"*) ;; # in PATH
    *) echo "Note: add $USER_BIN_DIR to your PATH to run 'dx'." ;;
  esac
fi

echo "Local release binary: $BIN_PATH"
echo "You can run: $BIN_PATH (uses plugins in $PLUG_DST_DIR)"


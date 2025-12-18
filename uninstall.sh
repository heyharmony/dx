#!/bin/bash

# dx uninstall script
# This script removes the dx binary from ~/.local/bin

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

INSTALL_DIR="$HOME/.local/bin"
BINARY_NAME="dx"
INSTALL_PATH="$INSTALL_DIR/$BINARY_NAME"

echo -e "${BLUE}dx uninstaller${NC}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# Check if the binary exists
if [[ ! -f "$INSTALL_PATH" ]]; then
    echo -e "${YELLOW}dx is not installed in $INSTALL_DIR${NC}"
    exit 0
fi

# Remove the binary
echo -e "${YELLOW}Removing dx from $INSTALL_PATH${NC}"
rm "$INSTALL_PATH"

# Verify removal
if [[ ! -f "$INSTALL_PATH" ]]; then
    echo -e "${GREEN}✓ dx uninstalled successfully!${NC}"
    
    # Clear shell cache
    hash -d dx 2>/dev/null || true
    
    echo -e "${GREEN}Installation removed from: $INSTALL_PATH${NC}"
else
    echo -e "${RED}Error: Uninstallation failed${NC}"
    exit 1
fi
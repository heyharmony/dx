#!/bin/bash

# dx install script
# This script builds and installs the dx binary to ~/.local/bin

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Get the directory where this script is located
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INSTALL_DIR="$HOME/.local/bin"
BINARY_NAME="dx"
TARGET_BINARY="$SCRIPT_DIR/target/release/$BINARY_NAME"
INSTALL_PATH="$INSTALL_DIR/$BINARY_NAME"

echo -e "${BLUE}dx installer${NC}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# Change to the project directory
cd "$SCRIPT_DIR"

# Build the project
echo -e "${YELLOW}Building dx in release mode...${NC}"
if ! cargo build --release; then
    echo -e "${RED}Error: Failed to build dx${NC}"
    exit 1
fi

# Check if the binary was created
if [[ ! -f "$TARGET_BINARY" ]]; then
    echo -e "${RED}Error: Binary not found at $TARGET_BINARY${NC}"
    exit 1
fi

# Create the install directory if it doesn't exist
if [[ ! -d "$INSTALL_DIR" ]]; then
    echo -e "${YELLOW}Creating install directory: $INSTALL_DIR${NC}"
    mkdir -p "$INSTALL_DIR"
fi

# Remove old binary if it exists
if [[ -f "$INSTALL_PATH" ]]; then
    echo -e "${YELLOW}Removing old binary...${NC}"
    rm "$INSTALL_PATH"
fi

# Copy the binary
echo -e "${YELLOW}Installing dx to $INSTALL_PATH${NC}"
cp "$TARGET_BINARY" "$INSTALL_PATH"

# Make sure it's executable
chmod +x "$INSTALL_PATH"

# Verify installation
if [[ -f "$INSTALL_PATH" ]]; then
    echo -e "${GREEN}✓ dx installed successfully!${NC}"
    echo -e "${GREEN}✓ Location: $INSTALL_PATH${NC}"
    
    # Check if ~/.local/bin is in PATH
    if [[ ":$PATH:" == *":$INSTALL_DIR:"* ]]; then
        echo -e "${GREEN}✓ $INSTALL_DIR is in your PATH${NC}"
    else
        echo -e "${YELLOW}⚠ Warning: $INSTALL_DIR is not in your PATH${NC}"
        echo -e "${YELLOW}  Add this line to your shell profile (~/.zshrc, ~/.bashrc, etc.):${NC}"
        echo -e "${BLUE}  export PATH=\"$INSTALL_DIR:\$PATH\"${NC}"
    fi
    
    # Show version
    echo -e "${BLUE}Installed version:${NC}"
    "$INSTALL_PATH" --version
    
    echo -e "\n${GREEN}Installation complete! You can now run: dx${NC}"
else
    echo -e "${RED}Error: Installation failed${NC}"
    exit 1
fi
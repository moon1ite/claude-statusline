#!/bin/bash
set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Directories
CLAUDE_DIR="$HOME/.claude"
BIN_DIR="$CLAUDE_DIR/bin"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo -e "${BLUE}╔════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║${NC}       Claude Code Statusline Plugin Installer          ${BLUE}║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════════════════════╝${NC}"
echo ""

# Check for required tools
echo -e "${YELLOW}Checking prerequisites...${NC}"

# Check for Rust/Cargo
if ! command -v cargo &> /dev/null; then
    echo -e "${RED}Error: Rust/Cargo not found.${NC}"
    echo -e "Please install Rust first: ${BLUE}https://rustup.rs${NC}"
    echo -e "Run: ${GREEN}curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh${NC}"
    exit 1
fi
echo -e "  ${GREEN}✓${NC} Rust/Cargo found: $(cargo --version)"

# Check for jq (required by statusline.sh)
if ! command -v jq &> /dev/null; then
    echo -e "${RED}Error: jq not found.${NC}"
    echo -e "Please install jq first:"
    echo -e "  macOS: ${GREEN}brew install jq${NC}"
    echo -e "  Linux: ${GREEN}sudo apt install jq${NC}"
    exit 1
fi
echo -e "  ${GREEN}✓${NC} jq found: $(jq --version)"

# Check for git (for git status in statusline)
if ! command -v git &> /dev/null; then
    echo -e "${YELLOW}Warning: git not found. Git status will not be displayed.${NC}"
else
    echo -e "  ${GREEN}✓${NC} git found: $(git --version | head -1)"
fi

echo ""

# Create directories
echo -e "${YELLOW}Creating directories...${NC}"
mkdir -p "$BIN_DIR"
echo -e "  ${GREEN}✓${NC} Created $BIN_DIR"

# Build the Rust binary
echo ""
echo -e "${YELLOW}Building claude-status binary...${NC}"
cd "$SCRIPT_DIR"
cargo build --release

if [ ! -f "$SCRIPT_DIR/target/release/claude-status" ]; then
    echo -e "${RED}Error: Build failed. Binary not found.${NC}"
    exit 1
fi
echo -e "  ${GREEN}✓${NC} Build successful"

# Install binary
echo ""
echo -e "${YELLOW}Installing files...${NC}"
cp "$SCRIPT_DIR/target/release/claude-status" "$BIN_DIR/"
chmod +x "$BIN_DIR/claude-status"
echo -e "  ${GREEN}✓${NC} Installed binary to $BIN_DIR/claude-status"

# Install statusline.sh
cp "$SCRIPT_DIR/statusline.sh" "$CLAUDE_DIR/"
chmod +x "$CLAUDE_DIR/statusline.sh"
echo -e "  ${GREEN}✓${NC} Installed statusline.sh to $CLAUDE_DIR/"

# Configure Claude Code settings
echo ""
echo -e "${YELLOW}Configuring Claude Code...${NC}"

SETTINGS_FILE="$CLAUDE_DIR/settings.json"

if [ -f "$SETTINGS_FILE" ]; then
    # Check if statusLine is already configured
    if grep -q '"statusLine"' "$SETTINGS_FILE"; then
        echo -e "  ${YELLOW}!${NC} statusLine already configured in settings.json"
        echo -e "    Please verify it points to: ${GREEN}~/.claude/statusline.sh${NC}"
    else
        # Backup existing settings
        cp "$SETTINGS_FILE" "$SETTINGS_FILE.backup"
        echo -e "  ${GREEN}✓${NC} Backed up settings to $SETTINGS_FILE.backup"

        # Add statusLine configuration using jq
        jq '. + {"statusLine": {"type": "command", "command": "~/.claude/statusline.sh", "padding": 0}}' \
            "$SETTINGS_FILE" > "$SETTINGS_FILE.tmp" && mv "$SETTINGS_FILE.tmp" "$SETTINGS_FILE"
        echo -e "  ${GREEN}✓${NC} Added statusLine configuration to settings.json"
    fi
else
    # Create new settings file
    cat > "$SETTINGS_FILE" << 'EOF'
{
  "statusLine": {
    "type": "command",
    "command": "~/.claude/statusline.sh",
    "padding": 0
  }
}
EOF
    echo -e "  ${GREEN}✓${NC} Created settings.json with statusLine configuration"
fi

# Verify installation
echo ""
echo -e "${YELLOW}Verifying installation...${NC}"

if [ -x "$BIN_DIR/claude-status" ]; then
    echo -e "  ${GREEN}✓${NC} Binary is executable"
else
    echo -e "  ${RED}✗${NC} Binary not executable"
fi

if [ -x "$CLAUDE_DIR/statusline.sh" ]; then
    echo -e "  ${GREEN}✓${NC} statusline.sh is executable"
else
    echo -e "  ${RED}✗${NC} statusline.sh not executable"
fi

# Success message
echo ""
echo -e "${GREEN}╔════════════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║${NC}            Installation Complete!                      ${GREEN}║${NC}"
echo -e "${GREEN}╚════════════════════════════════════════════════════════╝${NC}"
echo ""
echo -e "The statusline plugin is now installed. Restart Claude Code to see it."
echo ""
echo -e "${BLUE}What you'll see:${NC}"
echo -e "  Line 1: Context usage | Cost | Git status | Directory | Model"
echo -e "  Line 2: Todos | Agents | Tools (real-time activity)"
echo ""
echo -e "${BLUE}To uninstall:${NC}"
echo -e "  ${GREEN}./uninstall.sh${NC}"
echo ""

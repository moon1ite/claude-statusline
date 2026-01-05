#!/bin/bash
set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m'

CLAUDE_DIR="$HOME/.claude"
BIN_DIR="$CLAUDE_DIR/bin"
SETTINGS_FILE="$CLAUDE_DIR/settings.json"

echo -e "${BLUE}╔════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║${NC}       Claude Code Statusline Plugin Uninstaller        ${BLUE}║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════════════════════╝${NC}"
echo ""

# Remove binary
if [ -f "$BIN_DIR/claude-status" ]; then
    rm "$BIN_DIR/claude-status"
    echo -e "  ${GREEN}✓${NC} Removed $BIN_DIR/claude-status"
else
    echo -e "  ${YELLOW}!${NC} Binary not found (already removed?)"
fi

# Remove statusline.sh
if [ -f "$CLAUDE_DIR/statusline.sh" ]; then
    rm "$CLAUDE_DIR/statusline.sh"
    echo -e "  ${GREEN}✓${NC} Removed $CLAUDE_DIR/statusline.sh"
else
    echo -e "  ${YELLOW}!${NC} statusline.sh not found (already removed?)"
fi

# Remove statusLine from settings.json
if [ -f "$SETTINGS_FILE" ]; then
    if grep -q '"statusLine"' "$SETTINGS_FILE"; then
        # Backup settings
        cp "$SETTINGS_FILE" "$SETTINGS_FILE.backup"

        # Remove statusLine key using jq
        if command -v jq &> /dev/null; then
            jq 'del(.statusLine)' "$SETTINGS_FILE" > "$SETTINGS_FILE.tmp" && mv "$SETTINGS_FILE.tmp" "$SETTINGS_FILE"
            echo -e "  ${GREEN}✓${NC} Removed statusLine from settings.json"
            echo -e "  ${GREEN}✓${NC} Backup saved to $SETTINGS_FILE.backup"
        else
            echo -e "  ${YELLOW}!${NC} jq not found - please manually remove 'statusLine' from settings.json"
        fi
    else
        echo -e "  ${YELLOW}!${NC} statusLine not found in settings.json"
    fi
fi

echo ""
echo -e "${GREEN}╔════════════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║${NC}            Uninstallation Complete!                    ${GREEN}║${NC}"
echo -e "${GREEN}╚════════════════════════════════════════════════════════╝${NC}"
echo ""
echo -e "Restart Claude Code to apply changes."
echo ""

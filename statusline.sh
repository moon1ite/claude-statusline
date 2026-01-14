#!/bin/bash

# Read JSON input from stdin
input=$(cat)

# Extract information from JSON
model_name=$(echo "$input" | jq -r '.model.display_name')
current_dir=$(echo "$input" | jq -r '.workspace.current_dir')
transcript_path=$(echo "$input" | jq -r '.transcript_path // empty')

# Extract context percentage (available since Claude Code 2.1.6)
context_percent=$(echo "$input" | jq -r '.context_window.used_percentage // 0' | cut -d'.' -f1)

# Build context progress bar (20 chars wide)
bar_width=15
filled=$((context_percent * bar_width / 100))
empty=$((bar_width - filled))
bar=""
for ((i = 0; i < filled; i++)); do bar+="█"; done
for ((i = 0; i < empty; i++)); do bar+="░"; done

# Extract cost information
session_cost=$(echo "$input" | jq -r '.cost.total_cost_usd // empty')
[ "$session_cost" != "empty" ] && session_cost=$(printf "%.4f" "$session_cost") || session_cost=""

# Get directory name (basename)
dir_name=$(basename "$current_dir")

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
GRAY='\033[0;37m'
NC='\033[0m' # No Color

# Change to the current directory to get git info
cd "$current_dir" 2>/dev/null || cd /

# Get git branch (file stats now shown natively in input line)
if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  branch=$(git branch --show-current 2>/dev/null || echo "detached")
  git_info=" ${YELLOW}${branch}${NC}"
else
  git_info=""
fi

# Add session cost if available
cost_info=""
if [ -n "$session_cost" ] && [ "$session_cost" != "null" ] && [ "$session_cost" != "empty" ]; then
  cost_info=" ${GRAY}(\$$session_cost)${NC}"
fi

# Build context bar display
context_info="${GRAY}${bar}${NC} ${context_percent}%"

# Line 1: Existing bash output (context, cost, git, dir, model)
line1="${context_info}${cost_info}${git_info:+ ${GRAY}|${NC}}${git_info} ${GRAY}|${NC} ${BLUE}${dir_name}${NC} ${GRAY}|${NC} ${CYAN}${model_name}${NC}"

# Line 2: Tool/agent/todo status from Rust binary
line2=""
if [ -n "$transcript_path" ] && [ -f "$transcript_path" ]; then
    line2=$(~/.claude/bin/claude-status "$transcript_path" 2>/dev/null)
fi

# Output the status lines
if [ -n "$line2" ]; then
    echo -e "${line1}\n${line2}"
else
    echo -e "${line1}"
fi

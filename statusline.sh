#!/bin/bash

# Read JSON input from stdin
input=$(cat)

# Extract all fields from JSON in a single jq call (pipe delimiter avoids empty-field collapse)
IFS='|' read -r model_name current_dir transcript_path session_id session_cost context_percent \
  <<< "$(echo "$input" | jq -r '[
    .model.display_name,
    .workspace.current_dir,
    (.transcript_path // ""),
    (.session_id // ""),
    (.cost.total_cost_usd // ""),
    ((.context_window.used_percentage // 0) | floor | tostring)
  ] | join("|")')"

# Colors
BLUE='\033[0;34m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
GRAY='\033[0;37m'
NC='\033[0m' # No Color

# ============================================================================
# Usage Limit / Reset Time
# Primary: Anthropic OAuth usage API (exact utilization + reset time)
# Fallback: system-reminder parsing from transcripts
# ============================================================================

reset_info=""
now_sec=$(date +%s)

_file_age() {
    local f="$1"
    echo $((now_sec - $(stat -f%m "$f" 2>/dev/null || stat -c%Y "$f" 2>/dev/null || echo 0)))
}

_iso_to_epoch() {
    local ts="$1" stripped="${1%%.*}"
    stripped="${stripped%%+*}"
    stripped="${stripped%%Z*}"
    if [[ "$OSTYPE" == "darwin"* ]]; then
        date -j -u -f "%Y-%m-%dT%H:%M:%S" "$stripped" "+%s" 2>/dev/null
    else
        date -d "$ts" "+%s" 2>/dev/null
    fi
}

# --- Primary: Anthropic usage API -------------------------------------------

USAGE_API_CACHE="$HOME/.cache/claude-statusline/usage.json"
USAGE_API_LOCK="$HOME/.cache/claude-statusline/usage.lock"
USAGE_API_TTL=180    # 3 min cache
USAGE_API_LOCK_TTL=30

session_use_pct=""
reset_sec=""

# Load from API cache (stale-while-revalidate)
if [ -f "$USAGE_API_CACHE" ]; then
    IFS='|' read -r session_use_pct api_reset_str \
      <<< "$(jq -r '[
        (if .five_hour.utilization then (.five_hour.utilization | floor | tostring) else "" end),
        (.five_hour.resets_at // "")
      ] | join("|")' "$USAGE_API_CACHE" 2>/dev/null)"
    [ -n "$api_reset_str" ] && reset_sec=$(_iso_to_epoch "$api_reset_str")
fi

# Background API fetch when cache is stale
api_stale=1
[ -f "$USAGE_API_CACHE" ] && [ "$(_file_age "$USAGE_API_CACHE")" -lt "$USAGE_API_TTL" ] && api_stale=0

if [ "$api_stale" = "1" ]; then
    lock_ok=1
    [ -f "$USAGE_API_LOCK" ] && [ "$(_file_age "$USAGE_API_LOCK")" -lt "$USAGE_API_LOCK_TTL" ] && lock_ok=0

    if [ "$lock_ok" = "1" ]; then
        (
            mkdir -p "$(dirname "$USAGE_API_CACHE")" 2>/dev/null
            touch "$USAGE_API_LOCK"

            # Get OAuth token
            token=""
            if [[ "$OSTYPE" == "darwin"* ]]; then
                creds=$(security find-generic-password -s "Claude Code-credentials" -w 2>/dev/null)
                token=$(echo "$creds" | jq -r '.claudeAiOauth.accessToken // empty' 2>/dev/null)
            else
                creds_file="${CLAUDE_CONFIG_DIR:-$HOME/.claude}/.credentials.json"
                [ -f "$creds_file" ] && token=$(jq -r '.claudeAiOauth.accessToken // empty' "$creds_file" 2>/dev/null)
            fi
            [ -z "$token" ] && exit

            resp=$(curl -s --max-time 5 \
                -H "Authorization: Bearer $token" \
                -H "anthropic-beta: oauth-2025-04-20" \
                "https://api.anthropic.com/api/oauth/usage" 2>/dev/null)

            # Validate and cache
            echo "$resp" | jq -e '.five_hour.utilization' >/dev/null 2>&1 && echo "$resp" > "$USAGE_API_CACHE"
        ) &
    fi
fi

# --- Fallback: system-reminder from transcripts ------------------------------
# Only if API didn't provide data (no cache yet, or credentials missing)

FALLBACK_CACHE_TTL=15
TRANSCRIPT_MAX_AGE_MIN=360

if [ -z "$session_use_pct" ]; then
    SESSION_LIMIT_CACHE="$HOME/.claude/cache/claude-session-limit"

    if [ -f "$SESSION_LIMIT_CACHE" ]; then
        IFS=' ' read -r sl_cached_reset sl_cached_pct < "$SESSION_LIMIT_CACHE"
        if [ -n "$sl_cached_reset" ] && [ "$sl_cached_reset" -gt "$now_sec" ] 2>/dev/null; then
            reset_sec="$sl_cached_reset"
            session_use_pct="$sl_cached_pct"
        fi
    fi

    sl_stale=1
    [ -f "$SESSION_LIMIT_CACHE" ] && [ "$(_file_age "$SESSION_LIMIT_CACHE")" -lt "$FALLBACK_CACHE_TTL" ] && sl_stale=0

    if [ "$sl_stale" = "1" ]; then
        (
            best_reset=0 best_pct="" best_ts=0
            for f in $(find ~/.claude/projects -name "*.jsonl" ! -path "*/subagents/*" -mmin -$TRANSCRIPT_MAX_AGE_MIN 2>/dev/null); do
                line=$(grep 'Current session\\n\\nResets in' "$f" 2>/dev/null | grep '% used' | tail -1)
                [ -z "$line" ] && continue
                ts=$(echo "$line" | grep -oE '"timestamp":"[^"]*"' | head -1 | sed 's/"timestamp":"//;s/"//')
                [ -z "$ts" ] && continue
                msg_ep=$(_iso_to_epoch "$ts")
                [ -z "$msg_ep" ] || [ "$msg_ep" -le "$best_ts" ] && continue
                hours=$(echo "$line" | grep -oE 'Resets in [0-9]+ hr' | grep -oE '[0-9]+' | head -1)
                mins=$(echo "$line" | grep -oE '[0-9]+ min' | head -1 | grep -oE '[0-9]+')
                [ -z "$hours" ] && hours=0
                [ -z "$mins" ] && mins=0
                pct=$(echo "$line" | grep -oE '[0-9]+% used' | tail -1 | grep -oE '[0-9]+')
                [ -z "$pct" ] && continue
                reset=$((msg_ep + hours * 3600 + mins * 60))
                best_reset="$reset"; best_pct="$pct"; best_ts="$msg_ep"
            done
            [ "$best_reset" -gt 0 ] && echo "$best_reset $best_pct" > "$SESSION_LIMIT_CACHE"
        ) &
    fi
fi

# --- Format ------------------------------------------------------------------

if [ -n "$reset_sec" ] && [ "$reset_sec" -gt "$now_sec" ] 2>/dev/null; then
    remaining=$(( reset_sec - now_sec ))
    rh=$(( remaining / 3600 ))
    rm=$(( (remaining % 3600) / 60 ))
    use_info=""
    [ -n "$session_use_pct" ] && use_info=" (${session_use_pct}%)"
    if (( remaining > 60 )); then
        reset_info=" ${GRAY}⏱ ${rh}h${rm}m${use_info}${NC}"
    else
        reset_info=" ${GRAY}${use_info:-resetting}${NC}"
    fi
elif [ -n "$session_use_pct" ] && [ "$session_use_pct" -gt 0 ] 2>/dev/null; then
    reset_info=" ${GRAY}(${session_use_pct}%)${NC}"
fi

# Build context progress bar (15 chars wide)
full="███████████████"
empty_str="░░░░░░░░░░░░░░░"
bar_width=15
filled=$((context_percent * bar_width / 100))
empty=$((bar_width - filled))
bar="${full:0:filled}${empty_str:0:empty}"

# Format session cost
cost_info=""
if [ -n "$session_cost" ] && [ "$session_cost" != "null" ] && [ "$session_cost" != "empty" ]; then
    session_cost=$(printf "%.2f" "$session_cost")
    cost_info=" ${GRAY}\$${session_cost}${NC}"
fi

# Get directory name and git branch
dir_name="${current_dir##*/}"
if git -C "$current_dir" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    branch=$(git -C "$current_dir" branch --show-current 2>/dev/null || echo "detached")
    git_info=" ${YELLOW}${branch}${NC}"
else
    git_info=""
fi

# Build context bar display
context_info="${GRAY}${bar}${NC} ${context_percent}%"

# Line 1: Context, cost, usage limit, git, dir, model
line1="${context_info}${cost_info}${reset_info}${git_info:+ ${GRAY}|${NC}}${git_info} ${GRAY}|${NC} ${BLUE}${dir_name}${NC} ${GRAY}|${NC} ${CYAN}${model_name}${NC}"

# Line 2: Tool/agent/todo status from Rust binary
line2=""
if [ -n "$transcript_path" ]; then
    line2=$(~/.claude/bin/claude-status "$transcript_path" "$session_id" 2>/dev/null)
fi

# Output the status lines
if [ -n "$line2" ]; then
    echo -e "${line1}\n${line2}"
else
    echo -e "${line1}"
fi

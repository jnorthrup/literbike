#!/usr/bin/env bash
# literbike-claude.sh — Run Claude Code routed through modelmux
# Usage: ./literbike-claude.sh [model-id]
#   No args: interactive fzf picker from live modelmux models
#   With arg: use that model directly
#
# Sets the claude_rewrite_policy so Claude Code's model requests
# get rewritten to the selected modelmux model.

set -euo pipefail

MODELMUX_URL="${MODELMUX_URL:-http://127.0.0.1:8888}"

# Check modelmux is running
if ! curl -sf "$MODELMUX_URL/health" >/dev/null 2>&1; then
  echo "modelmux not running on $MODELMUX_URL" >&2
  echo "Start it: cd $(dirname "$0") && ./target/debug/modelmux &" >&2
  exit 1
fi

pick_model() {
  local models
  models=$(curl -sf "$MODELMUX_URL/v1/models" \
    | python3 -c "
import sys, json
d = json.load(sys.stdin)
for m in sorted(d.get('data', []), key=lambda x: x['id']):
    print(m['id'])
" 2>/dev/null)

  if [ -z "$models" ]; then
    echo "No models available from modelmux" >&2
    exit 1
  fi

  if command -v fzf >/dev/null 2>&1; then
    echo "$models" | fzf --prompt="model> " --height=40% --reverse
  else
    echo "Available models:" >&2
    local i=1
    while IFS= read -r m; do
      printf "  %3d  %s\n" "$i" "$m" >&2
      i=$((i + 1))
    done <<< "$models"
    printf "Select [1-%d]: " "$((i - 1))" >&2
    read -r choice
    echo "$models" | sed -n "${choice}p"
  fi
}

if [ $# -ge 1 ]; then
  MODEL="$1"
  shift
else
  MODEL=$(pick_model)
fi

if [ -z "$MODEL" ]; then
  echo "No model selected" >&2
  exit 1
fi

echo "Selected: $MODEL" >&2

# Set modelmux claude rewrite policy — all claude model variants → selected model
curl -sf "$MODELMUX_URL/toolbar/actions" \
  -H 'Content-Type: application/json' \
  -d "$(python3 -c "
import json
print(json.dumps({
    'action': 'set_claude_rewrite_policy',
    'enabled': True,
    'default_model': '$MODEL',
    'haiku_model': '$MODEL',
    'sonnet_model': '$MODEL',
    'opus_model': '$MODEL',
    'reasoning_model': '$MODEL'
}))
")" >/dev/null

echo "Rewrite policy set: claude-* → $MODEL" >&2
echo "Launching claude code → modelmux ($MODELMUX_URL)" >&2

# Point Claude Code at modelmux as its Anthropic backend
export ANTHROPIC_BASE_URL="$MODELMUX_URL"
export ANTHROPIC_API_KEY="${ANTHROPIC_API_KEY:-literbike}"

exec claude "$@"

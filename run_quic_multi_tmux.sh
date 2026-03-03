#!/bin/bash
# Run multiple concurrent tmux sessions/panes for QUIC smoke testing with Chrome
# This creates multiple parallel test environments
#
# Usage:
# ./run_quic_multi_tmux.sh [N]  # Create N parallel test sessions (default: 3)
# ./run_quic_multi_tmux.sh --kill  # Kill all quic-test sessions
# ./run_quic_multi_tmux.sh --watch  # Watch all sessions
# ./run_quic_multi_tmux.sh --status # Show status of all sessions

set -e

BASE_SESSION="quic-test"
NUM_SESSIONS=${1:-3}

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

kill_all_sessions() {
    echo -e "${RED}Killing all QUIC tmux sessions...${NC}"
    for i in $(seq 1 $NUM_SESSIONS); do
        local sess="${BASE_SESSION}-${i}"
        tmux kill-session -t "$sess" 2>/dev/null || true
        echo "  Killed $sess"
    done
    # Kill any remaining quic-test sessions
    tmux ls 2>/dev/null | grep "^${BASE_SESSION}" | cut -d: -f1 | while read sess; do
        tmux kill-session -t "$sess" 2>/dev/null || true
        echo "  Killed $sess"
    done
    exit 0
}

show_status() {
    echo -e "${BLUE}=== QUIC Test Session Status ===${NC}"
    tmux ls 2>/dev/null | grep "^${BASE_SESSION}" || echo "No active sessions"
    echo ""
    echo -e "${BLUE}=== Active Litebike Processes ===${NC}"
    pgrep -a -f "litebike.*quic-vqa" || echo "No litebike processes"
    echo ""
    echo -e "${BLUE}=== Chrome Processes ===${NC}"
    pgrep -a -f "Chrome.*quic" || echo "No Chrome QUIC processes"
}

watch_logs() {
    local session_name="${BASE_SESSION}-$1"
    if [ -z "$1" ]; then
        echo "Usage: $0 --watch N (where N is session number)"
        exit 1
    fi
    echo "Watching $session_name server logs (Ctrl+C to exit)..."
    tmux capture-pane -t "$session_name:server" -p 2>/dev/null | tail -50 || echo "Session not found"
}

# Handle flags
case "${1:-}" in
    --kill|-k)
        kill_all_sessions
        ;;
    --status|-s)
        show_status
        exit 0
        ;;
    --watch|-w)
        watch_logs "$2"
        exit 0
        ;;
    --help|-h)
        echo "Usage: $0 [N|OPTIONS]"
        echo ""
        echo "Create N parallel QUIC test sessions (default: 3)"
        echo ""
        echo "Options:"
        echo "  N              Number of parallel sessions to create (default: 3)"
        echo "  --kill, -k     Kill all QUIC test sessions"
        echo "  --status, -s   Show status of all sessions"
        echo "  --watch N, -w  Watch logs for session N"
        echo "  --help, -h     Show this help"
        echo ""
        echo "Examples:"
        echo "  $0 5           Create 5 parallel test sessions"
        echo "  $0 --kill      Kill all sessions"
        echo "  $0 --watch 1   Watch session 1 logs"
        exit 0
        ;;
esac

# Check if NUM_SESSIONS is a number
if ! [[ "$NUM_SESSIONS" =~ ^[0-9]+$ ]]; then
    echo "Error: Expected number of sessions, got: $NUM_SESSIONS"
    echo "Use --help for usage information"
    exit 1
fi

echo -e "${GREEN}Creating $NUM_SESSIONS parallel QUIC test sessions...${NC}"

# Kill existing sessions
for i in $(seq 1 $NUM_SESSIONS); do
    tmux kill-session -t "${BASE_SESSION}-${i}" 2>/dev/null || true
done

# Create sessions with staggered ports
for i in $(seq 1 $NUM_SESSIONS); do
    PORT=$((4433 + i - 1))
    SESSION_NAME="${BASE_SESSION}-${i}"

    echo -e "${BLUE}[$i/$NUM_SESSIONS] Creating session '$SESSION_NAME' on port $PORT...${NC}"

    # Create new session with server window
    tmux new-session -d -s "$SESSION_NAME" -n "server" \
        "echo 'Starting litebike QUIC server on port $PORT...'; cargo run --release --features='warp git2 tls-quic ring' --bin litebike -- quic-vqa $PORT 2>&1"

    # Window 2: Smoke test
    tmux new-window -t "$SESSION_NAME" -n "smoke-test"
    tmux send-keys -t "$SESSION_NAME:smoke-test" "echo 'Waiting for server on port $PORT...'; sleep 35; echo 'Running smoke test...'; cargo run --release --bin quic_smoke_test 2>&1" C-m

    # Window 3: Packet capture (optional, if tshark available)
    tmux new-window -t "$SESSION_NAME" -n "capture"
    tmux send-keys -t "$SESSION_NAME:capture" "echo 'Capturing QUIC traffic on port $PORT...'; sleep 5; sudo tshark -i lo -f 'udp port $PORT' -T text 2>&1 || echo 'tshark not available or needs sudo'" C-m

    # Window 4: Chrome (only for first session to avoid conflicts)
    if [ "$i" -eq 1 ]; then
        tmux new-window -t "$SESSION_NAME" -n "chrome"
        tmux send-keys -t "$SESSION_NAME:chrome" "cd /Users/jim/work/literbike; sleep 40; ./test_chrome_quic.sh 2>&1 || echo 'Chrome launch failed'" C-m
    fi

done

echo ""
echo -e "${GREEN}✅ Created $NUM_SESSIONS QUIC test sessions${NC}"
echo ""
echo "Session Layout (per session):"
echo "  Window 1: server     - QUIC server (litebike)"
echo "  Window 2: smoke-test - Automated smoke test"
echo "  Window 3: capture    - Packet capture (tshark)"
echo "  Window 4: chrome     - Chrome browser (session 1 only)"
echo ""
echo -e "${YELLOW}Commands:${NC}"
for i in $(seq 1 $NUM_SESSIONS); do
    echo "  tmux attach -t ${BASE_SESSION}-$i    # Attach to session $i"
done
echo ""
echo "  tmux ls                              # List all sessions"
echo "  $0 --kill                            # Kill all sessions"
echo "  $0 --status                          # Show status"
echo "  $0 --watch 1                         # Watch session 1 logs"
echo ""
echo -e "${YELLOW}In tmux:${NC}"
echo "  Ctrl+b n         Next window"
echo "  Ctrl+b p         Previous window"
echo "  Ctrl+b [0-9]     Go to window number"
echo "  Ctrl+b d         Detach"
echo ""
echo -e "${GREEN}Waiting for servers to start (this will take 30-60 seconds)...${NC}"
echo -e "${YELLOW}Monitor with: watch -n 5 'pgrep -f litebike'${NC}"

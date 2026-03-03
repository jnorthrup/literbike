#!/bin/bash
# Run multiple tmux sessions/panes for QUIC smoke testing with Chrome
#
# Usage:
#   ./run_quic_tmux.sh           # Create session with server + smoke test
#   ./run_quic_tmux.sh --chrome  # Include Chrome launch window
#   ./run_quic_tmux.sh --kill    # Kill the session
#   ./run_quic_tmux.sh --watch   # Watch server logs

set -e

SESSION="quic-test"

kill_session() {
    echo "Killing tmux session '$SESSION'..."
    tmux kill-session -t "$SESSION" 2>/dev/null || true
    exit 0
}

watch_logs() {
    echo "Watching server logs (Ctrl+C to exit)..."
    tmux capture-pane -t "$SESSION:server" -p | tail -50
    exit 0
}

# Parse arguments
LAUNCH_CHROME=false
if [[ "$1" == "--kill" ]]; then
    kill_session
elif [[ "$1" == "--watch" ]]; then
    watch_logs
elif [[ "$1" == "--chrome" ]]; then
    LAUNCH_CHROME=true
elif [[ "$1" == "--help" || "$1" == "-h" ]]; then
    echo "Usage: $0 [--chrome|--kill|--watch|--help]"
    echo "  --chrome  Include Chrome launch window"
    echo "  --watch   Watch server logs"
    echo "  --kill    Kill existing session"
    echo "  --help    Show this help"
    exit 0
fi

# Kill existing session if present
if tmux has-session -t "$SESSION" 2>/dev/null; then
    echo "Session '$SESSION' already exists. Killing and recreating..."
    tmux kill-session -t "$SESSION"
fi

echo "Creating tmux session '$SESSION'..."

# Create new session with server window
tmux new-session -d -s "$SESSION" -n "server" "cargo run --release --features='warp git2 tls-quic ring' --bin litebike -- quic-vqa 4433"

# Wait for compilation
echo "⏳ Waiting for server compilation (this may take 30-60 seconds)..."
sleep 30

# Window 2: HTTP test (Alt-Svc beacon)
tmux new-window -t "$SESSION" -n "http-test"
tmux send-keys -t "$SESSION:http-test" "curl -v http://127.0.0.1:4433/" C-m

# Window 3: Chrome (optional)
if $LAUNCH_CHROME; then
    tmux new-window -t "$SESSION" -n "chrome"
    tmux send-keys -t "$SESSION:chrome" "cd /Users/jim/work/literbike" C-m
    tmux send-keys -t "$SESSION:chrome" "./test_chrome_quic.sh" C-m
fi

echo ""
echo "✅ Created tmux session '$SESSION' with $(($LAUNCH_CHROME ? 3 : 2)) windows:"
echo "   1. server      - QUIC server (litebike quic-vqa 4433)"
echo "   2. http-test   - Test HTTP Alt-Svc beacon"
if $LAUNCH_CHROME; then
    echo "   3. chrome      - Chrome browser with QUIC"
fi
echo ""
echo "Useful commands:"
echo "   tmux attach -t $SESSION        # Attach to session"
echo "   tmux kill-session -t $SESSION  # Cleanup"
echo "   tmux ls                        # List sessions"
echo ""
echo "Server should start in ~30 seconds. Check logs with:"
echo "   tmux capture-pane -t $SESSION:server -p | tail -40"
echo ""
echo "In tmux: Ctrl+b then 1/2/3 to switch windows"
echo ""
echo "Note: The HTTP Alt-Svc beacon should work (stream 0)."
echo "      Chrome QUIC may close connection during TLS handshake (this is being debugged)."

#!/bin/bash
# Run multiple tmux sessions for parallel QUIC testing
# Usage: ./run_quic_parallel.sh [N] where N is number of parallel instances (default: 3)

set -e

NUM_INSTANCES=${1:-3}
BASE_PORT=4433

echo "Creating $NUM_INSTANCES parallel QUIC test sessions..."

# Kill existing sessions
for i in $(seq 1 $NUM_INSTANCES); do
    tmux kill-session -t "quic-$i" 2>/dev/null || true
done

# Create each session
for i in $(seq 1 $NUM_INSTANCES); do
    PORT=$((BASE_PORT + i - 1))
    SESSION="quic-$i"

    echo "Creating session $SESSION on port $PORT..."

    # Create session with three panes
    tmux new-session -d -s "$SESSION" -n "test"

    # Split horizontally - left pane for server, right pane for tests
    tmux split-window -h -t "$SESSION:test"

    # Left pane: Build and run server
    tmux send-keys -t "$SESSION:test.0" "echo '=== Server $i on port $PORT ===' && cargo run --release --features=\"warp git2 tls-quic ring\" --bin litebike -- quic-vqa $PORT 2>&1" C-m

    # Right pane: Wait then run test
    if [ "$i" -eq 1 ]; then
        # First instance gets Chrome test
        tmux send-keys -t "$SESSION:test.1" "echo '=== Chrome Test on port $PORT ===' && sleep 40 && ./test_chrome_quic.sh" C-m
    else
        # Others get smoke test
        tmux send-keys -t "$SESSION:test.1" "echo '=== Smoke Test on port $PORT ===' && sleep 35 && cargo run --release --bin quic_smoke_test 2>&1" C-m
    fi

done

echo ""
echo "Created $NUM_INSTANCES tmux sessions:"
for i in $(seq 1 $NUM_INSTANCES); do
    echo "  quic-$i: Server on port $((BASE_PORT + i - 1))"
done
echo ""
echo "Attach to a session: tmux attach -t quic-N"
echo "List all: tmux ls"
echo "Kill all: for i in \$(seq 1 $NUM_INSTANCES); do tmux kill-session -t quic-\$i; done"
echo ""
echo "To see all sessions at once, run: tmux ls"

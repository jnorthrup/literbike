#!/bin/bash
# Launch QUIC server + Chrome in split tmux session
SESSION="quic-chrome"

# Kill existing session if any
tmux kill-session -t "$SESSION" 2>/dev/null

# Create session with server pane
tmux new-session -d -s "$SESSION" -n "quic"

# Top pane: QUIC server
tmux send-keys -t "$SESSION" "cargo run --release --features='warp git2 tls-quic ring' --bin litebike -- quic-vqa 4433 2>&1 | tee /tmp/quic-server.log" Enter

# Wait for server to start and print the Chrome command
sleep 3

# Bottom pane: Chrome launcher (will extract SPKI from server output)
tmux split-window -v -t "$SESSION"
tmux send-keys -t "$SESSION" "echo '=== Waiting for SPKI from server output ===' && sleep 2 && grep -A1 'ignore-certificate-errors-spki-list' /tmp/quic-server.log | head -5 && echo '' && echo '=== Copy the Chrome command from the top pane and paste it here ===' && echo '=== Or run: ===' && echo 'SPKI=\$(grep \"SPKI hash\" /tmp/quic-server.log | awk \"{print \\$NF}\")' && echo '/Applications/Google\\ Chrome.app/Contents/MacOS/Google\\ Chrome --user-data-dir=/tmp/chrome-quic-profile --origin-to-force-quic-on=127.0.0.1:4433 --ignore-certificate-errors --ignore-certificate-errors-spki-list=\$SPKI --enable-quic https://127.0.0.1:4433'" Enter

# Select top pane
tmux select-pane -t "$SESSION":0.0

echo "Attach with: tmux attach -t $SESSION"

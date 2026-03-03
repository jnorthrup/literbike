#!/bin/bash
# Simple QUIC test runner without tmux

set -e

# Kill any existing quic-test tmux session
tmux kill-session -t quic-test 2>/dev/null || true

# Start QUIC server in background
echo "🚀 Starting QUIC server..."
cargo run --release --features='warp git2 tls-quic ring' --bin litebike -- quic-vqa 4433 &
SERVER_PID=$!

# Wait for server to compile and start
sleep 35

echo "⚙️  Server started (PID: $SERVER_PID)"

# Test Alt-Svc beacon (should work)
echo ""
echo "🔍 Testing HTTP Alt-Svc beacon..."
curl -s http://127.0.0.1:4433/ | grep "QUIC BOOTSTRAP" && echo "✅ Alt-Svc beacon working" || echo "❌ Alt-Svc beacon failed"

# Test with Chrome
echo ""
echo "🌐 Launching Chrome with QUIC forced..."
./test_chrome_quic.sh &

# Keep server running and show logs for 10 seconds
echo ""
echo "📊 Monitoring server for 10 seconds... (Ctrl+C to stop early)"
echo "Press Ctrl+C to kill all processes"
for i in {1..10}; do
    if ps -p $SERVER_PID > /dev/null; then
        echo "Second $i: Server running..."
        sleep 1
    else
        echo "❌ Server died unexpectedly"
        break
    fi
done

# Show what happened
echo ""
echo "📊 Final status:"
ps -p $SERVER_PID > /dev/null && echo "✅ Server still running" || echo "❌ Server stopped"

echo ""
echo "📋 Commands for further investigation:"
echo "   tmux new-session -d -s quic-test 'tail -f /tmp/quic-server.log' 2>/dev/null || true"
echo "   kill $SERVER_PID  # Stop server"
echo ""
echo "To manually attach and watch:"
echo "   tmux kill-session -t quic-test 2>/dev/null || true"
echo "   tmux new-session -s quic-test 'cargo run --release --features=\"warp git2 tls-quic ring\" --bin litebike -- quic-vqa 4433'"

# Cleanup if requested
echo ""
read -p "Press Enter to kill server, Ctrl+C to keep it running... "
kill $SERVER_PID 2>/dev/null || true
tmux kill-session -t quic-test 2>/dev/null || true
echo "Cleaned up."

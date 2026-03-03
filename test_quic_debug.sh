#!/bin/bash
# Minimal QUIC test script

echo "🔧 Testing QUIC server setup..."

# Kill existing servers
pkill -f "litebike quic-vqa" 2>/dev/null || true
pkill -f "quic_tls_server" 2>/dev/null || true
sleep 1

echo "🚀 Starting QUIC server in background..."
# Start server with debug output
RUST_LOG=debug cargo run --release --features='warp git2 tls-quic ring' --bin litebike -- quic-vqa 4433 > /tmp/quic-server.log 2>&1 &
SERVER_PID=$!
echo "Server PID: $SERVER_PID"

# Wait for compilation/startup
echo "⏳ Waiting for server to start (30 seconds)..."
sleep 30

# Check if server is running
if ! ps -p $SERVER_PID > /dev/null; then
    echo "❌ Server died. Logs:"
    cat /tmp/quic-server.log | tail -20
    exit 1
fi

echo "✅ Server is running"

# Test HTTP beacon (should work regardless of QUIC)
echo ""
echo "🔍 Testing HTTP Alt-Svc beacon..."
HTTP_RESPONSE=$(curl -s http://127.0.0.1:4433/ 2>/dev/null || echo "FAILED")
if echo "$HTTP_RESPONSE" | grep -q "QUIC BOOTSTRAP"; then
    echo "✅ HTTP beacon working - responds with Alt-Svc"
else
    echo "❌ HTTP beacon not responding"
fi

# Launch Chrome to test QUIC
echo ""
echo "🌐 Launching Chrome with QUIC enabled..."
./test_chrome_quic.sh &
CHROME_PID=$!

# Monitor server logs for 20 seconds
echo ""
echo "📊 Monitoring server logs for 20 seconds..."
echo "Looking for: TLS handshake completion, HTTP/3 SETTINGS, HTTP requests..."
sleep 10

# Check what happened
echo ""
echo "📋 Server log snippets:"
echo "--- TLS Handshake ---"
grep -A 2 -B 2 "process_crypto" /tmp/quic-server.log | tail -20 || echo "No crypto frames found"

echo ""
echo "--- HTTP/3 SETTINGS ---"
grep -E "HTTP/3|SETTINGS|Sent HANDSHAKE" /tmp/quic-server.log | tail -10 || echo "No HTTP/3 settings found"

echo ""
echo "--- HTTP requests ---"
grep "Server received request" /tmp/quic-server.log || echo "No HTTP requests received"

echo ""
echo "--- Connection closes ---"
grep "CONNECTION_CLOSE\|Connection close" /tmp/quic-server.log | tail -5 || echo "No connection closes found"

# Check for any errors
echo ""
echo "--- Errors ---"
grep -i "error\|failed\|panic" /tmp/quic-server.log | tail -10 || echo "No errors found"

echo ""
echo "📊 Summary:"
echo "- Server PID: $SERVER_PID"
echo "- Log file: /tmp/quic-server.log"
echo ""
echo "Commands:"
echo "  tail -f /tmp/quic-server.log           # Watch live logs"
echo "  kill $SERVER_PID                       # Stop server"
echo "  kill $CHROME_PID 2>/dev/null || true   # Stop Chrome"
echo ""
echo "Press Enter to cleanup and exit, Ctrl+C to keep processes running..."
read

# Cleanup
kill $SERVER_PID 2>/dev/null || true
kill $CHROME_PID 2>/dev/null || true
echo "Cleaned up."

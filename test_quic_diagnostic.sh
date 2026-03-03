#!/bin/bash
# Diagnostic QUIC test with Chrome

PORT=${1:-4433}
PROFILE_DIR="/tmp/chrome-quic-diagnostic-$(date +%s)"
mkdir -p "$PROFILE_DIR"

echo "=== QUIC Chrome Diagnostic Test ==="
echo "Profile: $PROFILE_DIR"
echo ""

# Check if server is running
if ! lsof -i :$PORT > /dev/null 2>&1; then
    echo "❌ Server not running on port $PORT"
    echo "Start server with: cargo run --release --features='warp git2 tls-quic ring' --bin litebike -- quic-vqa $PORT"
    exit 1
fi

echo "✅ Server running on port $PORT"
echo ""

# Check certificate
echo "=== Certificate Info ==="
openssl x509 -in /dev/null -text 2>/dev/null || echo "OpenSSL not available for cert inspection"
echo ""

# Launch Chrome with verbose QUIC logging
echo "=== Launching Chrome ==="
open -na "Google Chrome" --args \
    --user-data-dir="$PROFILE_DIR" \
    --origin-to-force-quic-on="127.0.0.1:$PORT" \
    --ignore-certificate-errors \
    --ignore-urlfetcher-cert-requests \
    --enable-quic \
    --quic-connection-options="COPA" \
    --enable-logging=stderr \
    --v=2 \
    --allow-insecure-localhost \
    "https://127.0.0.1:$PORT/index.html"

CHROME_PID=$!
echo "Chrome PID: $CHROME_PID"
echo ""

# Wait for Chrome to attempt connection
echo "Waiting 10 seconds for connection attempt..."
sleep 10

echo ""
echo "=== Check Chrome's QUIC status ==="
echo "Open chrome://net-internals/#quic in another Chrome window"
echo ""

# Check for error messages in logs
echo "=== Looking for Chrome errors ==="
ls -la "$PROFILE_DIR" 2>/dev/null | head -10

echo ""
echo "Profile directory: $PROFILE_DIR"
echo "To inspect:"
echo "  1. Open chrome://net-internals/#quic"
echo "  2. Check chrome://net-internals/#events with QUIC filter"
echo ""

# Capture server log excerpt during connection attempt
echo "=== Server log excerpt (if tmux is running) ==="
tmux capture-pane -t quic-1:test.0 -p 2>/dev/null | tail -30 || echo "Tmux session not available"

echo ""
echo "Test complete. Press Enter to cleanup..."
read

# Cleanup
rm -rf "$PROFILE_DIR"
echo "Cleaned up $PROFILE_DIR"

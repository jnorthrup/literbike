#!/bin/bash
# Launch Chrome with QUIC debugging and network logging

PORT=${1:-4433}
PROFILE_DIR="/tmp/chrome-quic-debug-$(date +%s)"
mkdir -p "$PROFILE_DIR"

# Net log file
NET_LOG="$PROFILE_DIR/net-log.json"

echo "=== Chrome QUIC Debug Launcher ==="
echo "Profile: $PROFILE_DIR"
echo "Port: $PORT"
echo "Net Log: $NET_LOG"
echo ""

# Launch Chrome with full QUIC debugging
open -na "Google Chrome" --args \
    --user-data-dir="$PROFILE_DIR" \
    --origin-to-force-quic-on="127.0.0.1:$PORT" \
    --ignore-certificate-errors \
    --enable-quic \
    --quic-connection-options="COPA" \
    --enable-logging=stderr \
    --v=1 \
    --log-net-log="$NET_LOG" \
    --net-log-capture-mode="Everything" \
    "https://127.0.0.1:$PORT/index.html"

# Alternative: Use chrome://net-export/ to capture logs
# open -na "Google Chrome" --args \
#     --user-data-dir="$PROFILE_DIR" \
#     --origin-to-force-quic-on="127.0.0.1:$PORT" \
#     --ignore-certificate-errors \
#     --enable-quic \
#     "chrome://net-export/"

echo ""
echo "Chrome launched. To view logs:"
echo "  1. Open chrome://net-internals/#quic in another Chrome window"
echo "  2. Or wait and check: $NET_LOG"
echo ""
echo "To capture with net-export:"
echo "  1. Open chrome://net-export/"
echo "  2. Click 'Start logging to disk'"
echo "  3. Navigate to https://127.0.0.1:$PORT/index.html"
echo ""
echo "Press Ctrl+C to cleanup..."

sleep 30

echo ""
echo "=== Checking for errors ==="
sleep 5

echo "--- Console log (if any) ---"
ls -la "$PROFILE_DIR/" 2>/dev/null | grep -E "log|error" || echo "No log files found"

echo ""
echo "--- Net log (first 100 lines) ---"
if [ -f "$NET_LOG" ]; then
    head -100 "$NET_LOG"
else
    echo "Net log not created yet or Chrome didn't write it"
fi

echo ""
echo "To analyze net log:"
echo "  cat $NET_LOG | python3 -m json.tool 2>/dev/null | head -50"
echo ""
echo "Profile dir: $PROFILE_DIR"

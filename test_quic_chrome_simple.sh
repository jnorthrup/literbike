#!/bin/bash
# Simple QUIC smoke test with Chrome

set -e

SERVER_PID=""
CHROME_PID=""

cleanup() {
    echo "Cleaning up..."
    [[ -n "$SERVER_PID" ]] && kill $SERVER_PID 2>/dev/null || true
    [[ -n "$CHROME_PID" ]] && kill $CHROME_PID 2>/dev/null || true
}
trap cleanup EXIT

echo "Building and starting QUIC server..."
cargo build --release --features='warp git2 tls-quic ring' --bin litebike 2>&1 | tail -5 &
BUILD_PID=$!
wait $BUILD_PID

echo "Starting server on port 4433..."
./target/release/litebike quic-vqa 4433 &
SERVER_PID=$!
sleep 5

echo "Testing HTTP endpoint..."
curl -v http://127.0.0.1:4433/ 2>&1 | head -20 || true

echo "Starting Chrome with QUIC..."
open -na "Google Chrome" --args \
    --user-data-dir="/tmp/chrome-quic-test-$(date +%s)" \
    --origin-to-force-quic-on="127.0.0.1:4433" \
    --ignore-certificate-errors \
    --enable-quic \
    "https://127.0.0.1:4433/index.html" &
CHROME_PID=$!

echo "Chrome launched (PID: $CHROME_PID)"
echo "Press Ctrl+C to stop test"
sleep 30

#!/bin/bash
# QUIC Server H2/H3 Test Script using curl
#
# This script tests the QUIC server with curl's HTTP/2 and HTTP/3 support.
# Requires: curl with HTTP/3 support (brew install curl --with-http3)
#
# Usage:
#   ./test_quic_curl.sh [server_url]
#
# Examples:
#   ./test_quic_curl.sh https://localhost:4433
#   ./test_quic_curl.sh https://127.0.0.1:8443

SERVER_URL="${1:-https://localhost:4433}"
OUTPUT_DIR="${2:-./curl_test_output}"

echo "🚀 QUIC Server H2/H3 Test Script"
echo "================================"
echo "Server: $SERVER_URL"
echo "Output: $OUTPUT_DIR"
echo ""

# Create output directory
mkdir -p "$OUTPUT_DIR"

# Check curl version and features
echo "📋 Checking curl capabilities..."
CURL_VERSION=$(curl --version | head -1)
echo "Curl: $CURL_VERSION"

# Check for HTTP/3 support
if curl --version | grep -q "HTTP3"; then
    echo "✅ HTTP/3 support detected"
    HTTP3_FLAG="--http3"
else
    echo "⚠️  HTTP/3 not available, using HTTP/2"
    HTTP3_FLAG="--http2"
fi

echo ""
echo "🧪 Running tests..."
echo ""

# Test 1: Fetch index.html with HTTP/3
echo "📄 Test 1: GET / (index.html)"
curl -k -s -o "$OUTPUT_DIR/index.html" \
    --connect-timeout 5 \
    $HTTP3_FLAG \
    -w "Status: %{http_code}\nTime: %{time_total}s\nSize: %{size_download} bytes\n" \
    "$SERVER_URL/"
echo ""

# Test 2: Fetch index.css
echo "🎨 Test 2: GET /index.css"
curl -k -s -o "$OUTPUT_DIR/index.css" \
    --connect-timeout 5 \
    $HTTP3_FLAG \
    -w "Status: %{http_code}\nTime: %{time_total}s\nSize: %{size_download} bytes\n" \
    "$SERVER_URL/index.css"
echo ""

# Test 3: Fetch bw_test_pattern.png
echo "🖼️  Test 3: GET /bw_test_pattern.png"
curl -k -s -o "$OUTPUT_DIR/bw_test_pattern.png" \
    --connect-timeout 10 \
    $HTTP3_FLAG \
    -w "Status: %{http_code}\nTime: %{time_total}s\nSize: %{size_download} bytes\n" \
    "$SERVER_URL/bw_test_pattern.png"
echo ""

# Test 4: Verbose output to see protocol negotiation
echo "🔍 Test 4: Protocol negotiation (verbose)"
curl -k -s -I \
    --connect-timeout 5 \
    $HTTP3_FLAG \
    -v 2>&1 | grep -E "(ALPN|HTTP/|SSL)" | head -10
echo ""

# Test 5: HTTP/2 comparison
echo "📊 Test 5: HTTP/2 comparison"
curl -k -s -o /dev/null \
    --connect-timeout 5 \
    --http2 \
    -w "HTTP/2 Status: %{http_code}\nTime: %{time_total}s\n" \
    "$SERVER_URL/"
echo ""

# Test 6: HTTP/3 if available
if [ "$HTTP3_FLAG" = "--http3" ]; then
    echo "📊 Test 6: HTTP/3 performance"
    curl -k -s -o /dev/null \
        --connect-timeout 5 \
        --http3 \
        -w "HTTP/3 Status: %{http_code}\nTime: %{time_total}s\n" \
        "$SERVER_URL/"
    echo ""
fi

# Summary
echo "📁 Output directory: $OUTPUT_DIR"
echo ""
echo "Files downloaded:"
ls -lh "$OUTPUT_DIR" | tail -n +2
echo ""

# Verify files
echo "✅ Verification:"
if [ -f "$OUTPUT_DIR/index.html" ] && [ -s "$OUTPUT_DIR/index.html" ]; then
    echo "  ✅ index.html: $(wc -c < "$OUTPUT_DIR/index.html") bytes"
else
    echo "  ❌ index.html: missing or empty"
fi

if [ -f "$OUTPUT_DIR/index.css" ] && [ -s "$OUTPUT_DIR/index.css" ]; then
    echo "  ✅ index.css: $(wc -c < "$OUTPUT_DIR/index.css") bytes"
else
    echo "  ❌ index.css: missing or empty"
fi

if [ -f "$OUTPUT_DIR/bw_test_pattern.png" ] && [ -s "$OUTPUT_DIR/bw_test_pattern.png" ]; then
    echo "  ✅ bw_test_pattern.png: $(wc -c < "$OUTPUT_DIR/bw_test_pattern.png") bytes"
else
    echo "  ❌ bw_test_pattern.png: missing or empty"
fi

echo ""
echo "🎉 Test complete!"

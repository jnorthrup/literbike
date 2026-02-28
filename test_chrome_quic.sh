#!/bin/bash
# Launches Google Chrome forcing QUIC to the local QA server on port 4433

# Use a temporary fresh profile so existing Chrome windows don't interfere
TEST_PROFILE="/tmp/chrome-quic-test-profile"
mkdir -p "$TEST_PROFILE"

echo "Launching Google Chrome with QUIC forced on for 127.0.0.1:4433..."
echo "Using temporary profile: $TEST_PROFILE"
echo "Target URL: https://127.0.0.1:4433/index.html"

# Use the idiomatic macOS 'open' command to launch a new instance (-n) with arguments
open -na "Google Chrome" --args \
    --user-data-dir="$TEST_PROFILE" \
    --origin-to-force-quic-on="127.0.0.1:4433" \
    --ignore-certificate-errors \
    --enable-quic \
    "https://127.0.0.1:4433/index.html"

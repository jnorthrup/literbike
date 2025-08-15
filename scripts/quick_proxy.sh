#!/bin/bash
# Quick proxy configuration for port 8888
# Handles both localhost and remote routing

PROXY_HOST="${1:-127.0.0.1}"
PROXY_PORT="${2:-8888}"

echo "🔧 Configuring system proxy: $PROXY_HOST:$PROXY_PORT"

# macOS system proxy settings
networksetup -setwebproxy "Wi-Fi" "$PROXY_HOST" "$PROXY_PORT"
networksetup -setsecurewebproxy "Wi-Fi" "$PROXY_HOST" "$PROXY_PORT" 
networksetup -setsocksfirewallproxy "Wi-Fi" "$PROXY_HOST" "$PROXY_PORT"

# Environment variables for current session
export http_proxy="http://$PROXY_HOST:$PROXY_PORT"
export https_proxy="http://$PROXY_HOST:$PROXY_PORT"
export all_proxy="socks5://$PROXY_HOST:$PROXY_PORT"

echo "✅ Proxy configured for $PROXY_HOST:$PROXY_PORT"
echo "💡 Usage: ./quick_proxy.sh [host] [port]"
echo "   Local:  ./quick_proxy.sh"
echo "   Remote: ./quick_proxy.sh 192.168.1.100"
#\!/bin/bash
echo "=== Protocol Dependencies Size Analysis ==="
echo ""

# Check individual dependency contributions
echo "Major protocol dependencies:"
echo ""

# Core networking
echo "1. Core Async/Networking:"
cargo tree -p litebike -e normal | grep -E "tokio|mio|socket2|bytes" | grep -v "├" | sort -u | head -10

echo ""
echo "2. DNS/DoH (trust-dns):"
cargo tree -p litebike -e normal | grep -E "trust-dns|resolv|idna" | grep -v "├" | sort -u | head -10

echo ""
echo "3. UPnP (igd-next):"
cargo tree -p litebike -e normal | grep -E "igd-next|xml|attohttpc" | grep -v "├" | sort -u | head -10

echo ""
echo "4. Network Interface (pnet):"
cargo tree -p litebike -e normal | grep -E "pnet" | grep -v "├" | sort -u | head -10

echo ""
echo "5. TLS/Crypto:"
cargo tree -p litebike -e normal | grep -E "rustls|ring|webpki" | grep -v "├" | sort -u | head -10

echo ""
echo "=== Dependency Count by Category ==="
echo "Total dependencies: $(cargo tree -p litebike -e normal | wc -l)"
echo "Direct dependencies: $(cargo tree -p litebike -e normal --depth 1 | grep -v "litebike" | wc -l)"

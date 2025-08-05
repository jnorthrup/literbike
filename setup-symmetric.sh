#!/bin/bash
# Setup script for LiteBike Symmetric Proxy System
# Creates network utilities and symmetric client

set -e

echo "Building LiteBike Symmetric Proxy System..."

# Build all binaries
echo "Building binaries..."
cargo build --release --bin netutils
cargo build --release --bin litebike_client
cargo build --release  # Main litebike server

# Check if binaries were built successfully
BINDIR="target/release"
if [ ! -f "$BINDIR/netutils" ]; then
    echo "Error: netutils binary not found"
    exit 1
fi

if [ ! -f "$BINDIR/litebike_client" ]; then
    echo "Error: litebike_client binary not found"
    exit 1
fi

if [ ! -f "$BINDIR/litebike" ]; then
    echo "Error: litebike server binary not found"
    exit 1
fi

# Create symlinks for netutils
echo "Creating network utility symlinks..."
cd "$BINDIR"
ln -sf netutils ifconfig
ln -sf netutils netstat
ln -sf netutils route
ln -sf netutils ip

echo ""
echo "=== LiteBike Symmetric Proxy System Ready ==="
echo ""
echo "Server Mode:"
echo "  ./litebike                    # Start proxy server on port 8888"
echo ""
echo "Client Mode:"
echo "  ./litebike_client /client     # Discover and connect to server"
echo ""
echo "Network Utilities (cross-platform syscall-based):"
echo "  ./ifconfig                    # Show network interfaces"
echo "  ./netstat                     # Show network connections"
echo "  ./route                       # Show routing table"
echo "  ./ip addr                     # Show IP addresses"
echo "  ./ip route                    # Show routes"
echo ""
echo "Usage Examples:"
echo ""
echo "# Start server on Android/Termux:"
echo "export LITEBIKE_INTERFACE=rmnet0"
echo "./litebike"
echo ""
echo "# Connect from macOS client:"
echo "./litebike_client /client"
echo ""
echo "# Or start server on macOS:"
echo "export LITEBIKE_INTERFACE=en0"
echo "./litebike"
echo ""
echo "Key Features:"
echo "- Pure syscall-based network utilities (no /proc, /sys, /dev access)"
echo "- Cross-platform Android/Termux and macOS support"
echo "- Automatic network discovery via default route to 8.8.8.8"
echo "- HTTP-based REPL for remote command execution"
echo "- Symmetric litebike-to-litebike communication"
echo "- rmnet interface detection for Android mobile data"
echo ""
echo "Add $(pwd) to your PATH to use utilities from anywhere:"
echo "export PATH=\"$(pwd):\$PATH\""
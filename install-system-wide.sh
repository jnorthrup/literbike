#!/bin/bash
# System-wide installation script for LiteBike

set -e

INSTALL_DIR="/usr/local/bin"
BUILD_DIR="target/release"

if [ "$EUID" -ne 0 ]; then
    echo "This script must be run as root for system-wide installation"
    echo "Usage: sudo ./install-system-wide.sh"
    exit 1
fi

echo "Installing LiteBike to $INSTALL_DIR..."

# Copy main binaries
cp "$BUILD_DIR/litebike" "$INSTALL_DIR/"
cp "$BUILD_DIR/netutils" "$INSTALL_DIR/"
cp "$BUILD_DIR/litebike_client" "$INSTALL_DIR/"

# Create legacy symlinks
cd "$INSTALL_DIR"
ln -sf litebike ifconfig
ln -sf litebike netstat
ln -sf litebike route
ln -sf litebike ip

# Create modern symlinks  
ln -sf litebike litebike-net
ln -sf litebike litebike-proxy
ln -sf litebike litebike-connect
ln -sf litebike litebike-discover

echo "✅ System-wide installation complete"
echo "Run 'litebike --install-completions' to install bash completions"

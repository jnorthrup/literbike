#!/bin/bash
# Setup script to create symlinks for network utilities

# Build the netutils binary
cargo build --release --bin netutils

# Create symlinks
BINARY="target/release/netutils"
if [ -f "$BINARY" ]; then
    echo "Creating symlinks for network utilities..."
    ln -sf netutils target/release/ifconfig
    ln -sf netutils target/release/netstat
    ln -sf netutils target/release/route
    ln -sf netutils target/release/ip
    echo "Done! You can now use: ifconfig, netstat, route, and ip"
    echo "Add $(pwd)/target/release to your PATH to use them from anywhere"
else
    echo "Error: Binary not found. Please run 'cargo build --release --bin netutils' first"
fi
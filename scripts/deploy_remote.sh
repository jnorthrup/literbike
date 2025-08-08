#!/bin/bash

# This script automates the deployment of the LiteBike binary to a remote host.
# Prerequisites:
# 1. SSH access to the remote host without password prompt (e.g., using SSH keys).
# 2. Git installed on the remote host.
# 3. Rust toolchain installed on the remote host (if building remotely).
# 4. Replace placeholder values with your actual remote host details.

# --- Configuration --- START
REMOTE_USER="your_remote_user" # e.g., ubuntu, ec2-user
REMOTE_HOST="your_remote_host_ip_or_hostname" # e.g., 192.168.1.100, example.com
REMOTE_PATH="/opt/litebike" # Path on the remote host where LiteBike will be deployed

# Set TARGET_TRIPLE based on your remote host's architecture (e.g., x86_64-unknown-linux-gnu, aarch64-unknown-linux-gnu)
# If cross-compiling, ensure you have the target toolchain installed (e.g., `rustup target add aarch64-unknown-linux-gnu`)
TARGET_TRIPLE="$(rustc -VV | grep host | cut -d ' ' -f 2)" # Defaults to local host triple
# --- Configuration --- END

# --- Deployment Steps ---

echo "Building LiteBike for target: ${TARGET_TRIPLE}"
# Build the LiteBike binary for the target architecture
# Use --release for optimized binary
cargo build --release --target ${TARGET_TRIPLE}

if [ $? -ne 0 ]; then
    echo "Error: Cargo build failed. Exiting."
    exit 1
fi

LOCAL_BINARY_PATH="./target/${TARGET_TRIPLE}/release/litebike"

if [ ! -f "${LOCAL_BINARY_PATH}" ]; then
    echo "Error: Built binary not found at ${LOCAL_BINARY_PATH}. Exiting."
    exit 1
fi

echo "Creating remote directory ${REMOTE_PATH} on ${REMOTE_USER}@${REMOTE_HOST}"
ssh "${REMOTE_USER}@${REMOTE_HOST}" "mkdir -p ${REMOTE_PATH}"

if [ $? -ne 0 ]; then
    echo "Error: Failed to create remote directory. Exiting."
    exit 1
fi

echo "Copying LiteBike binary to ${REMOTE_USER}@${REMOTE_HOST}:${REMOTE_PATH}"
scp "${LOCAL_BINARY_PATH}" "${REMOTE_USER}@${REMOTE_HOST}:${REMOTE_PATH}/litebike"

if [ $? -ne 0 ]; then
    echo "Error: Failed to copy binary. Exiting."
    exit 1
fi

echo "Setting execute permissions on remote binary"
ssh "${REMOTE_USER}@${REMOTE_HOST}" "chmod +x ${REMOTE_PATH}/litebike"

if [ $? -ne 0 ]; then
    echo "Error: Failed to set execute permissions. Exiting."
    exit 1
fi

echo "LiteBike deployed successfully to ${REMOTE_USER}@${REMOTE_HOST}:${REMOTE_PATH}/litebike"

# Optional: Start or manage the service on the remote host
# echo "Starting LiteBike service on remote host..."
# ssh "${REMOTE_USER}@${REMOTE_HOST}" "sudo systemctl restart litebike || ${REMOTE_PATH}/litebike &"

echo "Deployment script finished."

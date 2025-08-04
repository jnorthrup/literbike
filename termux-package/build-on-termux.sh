#!/data/data/com.termux/files/usr/bin/bash
# Handy and concise build script for LiteBike on Termux.

set -e

# --- Configuration ---
INSTALL_NAME="litebike-proxy"
SOURCE_BIN_NAME="litebike"

# --- Helper Functions ---
info() {
    echo -e "\033[1;34m[INFO]\033[0m $1"
}
success() {
    echo -e "\033[1;32m[SUCCESS]\033[0m $1"
}
error() {
    echo -e "\033[1;31m[ERROR]\033[0m $1" >&2
    exit 1
}

# --- Main Script ---
info "Starting LiteBike build for Termux..."

# 1. Ensure Rust is installed
if ! command -v rustc &>/dev/null; then
    info "Rust not found. Installing..."
    pkg install rust -y
fi

# 2. Build and install using 'cargo install' for a cleaner process
info "Building and installing '${SOURCE_BIN_NAME}'..."
# Use --path . to build the current crate and --root to install into $PREFIX
# This places the binary at $PREFIX/bin/litebike
cargo install --path . --root "$PREFIX" --bin "${SOURCE_BIN_NAME}"

# 3. Rename the binary for convenience
INSTALLED_PATH="$PREFIX/bin/${SOURCE_BIN_NAME}"
if [ -f "${INSTALLED_PATH}" ]; then
    info "Renaming binary to '${INSTALL_NAME}'..."
    mv "${INSTALLED_PATH}" "$PREFIX/bin/${INSTALL_NAME}"
else
    error "Build failed. Could not find binary at ${INSTALLED_PATH}."
fi

# 4. Final success message and usage instructions
success "LiteBike installed as '${INSTALL_NAME}'!"
echo
echo "Usage:"
echo "  ${INSTALL_NAME}                    # Start proxy server"
echo "  BIND_IP=0.0.0.0 ${INSTALL_NAME}   # Bind to all interfaces"
echo
echo "Proxy endpoint (Universal Port):"
echo "  HTTP/HTTPS/SOCKS5: localhost:8888"

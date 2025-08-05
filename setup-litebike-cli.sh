#!/bin/bash
# Enhanced setup script for LiteBike CLI with comprehensive symlink management
# Creates symlinks for both legacy utilities and modern commands

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}=== LiteBike CLI Setup ===${NC}"

# Build all binaries
echo -e "${YELLOW}Building LiteBike binaries...${NC}"
cargo build --release --bin litebike
cargo build --release --bin netutils  
cargo build --release --bin litebike_client

# Check if binaries were built successfully
BINARY_DIR="target/release"
LITEBIKE_BINARY="$BINARY_DIR/litebike"
NETUTILS_BINARY="$BINARY_DIR/netutils"
CLIENT_BINARY="$BINARY_DIR/litebike_client"

if [ ! -f "$LITEBIKE_BINARY" ]; then
    echo -e "${RED}Error: litebike binary not found at $LITEBIKE_BINARY${NC}"
    exit 1
fi

if [ ! -f "$NETUTILS_BINARY" ]; then
    echo -e "${RED}Error: netutils binary not found at $NETUTILS_BINARY${NC}"
    exit 1
fi

echo -e "${GREEN}✅ All binaries built successfully${NC}"

# Create symlinks for legacy utilities that map to litebike
echo -e "${YELLOW}Creating legacy utility symlinks...${NC}"

# Legacy utilities -> litebike with smart routing
LEGACY_UTILS=("ifconfig" "netstat" "route" "ip")
for util in "${LEGACY_UTILS[@]}"; do
    ln -sf litebike "$BINARY_DIR/$util"
    echo -e "${GREEN}✅ Created: $BINARY_DIR/$util -> litebike${NC}"
done

# Create symlinks for netutils (direct compatibility)
echo -e "${YELLOW}Creating netutils compatibility symlinks...${NC}"
NETUTILS_SYMLINKS=("netutils-ifconfig" "netutils-netstat" "netutils-route" "netutils-ip")
for i in "${!NETUTILS_SYMLINKS[@]}"; do
    ln -sf netutils "$BINARY_DIR/${NETUTILS_SYMLINKS[$i]}"
    echo -e "${GREEN}✅ Created: $BINARY_DIR/${NETUTILS_SYMLINKS[$i]} -> netutils${NC}"
done

# Create modern litebike utility symlinks
echo -e "${YELLOW}Creating modern utility symlinks...${NC}"
MODERN_UTILS=(
    "litebike-net"
    "litebike-proxy" 
    "litebike-connect"
    "litebike-discover"
)
for util in "${MODERN_UTILS[@]}"; do
    ln -sf litebike "$BINARY_DIR/$util"
    echo -e "${GREEN}✅ Created: $BINARY_DIR/$util -> litebike${NC}"
done

# Install bash completions
echo -e "${YELLOW}Installing bash completions...${NC}"
if "$LITEBIKE_BINARY" --install-completions; then
    echo -e "${GREEN}✅ Bash completions installed${NC}"
else
    echo -e "${YELLOW}⚠️  Completion installation may have failed - check output above${NC}"
fi

# Test the setup
echo -e "${YELLOW}Testing setup...${NC}"

# Test litebike main command
if "$LITEBIKE_BINARY" --help >/dev/null 2>&1; then
    echo -e "${GREEN}✅ litebike command works${NC}"
else
    echo -e "${RED}❌ litebike command failed${NC}"
fi

# Test legacy symlinks
for util in "${LEGACY_UTILS[@]}"; do
    if "$BINARY_DIR/$util" --help >/dev/null 2>&1; then
        echo -e "${GREEN}✅ $util symlink works${NC}"
    else
        echo -e "${RED}❌ $util symlink failed${NC}"
    fi
done

# Generate strategy report
echo -e "${YELLOW}Generating execution strategy report...${NC}"
"$LITEBIKE_BINARY" --strategy-report

# Create installation script for system-wide installation
cat > install-system-wide.sh << 'EOF'
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
EOF

chmod +x install-system-wide.sh

# Create PATH export suggestion
echo
echo -e "${BLUE}=== Setup Complete ===${NC}"
echo
echo -e "${GREEN}To use the utilities, add this to your PATH:${NC}"
echo -e "${YELLOW}export PATH=\"$(pwd)/$BINARY_DIR:\$PATH\"${NC}"
echo
echo -e "${GREEN}Or add this line to your ~/.bashrc:${NC}"
echo -e "${YELLOW}export PATH=\"$(pwd)/$BINARY_DIR:\$PATH\"${NC}"
echo
echo -e "${GREEN}For system-wide installation, run:${NC}"
echo -e "${YELLOW}sudo ./install-system-wide.sh${NC}"
echo
echo -e "${GREEN}Available commands:${NC}"
echo -e "  ${BLUE}litebike${NC}           - Main CLI with Git-like interface"
echo -e "  ${BLUE}ifconfig${NC}           - Legacy ifconfig compatibility (via litebike)"
echo -e "  ${BLUE}netstat${NC}            - Legacy netstat compatibility (via litebike)"
echo -e "  ${BLUE}route${NC}              - Legacy route compatibility (via litebike)"
echo -e "  ${BLUE}ip${NC}                 - Legacy ip compatibility (via litebike)"
echo -e "  ${BLUE}netutils${NC}           - Direct syscall-based utilities"
echo -e "  ${BLUE}litebike_client${NC}    - Symmetric client for REPL connections"
echo
echo -e "${GREEN}Modern interface examples:${NC}"
echo -e "  ${YELLOW}litebike net interfaces list${NC}"
echo -e "  ${YELLOW}litebike net routes list${NC}"
echo -e "  ${YELLOW}litebike proxy server --port 8080${NC}"
echo -e "  ${YELLOW}litebike connect repl 192.168.1.1${NC}"
echo
echo -e "${GREEN}Legacy compatibility examples:${NC}"
echo -e "  ${YELLOW}ifconfig -a${NC}                # Maps to: litebike utils ifconfig --all"
echo -e "  ${YELLOW}netstat -tuln${NC}             # Maps to: litebike utils netstat --tcp --udp --listening"
echo -e "  ${YELLOW}route${NC}                      # Maps to: litebike utils route"
echo -e "  ${YELLOW}ip addr show${NC}               # Maps to: litebike utils ip addr show"
echo
echo -e "${BLUE}For network lockdown scenarios, try:${NC}"
echo -e "  ${YELLOW}litebike --test-reentrant${NC}     # Test multiple execution pathways"
echo -e "  ${YELLOW}litebike --strategy-report${NC}    # Show execution strategy analysis"
echo
echo -e "${GREEN}Restart your shell or run 'source ~/.bashrc' to activate completions${NC}"
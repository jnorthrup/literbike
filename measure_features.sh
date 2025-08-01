#\!/bin/bash
echo "=== Binary Size Analysis by Feature ==="
echo ""

# Build with different feature combinations
echo "Building minimal (no features)..."
cargo build --release --bin litebike-proxy --no-default-features --features minimal 2>/dev/null
MINIMAL_SIZE=$(ls -l target/release/litebike-proxy | awk '{print $5}')

echo "Building with just tokio..."
cargo clean 2>/dev/null
echo '[dependencies]
tokio = { version = "1", features = ["net", "io-util", "rt", "macros"] }
log = "0.4"
env_logger = "0.11"
libc = "0.2"' > Cargo-test.toml
cp Cargo.toml Cargo-backup.toml
cp Cargo-test.toml Cargo.toml
cargo build --release --bin litebike-proxy 2>/dev/null
TOKIO_SIZE=$(ls -l target/release/litebike-proxy | awk '{print $5}')

# Restore original
cp Cargo-backup.toml Cargo.toml

echo ""
echo "Size Analysis:"
echo "Minimal build: $(echo $MINIMAL_SIZE | numfmt --to=iec-i)B"
echo "Tokio only: $(echo $TOKIO_SIZE | numfmt --to=iec-i)B"
echo "Full build: $(ls -lh target/release/litebike-proxy | awk '{print $5}')"

# Estimate protocol overhead
echo ""
echo "Estimated protocol complexity overhead:"
FULL_SIZE=$(ls -l target/release/litebike-proxy | awk '{print $5}')
echo "DoH/DNS: ~$(( ($FULL_SIZE - $TOKIO_SIZE) / 3 / 1024 ))KB"
echo "UPnP: ~$(( ($FULL_SIZE - $TOKIO_SIZE) / 3 / 1024 ))KB"  
echo "pnet: ~$(( ($FULL_SIZE - $TOKIO_SIZE) / 3 / 1024 ))KB"

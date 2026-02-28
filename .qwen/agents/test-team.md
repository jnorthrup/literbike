# Qwen Agent: Testing Infrastructure Team

## Assignment
Build testing tools and infrastructure.

## Branches
- `test/p1-litecurl` - HTTP client binary
- `test/p1-ipfs-client` - IPFS client binary
- `test/p1-qwen-agents` - Qwen agent test configs

## Priority
**P1 - High** (Enables automated testing)

---

## Task 1: litecurl

**Branch:** `test/p1-litecurl`

**Purpose:** Lightweight curl alternative for testing Literbike services

**Implementation:**
```rust
// src/bin/litecurl.rs
use clap::Parser;
use reqwest;

#[derive(Parser)]
#[command(name = "litecurl")]
struct Cli {
    /// URL to fetch
    url: String,
    
    /// HTTP method
    #[arg(short, long, default_value = "GET")]
    method: String,
    
    /// Headers (Key: Value)
    #[arg(short, long)]
    header: Vec<String>,
    
    /// POST/PUT data
    #[arg(short, long)]
    data: Option<String>,
    
    /// SOCKS5 proxy
    #[arg(long)]
    proxy: Option<String>,
    
    /// Timeout seconds
    #[arg(short, long, default_value = "30")]
    timeout: u64,
    
    /// Verbose output
    #[arg(short, long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    // Implement HTTP client
}
```

**Test:**
```bash
cargo build --bin litecurl
./target/debug/litecurl http://localhost:5984/_stats
```

---

## Task 2: ipfs_client

**Branch:** `test/p1-ipfs-client`

**Purpose:** Standalone IPFS test client

**Implementation:**
```rust
// src/bin/ipfs_client.rs
use clap::{Parser, Subcommand};
use ipfs_api_backend_hyper::{IpfsApi, IpfsClient};

#[derive(Parser)]
struct Cli {
    #[arg(short, long, default_value = "http://127.0.0.1:5001")]
    api_url: String,
    
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Add file to IPFS
    Add { path: String },
    /// Get file from IPFS
    Get { cid: String },
    /// Pin content
    Pin { cid: String },
    /// List pinned content
    Ls,
    /// Show stats
    Stats,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let client = IpfsClient::from_str(&cli.api_url)?;
    
    match cli.command {
        Commands::Add { path } => {
            // Add file
        }
        Commands::Get { cid } => {
            // Get file
        }
        // etc.
    }
}
```

**Test:**
```bash
cargo build --bin ipfs_client
./target/debug/ipfs_client add test.txt
./target/debug/ipfs_client get QmXxx...
```

---

## Task 3: Qwen Agent Configs

**Branch:** `test/p1-qwen-agents`

**Create agent configs:**
- `.qwen/agents/literbike-tester.md`
- `.qwen/agents/http-tester.md`

**literbike-tester.md:**
```markdown
# Literbike Test Agent

## Responsibilities
- Run smoke tests for new features
- Verify QUIC connectivity
- Test proxy configurations
- Validate IPFS operations

## Test Context
```bash
export LITERBIKE_TEST_DIR=/tmp/literbike-test-$$
mkdir -p $LITERBIKE_TEST_DIR

# Run tests
cargo test --features quic <test_name>

# Cleanup
rm -rf $LITERBIKE_TEST_DIR
```
```

**http-tester.md:**
```markdown
# HTTP Testing Agent

## Responsibilities
- Test litecurl against all HTTP endpoints
- Verify API server responses
- Test proxy chains

## Test Endpoints
- `http://localhost:5984/_stats`
- `http://localhost:8080/api/v1/health`
- `http://localhost:1234/test`
```

---

## Success Criteria

- [ ] litecurl can fetch HTTP endpoints
- [ ] litecurl supports SOCKS5 proxy
- [ ] ipfs_client can add/get files
- [ ] ipfs_client can pin/unpin
- [ ] Qwen agent configs are complete
- [ ] All binaries compile without warnings

---

## Merge Order

All branches can merge independently (no dependencies between them).

---

## Dependencies

- reqwest crate for litecurl
- ipfs_api_backend_hyper for ipfs_client
- clap for CLI parsing

---

**Created:** 2026-02-24  
**Status:** Ready to start

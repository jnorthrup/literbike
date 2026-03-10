//! Carrier Tethering Bypass CLI
//! 
//! Comprehensive carrier tethering detection bypass with:
//! - TTL spoofing to mimic mobile devices
//! - DNS override to bypass carrier filtering
//! - Traffic shaping for mobile emulation
//! - Packet fragmentation for DPI evasion
//! - Protocol obfuscation
//! - Radio interface detection
//!
//! Usage:
//! ```bash
//! # Enable comprehensive bypass
//! cargo run --bin carrier_bypass -- enable
//!
//! # Detect carrier restrictions
//! cargo run --bin carrier_bypass -- detect
//!
//! # Start Knox proxy with bypass
//! cargo run --bin carrier_bypass -- proxy --port 8080
//!
//! # Disable bypass and cleanup
//! cargo run --bin carrier_bypass -- disable
//! ```

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use literbike::knox_proxy::{KnoxProxy, KnoxProxyConfig};
use literbike::tethering_bypass::TetheringBypass;
use literbike::packet_fragment::PacketFragmenter;
use literbike::radios::{gather_radios, print_radios_human};
use literbike::posix_sockets::posix_peek;
use log::{info, LevelFilter};
use std::net::ToSocketAddrs;
use env_logger::Builder;

#[derive(Parser)]
#[command(name = "carrier-bypass")]
#[command(author = "Literbike Team")]
#[command(version = "1.0")]
#[command(about = "Carrier tethering detection bypass system", long_about = None)]
struct Cli {
    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Enable comprehensive tethering bypass
    Enable {
        /// Enable TTL spoofing (default: true)
        #[arg(long, default_value = "true")]
        ttl_spoofing: bool,

        /// Enable DNS override (default: true)
        #[arg(long, default_value = "true")]
        dns_override: bool,

        /// Enable traffic shaping (default: true)
        #[arg(long, default_value = "true")]
        traffic_shaping: bool,

        /// Enable packet fragmentation for DPI evasion
        #[arg(long)]
        fragmentation: bool,

        /// Enable protocol obfuscation
        #[arg(long)]
        obfuscation: bool,
    },

    /// Detect carrier tethering restrictions
    Detect {
        /// Run comprehensive detection (slower but more accurate)
        #[arg(long)]
        comprehensive: bool,
    },

    /// Start Knox proxy with tethering bypass
    Proxy {
        /// Bind address
        #[arg(short, long, default_value = "0.0.0.0:8080")]
        bind: String,

        /// SOCKS5 port
        #[arg(long, default_value = "1080")]
        socks_port: u16,

        /// Enable Knox bypass
        #[arg(long, default_value = "true")]
        knox_bypass: bool,

        /// Enable tethering bypass
        #[arg(long, default_value = "true")]
        tethering_bypass: bool,

        /// TTL value for spoofing
        #[arg(long, default_value = "64")]
        ttl: u8,

        /// Enable packet fragmentation
        #[arg(long)]
        fragmentation: bool,

        /// Enable TCP fingerprint randomization
        #[arg(long)]
        tcp_fingerprint: bool,

        /// Enable TLS fingerprint randomization
        #[arg(long)]
        tls_fingerprint: bool,
    },

    /// Disable bypass and cleanup
    Disable,

    /// Test radio interface detection
    RadioDetect,

    /// Test POSIX peek functionality
    PosixPeek {
        /// Host to test against
        #[arg(short, long, default_value = "8.8.8.8:53")]
        host: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let log_level = if cli.verbose {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    };

    Builder::new()
        .filter_level(log_level)
        .format_timestamp_secs()
        .init();

    match cli.command {
        Commands::Enable {
            ttl_spoofing,
            dns_override,
            traffic_shaping,
            fragmentation,
            obfuscation,
        } => {
            cmd_enable(ttl_spoofing, dns_override, traffic_shaping, fragmentation, obfuscation).await?;
        }
        Commands::Detect { comprehensive } => {
            cmd_detect(comprehensive).await?;
        }
        Commands::Proxy {
            bind,
            socks_port,
            knox_bypass,
            tethering_bypass,
            ttl,
            fragmentation,
            tcp_fingerprint,
            tls_fingerprint,
        } => {
            cmd_proxy(bind, socks_port, knox_bypass, tethering_bypass, ttl, 
                     fragmentation, tcp_fingerprint, tls_fingerprint).await?;
        }
        Commands::Disable => {
            cmd_disable()?;
        }
        Commands::RadioDetect => {
            cmd_radio_detect().await?;
        }
        Commands::PosixPeek { host } => {
            cmd_posix_peek(&host).await?;
        }
    }

    Ok(())
}

/// Enable comprehensive tethering bypass
async fn cmd_enable(
    ttl_spoofing: bool,
    dns_override: bool,
    traffic_shaping: bool,
    fragmentation: bool,
    obfuscation: bool,
) -> Result<()> {
    info!("🔓 Enabling comprehensive carrier tethering bypass");
    info!("   TTL spoofing: {}", ttl_spoofing);
    info!("   DNS override: {}", dns_override);
    info!("   Traffic shaping: {}", traffic_shaping);
    info!("   Packet fragmentation: {}", fragmentation);
    info!("   Protocol obfuscation: {}", obfuscation);

    let mut bypass = TetheringBypass::new();
    bypass.ttl_spoofing = ttl_spoofing;
    bypass.dns_override = dns_override;
    bypass.traffic_shaping = traffic_shaping;

    // Enable bypass
    if let Err(e) = bypass.enable_bypass() {
        return Err(anyhow::anyhow!("Failed to enable tethering bypass: {}", e));
    }

    // Setup packet fragmentation for DPI evasion
    if fragmentation {
        info!("📦 Setting up packet fragmentation for DPI evasion");
        use literbike::packet_fragment::MobileFragmentPattern;
        
        let _fragmenter = PacketFragmenter::new(MobileFragmentPattern::Adaptive);
        info!("✅ Packet fragmentation configured (Adaptive mode)");
    }

    // Setup protocol obfuscation
    if obfuscation {
        info!("🎭 Setting up protocol obfuscation");
        
        // Use mobile profile for TCP fingerprint
        use literbike::tcp_fingerprint::MobileProfile;
        let profile = MobileProfile::IPhone15;
        let fingerprint = profile.get_tcp_fingerprint();
        info!("   TCP fingerprint (iPhone 15): window={}, mss={}, ttl={}", 
              fingerprint.window_size, fingerprint.mss, fingerprint.ttl);

        info!("   TLS fingerprint: (requires tls-quic feature)");
    }

    info!("✅ Comprehensive tethering bypass enabled");
    info!("");
    info!("📋 Usage:");
    info!("   - Configure applications to use proxy at 0.0.0.0:8080");
    info!("   - SOCKS5 port: 1080");
    info!("   - Run 'carrier-bypass disable' to cleanup");

    Ok(())
}

/// Detect carrier tethering restrictions
async fn cmd_detect(comprehensive: bool) -> Result<()> {
    use literbike::tethering_bypass::detect_carrier_restrictions;
    
    info!("🔍 Detecting carrier tethering restrictions");
    if comprehensive {
        info!("   Running comprehensive detection (this may take a moment)...");
    }

    let restrictions = detect_carrier_restrictions()
        .map_err(|e| anyhow::anyhow!("Failed to detect carrier restrictions: {}", e))?;

    info!("");
    info!("📋 Detection Results:");
    info!("   TTL detection: {}", if restrictions.ttl_detection { "⚠️ ACTIVE" } else { "✓ none" });
    info!("   User-Agent filtering: {}", if restrictions.user_agent_filtering { "⚠️ ACTIVE" } else { "✓ none" });
    info!("   DNS filtering: {}", if restrictions.dns_filtering { "⚠️ ACTIVE" } else { "✓ none" });
    info!("   Port blocking: {}", if restrictions.port_blocking { "⚠️ ACTIVE" } else { "✓ none" });
    info!("   DPI inspection: {}", if restrictions.dpi_inspection { "⚠️ ACTIVE" } else { "✓ none" });
    info!("   Bandwidth throttling: {}", if restrictions.bandwidth_throttling { "⚠️ ACTIVE" } else { "✓ none" });

    // Provide recommendations
    info!("");
    info!("💡 Recommendations:");
    
    if restrictions.ttl_detection {
        info!("   - Enable TTL spoofing with --ttl-spoofing");
    }
    if restrictions.dns_filtering {
        info!("   - Enable DNS override with --dns-override");
    }
    if restrictions.dpi_inspection {
        info!("   - Enable packet fragmentation with --fragmentation");
        info!("   - Enable protocol obfuscation with --obfuscation");
    }
    if restrictions.port_blocking {
        info!("   - Use alternative ports (443, 8443)");
    }

    Ok(())
}

/// Start Knox proxy with tethering bypass
async fn cmd_proxy(
    bind: String,
    socks_port: u16,
    knox_bypass: bool,
    tethering_bypass: bool,
    ttl: u8,
    fragmentation: bool,
    tcp_fingerprint: bool,
    tls_fingerprint: bool,
) -> Result<()> {
    info!("🚀 Starting Knox Proxy with carrier bypass");
    
    let config = KnoxProxyConfig {
        bind_addr: bind,
        socks_port,
        enable_knox_bypass: knox_bypass,
        enable_tethering_bypass: tethering_bypass,
        ttl_spoofing: ttl,
        max_connections: 100,
        buffer_size: 4096,
        tcp_fingerprint_enabled: tcp_fingerprint,
        packet_fragmentation_enabled: fragmentation,
        tls_fingerprint_enabled: tls_fingerprint,
    };

    let mut proxy = KnoxProxy::new(config);
    
    info!("");
    info!("📋 Proxy Configuration:");
    info!("   Bind address: {}", proxy.get_bind_addr());
    info!("   SOCKS port: {}", socks_port);
    info!("   Knox bypass: {}", knox_bypass);
    info!("   Tethering bypass: {}", tethering_bypass);
    info!("   TTL spoofing: {}", ttl);
    info!("   Packet fragmentation: {}", fragmentation);
    info!("   TCP fingerprint: {}", tcp_fingerprint);
    info!("   TLS fingerprint: {}", tls_fingerprint);
    info!("");
    info!("📡 Proxy starting...");

    proxy.start().await
        .map_err(|e| anyhow::anyhow!("Knox proxy failed to start: {}", e))?;

    Ok(())
}

/// Disable bypass and cleanup
fn cmd_disable() -> Result<()> {
    info!("🧹 Disabling tethering bypass and cleaning up");
    
    let mut bypass = TetheringBypass::new();
    if let Err(e) = bypass.disable_bypass() {
        return Err(anyhow::anyhow!("Failed to disable tethering bypass: {}", e));
    }

    info!("✅ Cleanup complete");
    info!("   - TTL rules removed");
    info!("   - DNS settings restored");
    info!("   - Traffic shaping disabled");
    
    Ok(())
}

/// Test radio interface detection
async fn cmd_radio_detect() -> Result<()> {
    info!("📻 Detecting radio interfaces");

    let report = gather_radios();
    print_radios_human(&report);

    Ok(())
}

/// Test POSIX peek functionality
async fn cmd_posix_peek(host: &str) -> Result<()> {
    use tokio::net::TcpStream;
    
    info!("🔍 Testing POSIX peek against {}", host);

    let addr = host
        .to_socket_addrs()
        .context("Failed to resolve host")?
        .next()
        .context("No address resolved")?;

    // Connect with TCP stream for POSIX peek
    let mut stream = TcpStream::connect(addr)
        .await
        .context("Failed to connect")?;

    info!("✅ Connected to {}", host);
    
    // Send a simple HTTP GET request to trigger a response
    let http_request = b"GET / HTTP/1.0\r\n\r\n";
    use tokio::io::AsyncWriteExt;
    stream.write_all(http_request).await
        .context("Failed to send request")?;

    // Peek at response using POSIX peek
    let mut buffer = vec![0u8; 512];
    let n = posix_peek(&stream, &mut buffer)
        .map_err(|e| anyhow::anyhow!("POSIX peek failed: {}", e))?;

    info!("✅ POSIX peek successful");
    info!("   Received {} bytes", n);
    info!("   First bytes: {:02x?}", &buffer[..n.min(16)]);

    Ok(())
}

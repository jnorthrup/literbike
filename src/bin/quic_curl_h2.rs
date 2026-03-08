//! QUIC Server HTTP/2 Test Client using curl-h2
//!
//! This binary tests the QUIC server's HTTP/2 (H2) capabilities using curl with HTTP/2 support.
//! It fetches the UI test pattern assets (index.html, index.css, bw_test_pattern.png) and verifies
//! they are served correctly over HTTP/2.
//!
//! # Usage
//!
//! ```bash
//! # Run with default settings (localhost:4433)
//! cargo run --bin quic_curl_h2 --features curl-h2
//!
//! # Run with custom URL
//! cargo run --bin quic_curl_h2 --features curl-h2 -- --url https://localhost:4433
//!
//! # Run with verbose output
//! cargo run --bin quic_curl_h2 --features curl-h2 -- --verbose
//! ```

use clap::Parser;
use literbike::curl_h2::{H2Client, H2Error, H2Request};
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Parser, Debug)]
#[command(author, version, about = "QUIC Server HTTP/2 Test Client", long_about = None)]
struct Args {
    /// Server URL
    #[arg(short, long, default_value = "https://localhost:4433")]
    url: String,

    /// Output directory for downloaded files
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Enable verbose output
    #[arg(short, long)]
    verbose: bool,

    /// Timeout in seconds
    #[arg(short, long, default_value = "30")]
    timeout: u64,

    /// Verify SSL certificates
    #[arg(long)]
    verify_ssl: bool,

    /// Test specific resource path
    #[arg(short, long)]
    path: Option<String>,
}

struct TestResult {
    path: String,
    status: u16,
    size: usize,
    duration_ms: u64,
    success: bool,
    error: Option<String>,
}

fn main() {
    let args = Args::parse();

    println!("🚀 QUIC Server HTTP/2 Test Client (curl-h2)");
    println!("==========================================");
    println!("Server: {}", args.url);
    println!("Timeout: {}s", args.timeout);
    println!(
        "SSL Verification: {}",
        if args.verify_ssl {
            "enabled"
        } else {
            "disabled (self-signed OK)"
        }
    );
    println!();

    let mut client = match H2Client::with_timeout(args.timeout) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("❌ Failed to create HTTP/2 client: {}", e);
            std::process::exit(1);
        }
    };

    client.set_verify_ssl(args.verify_ssl);

    // Test resources
    let test_paths = if let Some(ref path) = args.path {
        vec![path.clone()]
    } else {
        vec![
            "/".to_string(),
            "/index.css".to_string(),
            "/bw_test_pattern.png".to_string(),
        ]
    };

    let mut results = Vec::new();
    let mut total_success = 0;

    for path in test_paths {
        println!("📡 Testing: {}", path);

        let start = Instant::now();
        let result = test_resource(&mut client, &args.url, &path, args.verbose);
        let duration = start.elapsed();

        let success = result.is_ok();
        if success {
            total_success += 1;
        }

        let test_result = match result {
            Ok(response) => {
                let size = response.body.len();

                // Save to output directory if specified
                if let Some(ref output_dir) = args.output {
                    if let Err(e) = save_response(output_dir, &path, &response.body) {
                        eprintln!("  ⚠️  Failed to save {}: {}", path, e);
                    } else if args.verbose {
                        println!("  💾 Saved to {:?}", output_dir);
                    }
                }

                TestResult {
                    path,
                    status: response.status,
                    size,
                    duration_ms: duration.as_millis() as u64,
                    success,
                    error: None,
                }
            }
            Err(e) => TestResult {
                path,
                status: 0,
                size: 0,
                duration_ms: duration.as_millis() as u64,
                success: false,
                error: Some(e.to_string()),
            },
        };

        // Print result
        print_result(&test_result, args.verbose);
        results.push(test_result);
    }

    // Summary
    println!();
    println!("📊 Test Summary");
    println!("---------------");
    println!(
        "Total: {} | Success: {} | Failed: {}",
        results.len(),
        total_success,
        results.len() - total_success
    );

    if total_success == results.len() {
        println!();
        println!("✨ All tests passed! QUIC server is serving HTTP/2 correctly.");

        // Check HTTP/2 protocol
        if let Some(first_result) = results.first() {
            if first_result.status == 200 {
                println!("✅ Server is responding with valid HTTP/2 responses");
            }
        }
    } else {
        println!();
        println!("⚠️  Some tests failed. Check the errors above.");
        std::process::exit(1);
    }
}

fn test_resource(
    client: &mut H2Client,
    base_url: &str,
    path: &str,
    verbose: bool,
) -> Result<literbike::curl_h2::H2Response, H2Error> {
    let url = format!("{}{}", base_url.trim_end_matches('/'), path);

    if verbose {
        println!("  → GET {}", url);
    }

    let request = H2Request::get(&url)
        .header("Accept", "*/*")
        .header("User-Agent", "literbike-quic-curl-h2-test/0.1")
        .build();

    let response = client.request(request)?;

    if verbose {
        println!("  ← Status: {} {}", response.status, response.version);
        println!("  ← Headers:");
        for (name, value) in &response.headers {
            println!("    {}: {}", name, value);
        }
        println!("  ← Body: {} bytes", response.body.len());
    }

    Ok(response)
}

fn print_result(result: &TestResult, verbose: bool) {
    let status_icon = if result.success { "✅" } else { "❌" };

    if result.success {
        println!(
            "  {} {} - {} bytes ({}ms)",
            status_icon, result.path, result.size, result.duration_ms
        );
    } else {
        println!("  {} {} - FAILED", status_icon, result.path);
        if let Some(ref error) = result.error {
            println!("     Error: {}", error);
        }
    }

    if verbose && result.success {
        println!(
            "     Status: {} | Size: {} bytes | Duration: {}ms",
            result.status, result.size, result.duration_ms
        );
    }
}

fn save_response(output_dir: &PathBuf, path: &str, body: &[u8]) -> std::io::Result<()> {
    // Create output directory if it doesn't exist
    fs::create_dir_all(output_dir)?;

    // Determine filename from path
    let filename = if path == "/" {
        "index.html".to_string()
    } else {
        path.trim_start_matches('/').to_string()
    };

    let output_path = output_dir.join(filename);
    fs::write(&output_path, body)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_args_parsing() {
        let args = Args::parse_from(["quic_curl_h2"]);
        assert_eq!(args.url, "https://localhost:4433");
        assert_eq!(args.timeout, 30);
        assert!(!args.verify_ssl);
    }

    #[test]
    fn test_args_custom_url() {
        let args = Args::parse_from(["quic_curl_h2", "-u", "https://example.com:8443"]);
        assert_eq!(args.url, "https://example.com:8443");
    }
}

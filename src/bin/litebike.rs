//! LiteBike Network Bootloader - Multi-pathway network operations
//! Like a bootloader for network utilities, detects environment and chooses optimal execution path

use litebike::cli_core::CliApp;
use litebike::reentrant_dsl::ReentrantDSL;
use std::env;
use std::path::Path;
use std::net::UdpSocket;
use std::os::unix::process::CommandExt;
use litebike::secure_knock;
use log::{info, warn};

fn main() {
    let args: Vec<String> = env::args().collect();
    
    // Initialize logging
    env_logger::init();
    
    // Check for default port 8888 mode
    if args.len() == 1 || (args.len() == 2 && args[1] == "--default") {
        println!("🚀 Starting LiteBike unified listeners on port 8888");
        
        // Start unified listeners
        let config = litebike::unified_listener::UnifiedConfig::default();
        
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => {
                eprintln!("Failed to create tokio runtime: {}", e);
                std::process::exit(1);
            }
        };
        
        let listener = litebike::unified_listener::UnifiedListener::new(config);
        if let Err(e) = rt.block_on(listener.start()) {
            eprintln!("Listener error: {}", e);
            std::process::exit(1);
        }
        
        return; // Don't continue to CLI processing
    }

    // Start the secure knock listener in a separate thread
    if let Ok(port_str) = env::var("LITEBIKE_KNOCK_PORT") {
        if let Ok(port) = port_str.parse::<u16>() {
            if let Ok(psk) = env::var("LITEBIKE_KNOCK_PSK") {
                std::thread::spawn(move || {
                    let listen_addr = format!("0.0.0.0:{}", port);
                    let socket = UdpSocket::bind(&listen_addr).expect("Failed to bind UDP socket");
                    info!("Secure knock listener started on {}", listen_addr);
                    let mut buf = [0; secure_knock::KNOCK_PACKET_SIZE];
                    loop {
                        match socket.recv_from(&mut buf) {
                            Ok((amt, src)) => {
                                if secure_knock::verify_knock(psk.as_bytes(), &buf[..amt]) {
                                    info!("Received valid secure knock from {}", src);
                                } else {
                                    warn!("Received invalid knock from {}", src);
                                }
                            }
                            Err(e) => {
                                warn!("Error receiving knock packet: {}", e);
                            }
                        }
                    }
                });
            } else {
                warn!("LITEBIKE_KNOCK_PORT is set, but LITEBIKE_KNOCK_PSK is not. Knock listener not started.");
            }
        }
    }


    // Determine how we were invoked
    let program_name = Path::new(&args[0])
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("litebike");

    // Create CLI app and re-entrant DSL
    let mut cli_app = CliApp::new();
    let mut reentrant_dsl = ReentrantDSL::new();

    // Handle special commands first
    if args.len() > 1 {
        match args[1].as_str() {
            // Generate strategy report
            "--strategy-report" => {
                println!("{}", reentrant_dsl.generate_strategy_report());
                return;
            }
            // Test re-entrant execution
            "--test-reentrant" => {
                test_reentrant_execution(&mut reentrant_dsl);
                return;
            }
            // Generate and install completions
            "--install-completions" => {
                if let Err(e) = install_all_completions() {
                    eprintln!("Failed to install completions: {}", e);
                    std::process::exit(1);
                }
                return;
            }
            "knock" | "listen" => {
                // Handle knock and listen commands directly
                if let Err(e) = cli_app.run(args) {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
                return;
            }
            _ => {}
        }
    }

    // Handle symlink-based legacy compatibility
    match program_name {
        "ifconfig" | "netstat" | "route" | "ip" => {
            handle_legacy_symlink(program_name, &args[1..], &mut reentrant_dsl);
        }
        _ => {
            // Handle regular CLI execution
            if let Err(e) = cli_app.run(args) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
    }
}

/// Handle legacy symlink execution with re-entrant fallbacks
fn handle_legacy_symlink(program_name: &str, args: &[String], dsl: &mut ReentrantDSL) {
    let command_path = match program_name {
        "ifconfig" => "net.interfaces.list",
        "netstat" => "net.stats.connections", 
        "route" => "net.routes.list",
        "ip" => {
            // IP command needs subcommand analysis
            if !args.is_empty() {
                match args[0].as_str() {
                    "addr" | "address" => "net.interfaces.list",
                    "route" => "net.routes.list",
                    _ => "net.interfaces.list", // default
                }
            } else {
                "net.interfaces.list"
            }
        }
        _ => {
            eprintln!("Unknown legacy command: {}", program_name);
            std::process::exit(1);
        }
    };

    // Execute using re-entrant DSL
    match dsl.execute_command(command_path, args) {
        Ok(output) => {
            println!("{}", output);
        }
        Err(e) => {
            eprintln!("Execution failed: {}", e);
            
            // In a lockdown scenario, try to provide helpful guidance
            eprintln!("\nTrying alternative execution methods...");
            suggest_alternative_methods(program_name, command_path);
            std::process::exit(1);
        }
    }
}

/// Suggest alternative execution methods in lockdown scenarios
fn suggest_alternative_methods(program_name: &str, command_path: &str) {
    eprintln!("Alternative execution methods for {}:", program_name);
    
    match command_path {
        "net.interfaces.list" => {
            eprintln!("  1. Try: litebike net interfaces list");
            eprintln!("  2. Try: litebike connect repl <gateway_ip>");
            eprintln!("  3. Try: cat /proc/net/dev (if available)");
            eprintln!("  4. Try: ls /sys/class/net/ (if available)");
        }
        "net.stats.connections" => {
            eprintln!("  1. Try: litebike net stats connections");
            eprintln!("  2. Try: litebike connect repl <gateway_ip>");
            eprintln!("  3. Try: cat /proc/net/tcp /proc/net/udp (if available)");
            eprintln!("  4. Try: ss -tuln (if available)");
        }
        "net.routes.list" => {
            eprintln!("  1. Try: litebike net routes list");
            eprintln!("  2. Try: litebike connect repl <gateway_ip>");
            eprintln!("  3. Try: cat /proc/net/route (if available)");
            eprintln!("  4. Try: ip route show (if available)");
        }
        _ => {
            eprintln!("  1. Try: litebike --help");
            eprintln!("  2. Try: litebike connect repl <gateway_ip>");
        }
    }
    
    eprintln!("\nFor network lockdown scenarios:");
    eprintln!("  - Use: litebike proxy server (to become a relay point)");
    eprintln!("  - Use: litebike connect ssh <host> (for tunneling)");
    eprintln!("  - Use: litebike net discover hosts (to find litebike servers)");
}

/// Test re-entrant execution capabilities
fn test_reentrant_execution(dsl: &mut ReentrantDSL) {
    println!("=== Testing Re-entrant Execution Capabilities ===\n");

    let test_commands = vec![
        ("net.interfaces.list", vec![]),
        ("net.routes.list", vec![]),
        ("net.stats.connections", vec!["--tcp".to_string()]),
    ];

    for (command_path, args) in test_commands {
        println!("Testing: {} {:?}", command_path, args);
        
        match dsl.execute_command(command_path, &args) {
            Ok(output) => {
                println!("✅ Success: {}", output.lines().next().unwrap_or("(empty)"));
            }
            Err(e) => {
                println!("❌ Failed: {}", e);
            }
        }
        println!();
    }

    // Show strategy report
    println!("{}", dsl.generate_strategy_report());
}

/// Install completions for all symlinks and main binary
fn install_all_completions() -> Result<(), Box<dyn std::error::Error>> {
    use std::fs;
    use std::path::PathBuf;
    use litebike::cli_dsl::CommandDef;

    println!("Installing bash completions...");

    // Generate enhanced completion script that handles all cases
    let completion_script = generate_enhanced_completion_script();
    
    // Try to install to user completion directory
    let home = env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let user_completion_dir = format!("{}/.local/share/bash-completion/completions", home);
    
    fs::create_dir_all(&user_completion_dir)?;
    
    // Install completion for main binary
    let litebike_completion = PathBuf::from(&user_completion_dir).join("litebike");
    fs::write(&litebike_completion, &completion_script)?;
    println!("✅ Installed: {}", litebike_completion.display());

    // Install completions for legacy symlinks
    let legacy_commands = vec!["ifconfig", "netstat", "route", "ip"];
    for cmd in legacy_commands {
        let completion_file = PathBuf::from(&user_completion_dir).join(cmd);
        fs::write(&completion_file, &completion_script)?;
        println!("✅ Installed: {}", completion_file.display());
    }

    println!("\nTo activate completions, restart your shell or run:");
    println!("  source ~/.bashrc");
    println!("  # or");
    println!("  exec bash");

    Ok(())
}

/// Generate enhanced completion script that handles symlinks
fn generate_enhanced_completion_script() -> String {
    format!(
        r#"#!/bin/bash
# Enhanced LiteBike bash completion script with symlink support
# Handles litebike, ifconfig, netstat, route, and ip commands

_litebike_enhanced_completion() {{
    local cur prev words cword
    _init_completion || return

    local cmd="$1"
    local binary_name=$(basename "$cmd")
    
    # Determine the actual command being completed
    case "$binary_name" in
        "ifconfig")
            # Complete ifconfig-style arguments
            if [[ "$cur" == -* ]]; then
                COMPREPLY=($(compgen -W "-a --all -h --help" -- "$cur"))
            else
                # Complete interface names
                local interfaces
                interfaces=$($cmd completion _internal ifconfig "${{words[@]}}")
                COMPREPLY=($(compgen -W "$interfaces" -- "$cur"))
            fi
            ;;
        "netstat")
            if [[ "$cur" == -* ]]; then
                COMPREPLY=($(compgen -W "-a --all -l --listening -t --tcp -u --udp -h --help" -- "$cur"))
            fi
            ;;
        "route")
            if [[ "${{#words[@]}}" -eq 2 ]]; then
                COMPREPLY=($(compgen -W "add del delete show -h --help" -- "$cur"))
            fi
            ;;
        "ip")
            if [[ "${{#words[@]}}" -eq 2 ]]; then
                COMPREPLY=($(compgen -W "addr address route link neighbor neigh -h --help -4 -6" -- "$cur"))
            elif [[ "${{#words[@]}}" -eq 3 && ("${{words[2]}}" == "addr" || "${{words[2]}}" == "address") ]]; then
                COMPREPLY=($(compgen -W "show add del delete" -- "$cur"))
            elif [[ "${{#words[@]}}" -eq 3 && "${{words[2]}}" == "route" ]]; then
                COMPREPLY=($(compgen -W "show add del delete get" -- "$cur"))
            fi
            ;;
        "litebike"|*)
            # Use litebike's built-in completion system
            local completions
            completions=$($cmd completion _internal "${{words[@]}}")
            
            if [[ -n "$completions" ]]; then
                COMPREPLY=($(compgen -W "$completions" -- "$cur"))
            fi
            ;;
    esac

    return 0
}}

# Register completion functions
complete -F _litebike_enhanced_completion litebike
complete -F _litebike_enhanced_completion ifconfig
complete -F _litebike_enhanced_completion netstat
complete -F _litebike_enhanced_completion route
complete -F _litebike_enhanced_completion ip

# Auto-detect and register completions for litebike symlinks
if command -v litebike >/dev/null 2>&1; then
    # Check if legacy commands are symlinks to litebike
    for cmd in ifconfig netstat route ip; do
        if [[ -L "$(command -v $cmd 2>/dev/null)" ]]; then
            local target=$(readlink -f "$(command -v $cmd)")
            if [[ "$target" == *"litebike"* ]]; then
                complete -F _litebike_enhanced_completion $cmd
            fi
        fi
    done
fi
"#
    )
}

mod tests {
    use super::*;

    #[test]
    fn test_cli_initialization() {
        let mut cli_app = CliApp::new();
        let mut dsl = ReentrantDSL::new();
        
        // Test that DSL initializes with expected commands
        let report = dsl.generate_strategy_report();
        assert!(report.contains("net.interfaces.list"));
        assert!(report.contains("net.routes.list"));
        assert!(report.contains("net.stats.connections"));
    }

    #[test]
    fn test_completion_generation() {
        let dsl = ReentrantDSL::new();
        let completions = dsl.get_completions("net.interfaces.list", "--");
        assert!(!completions.is_empty());
        assert!(completions.iter().any(|c| c.starts_with("--")));
    }
}
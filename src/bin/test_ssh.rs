//! Test SSH client integration with syscall-based network operations
//! Minimal test for Android/Termux compatibility

use litebike::ssh_client::{SshClient, SshConfig, SshTunnelManager, create_ssh_config, test_ssh_connection};
use litebike::syscall_netops::SyscallNetOps;
use std::net::Ipv4Addr;
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    
    println!("LiteBike SSH Integration Test");
    println!("============================");

    // Test 1: Basic syscall network operations
    println!("\n1. Testing syscall-based network discovery...");
    let netops = SyscallNetOps::new();
    
    match netops.get_interface_list() {
        Ok(interfaces) => {
            println!("   Found {} network interfaces:", interfaces.len());
            for (name, addr) in interfaces {
                println!("   - {}: {}", name, addr);
            }
        }
        Err(e) => {
            eprintln!("   Failed to get interfaces: {}", e);
        }
    }

    match netops.get_default_gateway() {
        Ok(gateway) => {
            println!("   Default gateway: {}", gateway);
            
            // Test 2: SSH connectivity test to gateway
            println!("\n2. Testing SSH connectivity to gateway...");
            
            if let Ok(gateway_ip) = gateway.parse::<Ipv4Addr>() {
                match test_ssh_connection(gateway_ip, 22) {
                    Ok(true) => {
                        println!("   SSH service detected on gateway {}:22", gateway_ip);
                        
                        // Test 3: SSH client creation
                        println!("\n3. Testing SSH client creation...");
                        let ssh_config = create_ssh_config(
                            gateway_ip,
                            Some("root".to_string()),
                            None,
                            Some(22),
                        );
                        
                        match SshClient::new(ssh_config) {
                            Ok(mut client) => {
                                println!("   SSH client created successfully");
                                
                                // Test connectivity (may fail without proper auth)
                                match client.test_connectivity() {
                                    Ok(connected) => {
                                        if connected {
                                            println!("   SSH client connectivity test: PASSED");
                                        } else {
                                            println!("   SSH client connectivity test: Not connected (expected)");
                                        }
                                    }
                                    Err(e) => {
                                        println!("   SSH client connectivity test failed: {}", e);
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("   Failed to create SSH client: {}", e);
                            }
                        }
                        
                        // Test 4: SSH tunnel manager
                        println!("\n4. Testing SSH tunnel manager...");
                        let mut tunnel_manager = SshTunnelManager::new();
                        
                        // Add some test tunnels
                        match tunnel_manager.add_tunnel(8081, "127.0.0.1".to_string(), 8080) {
                            Ok(tunnel_id) => {
                                println!("   Added tunnel {}: localhost:8081 -> 127.0.0.1:8080", tunnel_id);
                                
                                // List tunnels
                                let tunnels = tunnel_manager.list_tunnels();
                                for tunnel in tunnels {
                                    println!("   {}", tunnel);
                                }
                                
                                println!("   Active tunnels: {}", tunnel_manager.active_tunnel_count());
                            }
                            Err(e) => {
                                eprintln!("   Failed to add tunnel: {}", e);
                            }
                        }
                    }
                    Ok(false) => {
                        println!("   No SSH service detected on gateway");
                    }
                    Err(e) => {
                        eprintln!("   SSH connectivity test failed: {}", e);
                    }
                }
            } else {
                eprintln!("   Invalid gateway IP address: {}", gateway);
            }
        }
        Err(e) => {
            eprintln!("   Failed to get default gateway: {}", e);
        }
    }

    // Test 5: Direct SSH connection test with custom parameters
    if args.len() >= 3 {
        println!("\n5. Testing direct SSH connection...");
        let host = &args[1];
        let username = &args[2];
        
        if let Ok(host_ip) = host.parse::<Ipv4Addr>() {
            let ssh_config = create_ssh_config(
                host_ip,
                Some(username.clone()),
                args.get(3).cloned(),
                Some(22),
            );
            
            match SshClient::new(ssh_config) {
                Ok(mut client) => {
                    println!("   Attempting SSH connection to {}@{}...", username, host);
                    match client.connect() {
                        Ok(()) => {
                            println!("   SSH connection successful!");
                        }
                        Err(e) => {
                            eprintln!("   SSH connection failed: {}", e);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("   Failed to create SSH client: {}", e);
                }
            }
        } else {
            eprintln!("   Invalid host IP: {}", host);
        }
    } else {
        println!("\n5. Skipping direct SSH test (usage: test_ssh <host_ip> <username> [key_path])");
    }

    println!("\nSSH integration test completed.");
}
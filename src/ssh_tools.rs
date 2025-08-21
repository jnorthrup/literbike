/// Configure SSH ProxyCommand for ~/.ssh/config
pub fn configure_ssh_proxy_command(host: &str, socks_port: u16) -> std::io::Result<()> {
    use std::fs::{OpenOptions, create_dir_all};
    use std::path::Path;
    let home = std::env::var("HOME").unwrap_or_else(|_| "~".to_string());
    let ssh_dir = format!("{}/.ssh", home);
    create_dir_all(&ssh_dir)?;
    let config_path = format!("{}/config", ssh_dir);
    let mut file = OpenOptions::new().append(true).create(true).open(&config_path)?;
    writeln!(file, "\n# Proxy configuration for SSH")?;
    writeln!(file, "Host *")?;
    writeln!(file, "    ProxyCommand nc -x {}:{} %h %p", host, socks_port)?;
    writeln!(file, "# End Proxy Bridge Settings\n")?;
    Ok(())
}

/// Remove SSH ProxyCommand from ~/.ssh/config
pub fn clear_ssh_proxy_command() -> std::io::Result<()> {
    use std::fs::{read_to_string, write};
    let home = std::env::var("HOME").unwrap_or_else(|_| "~".to_string());
    let config_path = format!("{}/.ssh/config", home);
    if let Ok(contents) = read_to_string(&config_path) {
        let filtered: Vec<_> = contents
            .lines()
            .filter(|l| !l.contains("ProxyCommand nc -x") && !l.contains("# Proxy configuration for SSH") && !l.contains("# End Proxy Bridge Settings"))
            .collect();
        write(&config_path, filtered.join("\n"))?;
    }
    Ok(())
}

/// Client logic with SSH fallback
pub fn client_auto(host: Option<&str>, http_port: u16, socks_port: u16, user: &str, ssh_port: u16, script_path: &str) {
    let binding = get_gateway_ip();
    let gateway = host.unwrap_or(&binding);
    println!("[CLIENT] Configuring client for: {}", gateway);
    // Test proxy
    let http_test = Command::new("curl")
        .arg("-s").arg("--max-time").arg("2")
        .arg("--proxy").arg(format!("http://{}:{}", gateway, http_port))
        .arg("http://httpbin.org/ip")
        .output();
    if http_test.map_or(true, |o| o.stdout.is_empty()) {
        println!("[CLIENT] Proxy not responding, attempting to start via SSH...");
        let _ = ssh_start_server(gateway, user, ssh_port, script_path);
        std::thread::sleep(std::time::Duration::from_secs(3));
    }
    // Configure proxies
    configure_env_vars(gateway, http_port, socks_port);
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
    let _ = update_shell_profile(&shell, gateway, http_port, socks_port);
    let _ = configure_ssh_proxy_command(gateway, socks_port);
    show_status();
    test_connection(gateway, http_port, socks_port);
    println!("[CLIENT] Client configuration complete.");
}
/// Beastmode Install (Gist): Aggressive local troubleshooting and environment setup per referenced gist
pub fn beastmode_install_gist() {
    println!("[BEASTMODE-GIST] Starting aggressive local install and troubleshooting...");
    let host = "127.0.0.1";
    let http_port = 8080;
    let socks_port = 1080;
    configure_env_vars(host, http_port, socks_port);
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
    let _ = update_shell_profile(&shell, host, http_port, socks_port);
    show_status();
    test_connection(host, http_port, socks_port);
    println!("[BEASTMODE-GIST] Aggressive local install and troubleshooting complete.");
}
/// Beastmode Immediate: Aggressive local install and troubleshooting mode (no SSH)
pub fn beastmode_immediate(host: &str, http_port: u16, socks_port: u16) {
    println!("[BEASTMODE-IMMEDIATE] Aggressive local install and troubleshooting for {}:{}:{}", host, http_port, socks_port);
    configure_env_vars(host, http_port, socks_port);
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
    let _ = update_shell_profile(&shell, host, http_port, socks_port);
    show_status();
    test_connection(host, http_port, socks_port);
    println!("[BEASTMODE-IMMEDIATE] Aggressive local install and troubleshooting complete.");
}
/// Beastmode: Aggressive install and troubleshooting mode
pub fn beastmode_install(host: &str, user: &str, port: u16, script_path: &str) {
    println!("[BEASTMODE] Starting aggressive install and troubleshooting for {}@{}:{}", user, host, port);
    // Attempt SSH diagnostics
    ssh_diagnostics(host, user, port);
    // Attempt remote script copy and server start
    match ssh_start_server(host, user, port, script_path) {
        Ok(_) => println!("[BEASTMODE] Remote server started successfully."),
        Err(e) => println!("[BEASTMODE] Remote server start failed: {}", e),
    }
    // Aggressive environment setup
    configure_env_vars(host, 8080, 1080);
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
    let _ = update_shell_profile(&shell, host, 8080, 1080);
    // Aggressive status and connection test
    show_status();
    test_connection(host, 8080, 1080);
    println!("[BEASTMODE] Aggressive install and troubleshooting complete.");
}
pub fn ssh_diagnostics(host: &str, user: &str, port: u16) {
    let ssh_cmd = format!("ssh -p {} {}@{} echo OK", port, user, host);
    let status = Command::new("ssh")
        .arg("-p").arg(port.to_string())
        .arg(format!("{}@{}", user, host))
        .arg("echo OK")
        .status();
    match status {
        Ok(s) if s.success() => println!("SSH to {}@{}:{} succeeded", user, host, port),
        Ok(_) | Err(_) => println!("SSH to {}@{}:{} failed", user, host, port),
    }
    // TODO: Add more diagnostics (latency, key exchange, remote command execution, error reporting)
}
pub fn get_rmnet_interfaces() -> Vec<String> {
    // TODO: Replace with direct ioctl for interface enumeration
    let output = match Command::new("ifconfig").output() {
        Ok(output) => output,
        Err(_) => return vec![],
    };
    let s = String::from_utf8_lossy(&output.stdout);
    s.lines()
        .filter(|l| l.starts_with("rmnet_data"))
        .map(|l| l.split(':').next().unwrap_or("").to_string())
        .collect()
}

pub fn get_rmnet_ips() -> Vec<String> {
    // TODO: Replace with direct ioctl for IP address enumeration
    let output = match Command::new("ifconfig").output() {
        Ok(output) => output,
        Err(_) => return vec![],
    };
    let s = String::from_utf8_lossy(&output.stdout);
    let mut ips = Vec::new();
    let mut current_iface = "";
    for line in s.lines() {
        if line.starts_with("rmnet_data") {
            current_iface = line.split(':').next().unwrap_or("");
        } else if line.contains("inet ") {
            let ip = line.split_whitespace().nth(1).unwrap_or("");
            if !current_iface.is_empty() {
                ips.push(ip.to_string());
            }
        }
    }
    ips
}
pub fn update_shell_profile(shell: &str, host: &str, http_port: u16, socks_port: u16) -> io::Result<()> {
    let home = env::var("HOME").unwrap_or_else(|_| "~".to_string());
    let profile = if shell.contains("zsh") {
        format!("{}/.zshrc", home)
    } else {
        format!("{}/.bashrc", home)
    };
    let mut file = std::fs::OpenOptions::new().append(true).create(true).open(&profile)?;
    writeln!(file, "\n# Proxy Bridge Settings")?;
    writeln!(file, "export HTTP_PROXY=\"http://{}:{}\"", host, http_port)?;
    writeln!(file, "export HTTPS_PROXY=\"http://{}:{}\"", host, http_port)?;
    writeln!(file, "export ALL_PROXY=\"socks5h://{}:{}\"", host, socks_port)?;
    writeln!(file, "export NO_PROXY=\"localhost,127.0.0.1,::1\"")?;
    writeln!(file, "# End Proxy Bridge Settings\n")?;
    Ok(())
}
pub fn show_status() {
    // TODO: Replace with direct syscall/ioctl for process and network status
    let platform = detect_platform();
    println!("Platform: {}", platform);
    println!("Local IP: {}", get_local_ip());
    println!("Gateway IP: {}", get_gateway_ip());
    // Check proxy process
    let proxy_running = Command::new("pgrep").arg("-f").arg("litebike-proxy").output().map_or(false, |o| !o.stdout.is_empty());
    println!("Proxy server running: {}", if proxy_running { "yes" } else { "no" });
}

pub fn test_connection(host: &str, http_port: u16, socks_port: u16) {
    // TODO: Replace curl with direct socket connection
    let http_test = Command::new("curl")
        .arg("-s").arg("--max-time").arg("10")
        .arg("--proxy").arg(format!("http://{}:{}", host, http_port))
        .arg("http://httpbin.org/ip")
        .output();
    println!("HTTP proxy test: {}", if http_test.map_or(false, |o| !o.stdout.is_empty()) { "success" } else { "fail" });
    let socks_test = Command::new("curl")
        .arg("-s").arg("--max-time").arg("10")
        .arg("--socks5").arg(format!("{}:{}", host, socks_port))
        .arg("http://httpbin.org/ip")
        .output();
    println!("SOCKS5 proxy test: {}", if socks_test.map_or(false, |o| !o.stdout.is_empty()) { "success" } else { "fail" });
}
/// SSH session troubleshooting and instrumentation tools ported from s/proxy-bridge

use std::process::Command;
use std::env;
use std::io::{self, Write};

pub fn detect_platform() -> &'static str {
    if let Ok(prefix) = env::var("PREFIX") {
        if prefix.contains("com.termux") {
            return "termux";
        }
    }
    if cfg!(target_os = "macos") {
        return "macos";
    }
    "linux"
}

pub fn get_local_ip() -> String {
    match detect_platform() {
        "termux" => {
            // TODO: Replace with direct syscall/ioctl
            // Fallback: parse ifconfig output
            let output = match Command::new("ifconfig").output() {
                Ok(output) => output,
                Err(_) => return "127.0.0.1".to_string(),
            };
            let s = String::from_utf8_lossy(&output.stdout);
            for line in s.lines() {
                if line.contains("swlan0") {
                    // TODO: parse inet address
                }
            }
            "192.168.111.176".to_string()
        },
        "macos" => {
            // TODO: Replace with direct syscall/ioctl
            "192.168.1.1".to_string()
        },
        _ => {
            // TODO: Replace with direct syscall/ioctl
            "127.0.0.1".to_string()
        }
    }
}

pub fn get_gateway_ip() -> String {
    match detect_platform() {
        "macos" => "192.168.1.1".to_string(),
        _ => "192.168.1.1".to_string(),
    }
}

// TODO: Add SSH remote start, proxy server management, environment setup, and fallback logic

pub fn ssh_start_server(host: &str, user: &str, port: u16, script_path: &str) -> io::Result<()> {
    // Copy the script to remote host (if needed)
    let scp_status = Command::new("scp")
        .arg("-P").arg(port.to_string())
        .arg(script_path)
        .arg(format!("{}@{}:proxy-bridge-improved", user, host))
        .status()?;
    if !scp_status.success() {
        eprintln!("Failed to copy script to remote host");
        // Continue anyway
    }
    // Start the server remotely
    let ssh_status = Command::new("ssh")
        .arg("-p").arg(port.to_string())
        .arg(format!("{}@{}", user, host))
        .arg("bash proxy-bridge-improved server")
        .status()?;
    if !ssh_status.success() {
        eprintln!("Failed to start server on remote host");
        return Err(io::Error::new(io::ErrorKind::Other, "Remote server start failed"));
    }
    Ok(())
}

// TODO: Incrementally port all actionable features from s/proxy-bridge

pub fn start_proxy_server(bind_ip: &str, egress_interface: Option<&str>, litebike_path: &str) -> io::Result<()> {
    let mut cmd = Command::new(litebike_path);
    cmd.env("BIND_IP", bind_ip);
    if let Some(iface) = egress_interface {
        cmd.env("EGRESS_INTERFACE", iface);
    }
    let status = cmd.status()?;
    if !status.success() {
        eprintln!("Failed to start proxy server");
        return Err(io::Error::new(io::ErrorKind::Other, "Proxy server start failed"));
    }
    Ok(())
}

pub fn stop_proxy_server() {
    // TODO: Replace pkill with direct syscall/ioctl if possible
    let _ = Command::new("pkill").arg("-f").arg("litebike-proxy").status();
    let _ = Command::new("pkill").arg("-f").arg("3proxy").status();
    let _ = Command::new("pkill").arg("-f").arg("vproxy").status();
}

pub fn configure_env_vars(host: &str, http_port: u16, socks_port: u16) {
    // Set environment variables for proxy
    env::set_var("HTTP_PROXY", format!("http://{}:{}", host, http_port));
    env::set_var("HTTPS_PROXY", format!("http://{}:{}", host, http_port));
    env::set_var("ALL_PROXY", format!("socks5h://{}:{}", host, socks_port));
    env::set_var("NO_PROXY", "localhost,127.0.0.1,::1");
    // TODO: Add shell profile persistence and fallback logic
}

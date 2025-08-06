//! Core CLI implementation with integrated bash completion and symlink support
//! Provides Git-like command structure with intelligent completion

use crate::cli_dsl::{CommandDef, CompletionHint, CompletionRegistry};
use crate::syscall_netops::SyscallNetOps;
use crate::ssh_client::{SshClient, SshConfig, SshTunnel, create_ssh_config, test_ssh_connection};
use std::collections::HashMap;
use std::env;
use std::path::Path;

/// Main CLI application
pub struct CliApp {
    root_command: CommandDef,
    completion_registry: CompletionRegistry,
    symlink_mappings: HashMap<String, Vec<String>>,
}

impl CliApp {
    pub fn new() -> Self {
        let mut app = Self {
            root_command: CommandDef::root(),
            completion_registry: CompletionRegistry::new(),
            symlink_mappings: HashMap::new(),
        };
        
        // Register symlink mappings to their corresponding command paths
        app.register_symlink_mappings();
        app
    }

    /// Register mappings from symlink names to command paths
    fn register_symlink_mappings(&mut self) {
        self.symlink_mappings.insert(
            "ifconfig".to_string(),
            vec!["litebike".to_string(), "utils".to_string(), "ifconfig".to_string()]
        );
        self.symlink_mappings.insert(
            "netstat".to_string(),
            vec!["litebike".to_string(), "utils".to_string(), "netstat".to_string()]
        );
        self.symlink_mappings.insert(
            "route".to_string(),
            vec!["litebike".to_string(), "utils".to_string(), "route".to_string()]
        );
        self.symlink_mappings.insert(
            "ip".to_string(),
            vec!["litebike".to_string(), "utils".to_string(), "ip".to_string()]
        );
        
        // Also register the modern equivalents
        self.symlink_mappings.insert(
            "litebike-ifconfig".to_string(),
            vec!["litebike".to_string(), "net".to_string(), "interfaces".to_string()]
        );
        self.symlink_mappings.insert(
            "litebike-netstat".to_string(),
            vec!["litebike".to_string(), "net".to_string(), "stats".to_string(), "connections".to_string()]
        );
        self.symlink_mappings.insert(
            "litebike-route".to_string(),
            vec!["litebike".to_string(), "net".to_string(), "routes".to_string()]
        );
    }

    /// Main entry point for CLI processing
    pub fn run(&mut self, args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
        if args.is_empty() {
            return Err("No arguments provided".into());
        }

        // Detect how we were invoked
        let program_name = Path::new(&args[0])
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("litebike");

        // Handle completion requests
        if args.len() > 2 && args[1] == "completion" && args[2] == "_internal" {
            return self.handle_completion(&args[3..]);
        }

        // Handle symlink invocation
        if let Some(mapped_path) = self.symlink_mappings.get(program_name) {
            return self.handle_symlink_command(program_name, mapped_path, &args[1..]);
        }

        // Handle regular litebike command
        self.handle_command(&args[1..])
    }

    /// Handle symlink-based command invocation
    fn handle_symlink_command(
        &self,
        symlink_name: &str,
        command_path: &[String],
        args: &[String],
    ) -> Result<(), Box<dyn std::error::Error>> {
        println!("Debug: Handling symlink '{}' -> {:?} with args {:?}", symlink_name, command_path, args);
        
        // Find the target command in our tree
        if let Some(target_cmd) = self.root_command.find_command(command_path) {
            // For legacy utilities, we need special handling
            match symlink_name {
                "ifconfig" => self.handle_ifconfig_compat(args),
                "netstat" => self.handle_netstat_compat(args),
                "route" => self.handle_route_compat(args), 
                "ip" => self.handle_ip_compat(args),
                _ => {
                    // For modern equivalents, just execute the mapped command
                    self.execute_command(target_cmd, args)
                }
            }
        } else {
            Err(format!("Command path not found: {:?}", command_path).into())
        }
    }

    /// Handle regular litebike command
    fn handle_command(&self, args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
        if args.is_empty() {
            self.print_help(&self.root_command);
            return Ok(());
        }

        // Parse command path
        let mut command_path = Vec::new();
        let mut remaining_args = args;
        let mut current_cmd = &self.root_command;

        // Walk down the command tree
        while !remaining_args.is_empty() {
            let arg = &remaining_args[0];
            
            // Check if this is an option (starts with -)
            if arg.starts_with('-') {
                break;
            }

            // Look for subcommand
            if let Some(subcmd) = current_cmd.subcommands.iter().find(|cmd| cmd.name == *arg) {
                command_path.push(arg.clone());
                current_cmd = subcmd;
                remaining_args = &remaining_args[1..];
            } else {
                break;
            }
        }

        // Handle special commands
        match command_path.get(0).map(|s| s.as_str()) {
            Some("completion") => self.handle_completion_command(&command_path[1..], remaining_args),
            _ => self.execute_command(current_cmd, remaining_args),
        }
    }

    /// Handle bash completion generation
    fn handle_completion(&self, args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
        if args.is_empty() {
            return Ok(());
        }

        // Parse the command line being completed
        let empty_string = String::new();
        let current_word = args.last().unwrap_or(&empty_string);
        let command_words = if args.len() > 1 { &args[..args.len()-1] } else { &[] };

        // Determine if we're completing a symlink command
        let program_name = if !command_words.is_empty() {
            Path::new(&command_words[0])
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("litebike")
        } else {
            "litebike"
        };

        let completions = if let Some(mapped_path) = self.symlink_mappings.get(program_name) {
            // Complete for symlink command
            self.get_symlink_completions(program_name, mapped_path, &command_words[1..], current_word)
        } else {
            // Complete for regular command
            self.root_command.get_completions(command_words, current_word)
        };

        // Output completions
        for completion in completions {
            println!("{}", completion);
        }

        Ok(())
    }

    /// Get completions for symlink commands
    fn get_symlink_completions(
        &self,
        symlink_name: &str,
        command_path: &[String],
        args: &[String],
        current_word: &str,
    ) -> Vec<String> {
        // Find the mapped command
        if let Some(target_cmd) = self.root_command.find_command(command_path) {
            match symlink_name {
                "ifconfig" => self.get_ifconfig_completions(args, current_word),
                "netstat" => self.get_netstat_completions(args, current_word),
                "route" => self.get_route_completions(args, current_word),
                "ip" => self.get_ip_completions(args, current_word),
                _ => target_cmd.get_completions(args, current_word),
            }
        } else {
            vec![]
        }
    }

    /// Get completions for ifconfig command
    fn get_ifconfig_completions(&self, args: &[String], current_word: &str) -> Vec<String> {
        let mut completions = Vec::new();

        // If no args yet, complete with interface names and options
        if args.is_empty() {
            // Add interface completions
            completions.extend(self.completion_registry.get_completions(&CompletionHint::Interfaces));
            
            // Add option completions
            if current_word.starts_with('-') {
                completions.extend(vec![
                    "--all".to_string(),
                    "--help".to_string(),
                    "-a".to_string(),
                    "-h".to_string(),
                ]);
            }
        } else if args.len() == 1 && !args[0].starts_with('-') {
            // After interface name, offer configuration options
            if current_word.is_empty() || !current_word.starts_with('-') {
                completions.extend(vec![
                    "up".to_string(),
                    "down".to_string(),
                ]);
            }
        }

        completions.into_iter()
            .filter(|comp| comp.starts_with(current_word))
            .collect()
    }

    /// Get completions for netstat command  
    fn get_netstat_completions(&self, _args: &[String], current_word: &str) -> Vec<String> {
        let mut completions = Vec::new();

        if current_word.starts_with('-') {
            completions.extend(vec![
                "--all".to_string(),
                "--listening".to_string(),
                "--tcp".to_string(),
                "--udp".to_string(),
                "--help".to_string(),
                "-a".to_string(),
                "-l".to_string(),
                "-t".to_string(),
                "-u".to_string(),
                "-h".to_string(),
            ]);
        }

        completions.into_iter()
            .filter(|comp| comp.starts_with(current_word))
            .collect()
    }

    /// Get completions for route command
    fn get_route_completions(&self, args: &[String], current_word: &str) -> Vec<String> {
        let mut completions = Vec::new();

        if args.is_empty() {
            completions.extend(vec![
                "add".to_string(),
                "del".to_string(),
                "delete".to_string(),
                "show".to_string(),
            ]);
        }

        if current_word.starts_with('-') {
            completions.extend(vec![
                "--help".to_string(),
                "-h".to_string(),
            ]);
        }

        completions.into_iter()
            .filter(|comp| comp.starts_with(current_word))
            .collect()
    }

    /// Get completions for ip command
    fn get_ip_completions(&self, args: &[String], current_word: &str) -> Vec<String> {
        let mut completions = Vec::new();

        if args.is_empty() {
            completions.extend(vec![
                "addr".to_string(),
                "address".to_string(),
                "route".to_string(),
                "link".to_string(),
                "neighbor".to_string(),
                "neigh".to_string(),
            ]);
        } else if args.len() == 1 {
            match args[0].as_str() {
                "addr" | "address" => {
                    completions.extend(vec![
                        "show".to_string(),
                        "add".to_string(),
                        "del".to_string(),
                        "delete".to_string(),
                    ]);
                }
                "route" => {
                    completions.extend(vec![
                        "show".to_string(),
                        "add".to_string(),
                        "del".to_string(),
                        "delete".to_string(),
                        "get".to_string(),
                    ]);
                }
                "link" => {
                    completions.extend(vec![
                        "show".to_string(),
                        "set".to_string(),
                    ]);
                }
                _ => {}
            }
        } else if args.len() == 2 && (args[0] == "addr" || args[0] == "address") && args[1] == "show" {
            // Complete interface names for "ip addr show"
            completions.extend(self.completion_registry.get_completions(&CompletionHint::Interfaces));
        }

        if current_word.starts_with('-') {
            completions.extend(vec![
                "--help".to_string(),
                "-h".to_string(),
                "-4".to_string(),
                "-6".to_string(),
            ]);
        }

        completions.into_iter()
            .filter(|comp| comp.starts_with(current_word))
            .collect()
    }

    /// Handle completion subcommand
    fn handle_completion_command(
        &self,
        subcommand: &[String],
        _args: &[String],
    ) -> Result<(), Box<dyn std::error::Error>> {
        if subcommand.is_empty() || subcommand[0] == "generate" {
            // Generate bash completion script
            println!("{}", CommandDef::generate_bash_completion());
        } else if subcommand[0] == "install" {
            // Install completion script
            self.install_completion()?;
        }
        Ok(())
    }

    /// Install completion script to system
    fn install_completion(&self) -> Result<(), Box<dyn std::error::Error>> {
        use std::fs;
        use std::path::PathBuf;

        let completion_script = CommandDef::generate_bash_completion();
        
        // Try different completion directories
        let completion_dirs = vec![
            "/usr/share/bash-completion/completions",
            "/etc/bash_completion.d",
            "/usr/local/share/bash-completion/completions",
        ];

        let home = env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let user_completion_dir = format!("{}/.local/share/bash-completion/completions", home);

        // Try to install to user directory first
        if let Ok(()) = fs::create_dir_all(&user_completion_dir) {
            let completion_file = PathBuf::from(&user_completion_dir).join("litebike");
            fs::write(&completion_file, &completion_script)?;
            println!("✓ Installed completion script to: {}", completion_file.display());
            return Ok(());
        }

        // Try system directories
        for dir in completion_dirs {
            if PathBuf::from(dir).exists() {
                let completion_file = PathBuf::from(dir).join("litebike");
                match fs::write(&completion_file, &completion_script) {
                    Ok(()) => {
                        println!("✓ Installed completion script to: {}", completion_file.display());
                        return Ok(());
                    }
                    Err(e) => {
                        eprintln!("Failed to write to {}: {}", completion_file.display(), e);
                        continue;
                    }
                }
            }
        }

        // Fallback: write to current directory
        let completion_file = PathBuf::from("litebike_completion.bash");
        fs::write(&completion_file, &completion_script)?;
        println!("✓ Generated completion script: {}", completion_file.display());
        println!("  To install, run: sudo cp {} /usr/share/bash-completion/completions/litebike", completion_file.display());

        Ok(())
    }

    /// Execute a command
    fn execute_command(
        &self,
        command: &CommandDef,
        args: &[String],
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Check for help flag
        if args.contains(&"--help".to_string()) || args.contains(&"-h".to_string()) {
            self.print_help(command);
            return Ok(());
        }

        // For now, just print what would be executed
        println!("Executing command: {}", command.name);
        if !args.is_empty() {
            println!("Arguments: {:?}", args);
        }
        
        // TODO: Implement actual command execution
        match command.name.as_str() {
            "interfaces" => println!("Would list network interfaces"),
            "routes" => println!("Would show routing table"),
            "connections" => println!("Would show network connections"),
            "ifconfig" => println!("Would run ifconfig compatibility mode"),
            "netstat" => println!("Would run netstat compatibility mode"),
            "route" => println!("Would run route compatibility mode"),
            "ip" => println!("Would run ip compatibility mode"),
            "auto" => self.execute_client_auto(args),
            "connect" => self.execute_client_connect(args),
            _ => println!("Command implementation pending"),
        }

        Ok(())
    }

    /// Legacy ifconfig compatibility
    fn handle_ifconfig_compat(&self, args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
        println!("ifconfig compatibility mode");
        if !args.is_empty() {
            println!("Args: {:?}", args);
        }
        // TODO: Call into existing netutils ifconfig implementation
        Ok(())
    }

    /// Legacy netstat compatibility
    fn handle_netstat_compat(&self, args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
        println!("netstat compatibility mode");
        if !args.is_empty() {
            println!("Args: {:?}", args);
        }
        // TODO: Call into existing netutils netstat implementation
        Ok(())
    }

    /// Legacy route compatibility
    fn handle_route_compat(&self, args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
        println!("route compatibility mode");
        if !args.is_empty() {
            println!("Args: {:?}", args);
        }
        // TODO: Call into existing netutils route implementation
        Ok(())
    }

    /// Legacy ip compatibility
    fn handle_ip_compat(&self, args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
        println!("ip compatibility mode");
        if !args.is_empty() {
            println!("Args: {:?}", args);
        }
        // TODO: Call into existing netutils ip implementation
        Ok(())
    }

    /// Print help for a command
    fn print_help(&self, command: &CommandDef) {
        println!("{}", command.description);
        println!();

        if !command.subcommands.is_empty() {
            println!("SUBCOMMANDS:");
            for subcmd in &command.subcommands {
                println!("    {:<15} {}", subcmd.name, subcmd.description);
            }
            println!();
        }

        if !command.options.is_empty() {
            println!("OPTIONS:");
            for opt in &command.options {
                let short_flag = opt.short.map(|c| format!("-{}", c)).unwrap_or_else(|| "   ".to_string());
                let long_flag = format!("--{}", opt.long);
                let flags = if opt.short.is_some() {
                    format!("{}, {}", short_flag, long_flag)
                } else {
                    format!("    {}", long_flag)
                };
                
                let value_hint = if opt.takes_value {
                    format!(" <{}>", opt.value_name.as_ref().unwrap_or(&"VALUE".to_string()))
                } else {
                    "".to_string()
                };

                println!("    {:<20}{} {}", flags + &value_hint, "", opt.description);
            }
            println!();
        }

        if !command.examples.is_empty() {
            println!("EXAMPLES:");
            for example in &command.examples {
                println!("    {}", example);
            }
            println!();
        }
    }

    /// Execute client auto-detection command with SSH integration
    fn execute_client_auto(&self, args: &[String]) {
        println!("Starting LiteBike client auto-detection...");
        
        // Parse arguments including SSH parameters
        let mut port_preference = None;
        let mut timeout = 5;
        let mut ip_range = None;
        let mut ssh_user = None;
        let mut ssh_key = None;
        let mut ssh_port = None;
        let mut ssh_tunnel = false;
        let mut ssh_local_port = None;
        
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--port" | "-p" => {
                    if i + 1 < args.len() {
                        port_preference = args[i + 1].parse().ok();
                        i += 2;
                    } else {
                        eprintln!("Error: --port requires a value");
                        return;
                    }
                }
                "--timeout" | "-t" => {
                    if i + 1 < args.len() {
                        timeout = args[i + 1].parse().unwrap_or(5);
                        i += 2;
                    } else {
                        eprintln!("Error: --timeout requires a value");
                        return;
                    }
                }
                "--range" | "-r" => {
                    if i + 1 < args.len() {
                        ip_range = Some(args[i + 1].clone());
                        i += 2;
                    } else {
                        eprintln!("Error: --range requires a value");
                        return;
                    }
                }
                "--ssh-user" => {
                    if i + 1 < args.len() {
                        ssh_user = Some(args[i + 1].clone());
                        i += 2;
                    } else {
                        eprintln!("Error: --ssh-user requires a value");
                        return;
                    }
                }
                "--ssh-key" => {
                    if i + 1 < args.len() {
                        ssh_key = Some(args[i + 1].clone());
                        i += 2;
                    } else {
                        eprintln!("Error: --ssh-key requires a value");
                        return;
                    }
                }
                "--ssh-port" => {
                    if i + 1 < args.len() {
                        ssh_port = args[i + 1].parse().ok();
                        i += 2;
                    } else {
                        eprintln!("Error: --ssh-port requires a value");
                        return;
                    }
                }
                "--ssh-tunnel" => {
                    ssh_tunnel = true;
                    i += 1;
                }
                "--ssh-local-port" => {
                    if i + 1 < args.len() {
                        ssh_local_port = args[i + 1].parse().ok();
                        i += 2;
                    } else {
                        eprintln!("Error: --ssh-local-port requires a value");
                        return;
                    }
                }
                _ => {
                    i += 1;
                }
            }
        }

        // Execute enhanced auto-detection with SSH integration
        match self.perform_enhanced_auto_detection(
            port_preference, 
            timeout, 
            ip_range, 
            ssh_user,
            ssh_key,
            ssh_port,
            ssh_tunnel,
            ssh_local_port
        ) {
            Ok(result) => {
                println!("Auto-detection completed successfully:");
                println!("{}", result);
            }
            Err(e) => {
                eprintln!("Auto-detection failed: {}", e);
                eprintln!("This may be due to network restrictions or lack of available proxy servers.");
                if ssh_tunnel {
                    eprintln!("SSH tunnel setup may have failed - check SSH connectivity and credentials.");
                }
            }
        }
    }

    /// Execute client connect command
    fn execute_client_connect(&self, args: &[String]) {
        println!("Connecting to specific proxy server...");
        
        let mut server = None;
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--server" | "-s" => {
                    if i + 1 < args.len() {
                        server = Some(args[i + 1].clone());
                        i += 2;
                    } else {
                        eprintln!("Error: --server requires a value");
                        return;
                    }
                }
                _ => {
                    i += 1;
                }
            }
        }

        if let Some(server_addr) = server {
            match self.connect_to_proxy_server(&server_addr) {
                Ok(result) => {
                    println!("Connection established: {}", result);
                }
                Err(e) => {
                    eprintln!("Connection failed: {}", e);
                }
            }
        } else {
            eprintln!("Error: --server parameter is required");
        }
    }

    /// Perform network auto-detection using only syscalls
    fn perform_network_auto_detection(
        &self,
        port_preference: Option<u16>,
        timeout: u32,
        ip_range: Option<String>,
    ) -> Result<String, Box<dyn std::error::Error>> {
        // Step 3: Determine scan range
        let scan_range = if let Some(range) = ip_range {
            range
        } else {
            // Step 1: Discover local network interfaces using syscalls
            let interfaces = self.discover_network_interfaces()?;
            
            // Step 2: Get default gateway using routing table syscalls  
            let gateway = self.discover_default_gateway()?;
            
            // Auto-determine based on interface configuration
            self.auto_determine_scan_range(&interfaces, &gateway)?
        };
        
        println!("Scanning range: {}", scan_range);
        
        // Step 4: Scan for proxy servers
        let proxy_servers = self.scan_for_proxy_servers(&scan_range, port_preference, timeout)?;
        
        // Step 5: Test and rank discovered servers
        let best_server = self.test_and_rank_proxy_servers(proxy_servers, timeout)?;
        
        // Step 6: Establish connection
        if let Some(server) = best_server {
            self.connect_to_proxy_server(&server)
        } else {
            Err("No suitable proxy servers found in the network".into())
        }
    }

    /// Enhanced auto-detection with SSH tunnel integration
    fn perform_enhanced_auto_detection(
        &self,
        port_preference: Option<u16>,
        timeout: u32,
        ip_range: Option<String>,
        ssh_user: Option<String>,
        ssh_key: Option<String>,
        ssh_port: Option<u16>,
        ssh_tunnel: bool,
        ssh_local_port: Option<u16>,
    ) -> Result<String, Box<dyn std::error::Error>> {

        // Step 1: Discover local network interfaces using syscalls
        let interfaces = self.discover_network_interfaces()?;
        
        // Step 2: Get default gateway using routing table syscalls  
        let gateway_ip = self.discover_default_gateway()?;
        println!("Discovered gateway: {}", gateway_ip);
        
        // Step 3: Check for SSH service on gateway if SSH tunnel is requested
        if ssh_tunnel {
            println!("SSH tunnel requested - testing SSH connectivity to gateway...");
            
            let ssh_port_to_use = ssh_port.unwrap_or(22);
            
            // Test SSH connectivity using direct syscalls
            match test_ssh_connection(gateway_ip, ssh_port_to_use) {
                Ok(true) => {
                    println!("SSH service detected on gateway {}:{}", gateway_ip, ssh_port_to_use);
                    
                    // Attempt to establish SSH connection
                    let ssh_config = create_ssh_config(
                        gateway_ip,
                        ssh_user,
                        ssh_key,
                        Some(ssh_port_to_use),
                    );
                    
                    match self.establish_ssh_tunnel(ssh_config, ssh_local_port) {
                        Ok(tunnel_info) => {
                            println!("SSH tunnel established successfully: {}", tunnel_info);
                            
                            // Continue with proxy detection, potentially using the tunnel
                            return self.perform_network_auto_detection_with_tunnel(
                                port_preference, timeout, ip_range, Some(tunnel_info)
                            );
                        }
                        Err(e) => {
                            eprintln!("SSH tunnel establishment failed: {}", e);
                            eprintln!("Falling back to direct proxy detection...");
                        }
                    }
                }
                Ok(false) => {
                    println!("No SSH service detected on gateway - continuing with direct proxy detection");
                }
                Err(e) => {
                    eprintln!("SSH connectivity test failed: {}", e);
                }
            }
        }

        // Fall back to standard auto-detection using gateway for range determination
        let scan_range = if let Some(range) = ip_range {
            range
        } else {
            // Auto-determine based on interface configuration
            self.auto_determine_scan_range(&interfaces, &gateway_ip)?
        };
        
        println!("Scanning range: {}", scan_range);
        
        // Step 4: Scan for proxy servers
        let proxy_servers = self.scan_for_proxy_servers(&scan_range, port_preference, timeout)?;
        
        // Step 5: Test and rank discovered servers
        let best_server = self.test_and_rank_proxy_servers(proxy_servers, timeout)?;
        
        // Step 6: Establish connection
        if let Some(server) = best_server {
            self.connect_to_proxy_server(&server)
        } else {
            Err("No suitable proxy servers found in the network".into())
        }
    }

    /// Establish SSH tunnel to gateway
    fn establish_ssh_tunnel(
        &self,
        ssh_config: SshConfig,
        local_port: Option<u16>,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let mut ssh_client = SshClient::new(ssh_config)?;
        
        // Connect to SSH server
        ssh_client.connect()?;
        
        if !ssh_client.is_connected() {
            return Err("SSH connection failed".into());
        }
        
        // Create tunnel configuration
        let local_port = local_port.unwrap_or(8080);
        let tunnel = SshTunnel {
            local_port,
            remote_host: "127.0.0.1".to_string(),
            remote_port: 8080, // Default proxy port
        };
        
        // Establish tunnel
        let mut tunnel_handle = ssh_client.establish_tunnel(&tunnel)?;
        tunnel_handle.start_forwarding()?;
        
        Ok(format!("localhost:{} -> {}", 
                  tunnel_handle.local_port(), 
                  tunnel_handle.remote_endpoint()))
    }

    /// Perform network auto-detection with tunnel support
    fn perform_network_auto_detection_with_tunnel(
        &self,
        port_preference: Option<u16>,
        timeout: u32,
        ip_range: Option<String>,
        tunnel_info: Option<String>,
    ) -> Result<String, Box<dyn std::error::Error>> {
        // If we have a tunnel, try to use it first
        if let Some(tunnel) = tunnel_info {
            println!("Using SSH tunnel for proxy connection: {}", tunnel);
            
            // Extract local port from tunnel info
            if let Some(_colon_pos) = tunnel.find(':') {
                if let Some(arrow_pos) = tunnel.find(" -> ") {
                    let local_part = &tunnel[..arrow_pos];
                    if let Some(port_str) = local_part.split(':').nth(1) {
                        if let Ok(port) = port_str.parse::<u16>() {
                            let tunnel_address = format!("localhost:{}", port);
                            return self.connect_to_proxy_server(&tunnel_address);
                        }
                    }
                }
            }
        }
        
        // Fall back to standard detection
        self.perform_network_auto_detection(port_preference, timeout, ip_range)
    }

    /// Connect to a specific proxy server
    fn connect_to_proxy_server(&self, server_addr: &str) -> Result<String, Box<dyn std::error::Error>> {
        println!("Connecting to proxy server: {}", server_addr);
        
        // Parse server address
        let parts: Vec<&str> = server_addr.split(':').collect();
        if parts.len() != 2 {
            return Err("Invalid server address format. Expected HOST:PORT".into());
        }
        
        let host = parts[0];
        let port: u16 = parts[1].parse()
            .map_err(|_| "Invalid port number")?;
        
        // Convert hostname to IP if needed
        let ip = host.parse::<std::net::Ipv4Addr>()
            .or_else(|_| {
                // Simple hostname resolution fallback
                // In a full implementation, we'd use getaddrinfo() syscall
                match host {
                    "localhost" => Ok(std::net::Ipv4Addr::new(127, 0, 0, 1)),
                    _ => Err("Hostname resolution not implemented - use IP address")
                }
            })
            .map_err(|e| format!("Failed to resolve hostname {}: {}", host, e))?;
        
        // Test connection using raw syscalls
        match SyscallNetOps::test_detailed_proxy_connection(ip, port, 5) {
            Ok(connection_info) => {
                Ok(format!(
                    "Successfully connected to {}:{}\nConnection details: {}",
                    ip, port, connection_info
                ))
            }
            Err(e) => {
                Err(format!("Failed to connect to {}:{}: {}", ip, port, e).into())
            }
        }
    }

    /// Discover network interfaces using syscalls
    fn discover_network_interfaces(&self) -> Result<Vec<crate::syscall_netops::NetworkInterface>, Box<dyn std::error::Error>> {
        SyscallNetOps::discover_interfaces()
            .map_err(|e| e.into())
    }

    /// Discover default gateway using syscalls
    fn discover_default_gateway(&self) -> Result<std::net::Ipv4Addr, Box<dyn std::error::Error>> {
        SyscallNetOps::discover_default_gateway()
            .map_err(|e| e.into())
    }

    /// Auto-determine scan range based on network configuration
    fn auto_determine_scan_range(
        &self,
        interfaces: &[crate::syscall_netops::NetworkInterface],
        gateway: &std::net::Ipv4Addr,
    ) -> Result<String, Box<dyn std::error::Error>> {
        SyscallNetOps::auto_determine_scan_range(interfaces, gateway)
            .map_err(|e| e.into())
    }

    /// Scan for proxy servers using raw socket connections
    fn scan_for_proxy_servers(
        &self,
        scan_range: &str,
        port_preference: Option<u16>,
        timeout: u32,
    ) -> Result<Vec<crate::syscall_netops::ProxyServer>, Box<dyn std::error::Error>> {
        SyscallNetOps::scan_for_proxy_servers(scan_range, port_preference, timeout)
            .map_err(|e| e.into())
    }

    /// Test and rank discovered proxy servers
    fn test_and_rank_proxy_servers(
        &self,
        servers: Vec<crate::syscall_netops::ProxyServer>,
        timeout: u32,
    ) -> Result<Option<String>, Box<dyn std::error::Error>> {
        SyscallNetOps::test_and_rank_proxy_servers(servers, timeout)
            .map_err(|e| e.into())
    }
}
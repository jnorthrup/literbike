//! Re-entrant DSL for network operations with multiple execution pathways
//! Designed to maintain functionality even under various network lockdown scenarios

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

/// Re-entrant command execution strategy
#[derive(Debug, Clone)]
pub enum ExecutionStrategy {
    /// Direct syscall execution (most reliable)
    DirectSyscall,
    /// Legacy utility exec (ifconfig, netstat, etc.)
    LegacyExec,
    /// Library function call
    LibraryCall,
    /// Network-based execution via REPL
    NetworkREPL,
    /// SSH tunnel execution
    SSHTunnel,
    /// HTTP proxy execution
    HTTPProxy,
    /// SOCKS proxy execution
    SOCKSProxy,
    /// Fallback shell execution
    ShellFallback,
}

/// Command execution context with fallback strategies
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    pub primary_strategy: ExecutionStrategy,
    pub fallback_strategies: Vec<ExecutionStrategy>,
    pub environment_constraints: Vec<EnvironmentConstraint>,
    pub security_level: SecurityLevel,
}

/// Environment constraints that may affect execution
#[derive(Debug, Clone, PartialEq)]
pub enum EnvironmentConstraint {
    /// No root/admin privileges
    NoRoot,
    /// No /proc filesystem access
    NoProcFS,
    /// No /sys filesystem access  
    NoSysFS,
    /// No network access
    NoNetwork,
    /// Restricted binary execution
    RestrictedExec,
    /// SELinux/AppArmor restrictions
    MACRestrictions,
    /// Container/sandbox environment
    Containerized,
    /// Android/Termux environment
    AndroidTermux,
}

/// Security level for operations
#[derive(Debug, Clone)]
pub enum SecurityLevel {
    /// Best effort, any method
    BestEffort,
    /// Verified syscalls only
    SyscallOnly,
    /// Network operations allowed
    NetworkAllowed,
    /// Stealth mode (avoid detection)
    Stealth,
}

/// Re-entrant command definition with multiple execution paths
#[derive(Debug, Clone)]
pub struct ReentrantCommand {
    pub name: String,
    pub description: String,
    pub execution_contexts: Vec<ExecutionContext>,
    pub syscall_impl: Option<SyscallImplementation>,
    pub legacy_impl: Option<LegacyImplementation>,
    pub network_impl: Option<NetworkImplementation>,
    pub completion_provider: Option<CompletionProvider>,
}

/// Syscall-based implementation
#[derive(Debug, Clone)]
pub struct SyscallImplementation {
    pub function_name: String,
    pub required_capabilities: Vec<String>,
    pub platform_variants: HashMap<String, String>, // platform -> implementation
}

/// Legacy binary implementation
#[derive(Debug, Clone)]
pub struct LegacyImplementation {
    pub binary_name: String,
    pub arg_mapping: HashMap<String, String>, // our args -> legacy args
    pub output_parser: Option<String>, // parsing strategy
}

/// Network-based implementation
#[derive(Debug, Clone)]
pub struct NetworkImplementation {
    pub repl_command: String,
    pub http_endpoint: Option<String>,
    pub fallback_ports: Vec<u16>,
}

/// Dynamic completion provider
#[derive(Debug, Clone)]
pub struct CompletionProvider {
    pub static_completions: Vec<String>,
    pub dynamic_function: Option<String>, // function name for dynamic completion
    pub network_completion: bool, // can use network for completion
}

/// Re-entrant DSL registry
pub struct ReentrantDSL {
    commands: HashMap<String, ReentrantCommand>,
    execution_history: Arc<Mutex<Vec<ExecutionAttempt>>>,
    environment_profile: EnvironmentProfile,
}

/// Execution attempt record for learning
#[derive(Debug, Clone)]
pub struct ExecutionAttempt {
    pub command: String,
    pub strategy: ExecutionStrategy,
    pub success: bool,
    pub error_message: Option<String>,
    pub execution_time_ms: u64,
    pub timestamp: u64,
}

/// Environment profile for adaptive execution
#[derive(Debug, Clone)]
pub struct EnvironmentProfile {
    pub detected_constraints: Vec<EnvironmentConstraint>,
    pub working_strategies: HashMap<String, Vec<ExecutionStrategy>>,
    pub failed_strategies: HashMap<String, Vec<ExecutionStrategy>>,
    pub preferred_strategy: ExecutionStrategy,
}

impl ReentrantDSL {
    pub fn new() -> Self {
        let mut dsl = Self {
            commands: HashMap::new(),
            execution_history: Arc::new(Mutex::new(Vec::new())),
            environment_profile: EnvironmentProfile {
                detected_constraints: Vec::new(),
                working_strategies: HashMap::new(),
                failed_strategies: HashMap::new(),
                preferred_strategy: ExecutionStrategy::DirectSyscall,
            },
        };
        
        dsl.register_core_commands();
        dsl.detect_environment();
        dsl
    }

    /// Register core network commands with multiple execution strategies
    fn register_core_commands(&mut self) {
        // Interface listing command
        self.commands.insert("net.interfaces.list".to_string(), ReentrantCommand {
            name: "list".to_string(),
            description: "List network interfaces".to_string(),
            execution_contexts: vec![
                ExecutionContext {
                    primary_strategy: ExecutionStrategy::DirectSyscall,
                    fallback_strategies: vec![
                        ExecutionStrategy::LegacyExec,
                        ExecutionStrategy::NetworkREPL,
                        ExecutionStrategy::ShellFallback,
                    ],
                    environment_constraints: vec![],
                    security_level: SecurityLevel::BestEffort,
                },
                ExecutionContext {
                    primary_strategy: ExecutionStrategy::LegacyExec,
                    fallback_strategies: vec![ExecutionStrategy::ShellFallback],
                    environment_constraints: vec![EnvironmentConstraint::NoProcFS],
                    security_level: SecurityLevel::BestEffort,
                },
            ],
            syscall_impl: Some(SyscallImplementation {
                function_name: "get_interfaces_syscall".to_string(),
                required_capabilities: vec![],
                platform_variants: {
                    let mut variants = HashMap::new();
                    variants.insert("linux".to_string(), "siocgifconf_linux".to_string());
                    variants.insert("macos".to_string(), "siocgifconf_macos".to_string());
                    variants.insert("android".to_string(), "siocgifconf_android".to_string());
                    variants
                },
            }),
            legacy_impl: Some(LegacyImplementation {
                binary_name: "ifconfig".to_string(),
                arg_mapping: {
                    let mut mapping = HashMap::new();
                    mapping.insert("--all".to_string(), "-a".to_string());
                    mapping
                },
                output_parser: Some("ifconfig_parser".to_string()),
            }),
            network_impl: Some(NetworkImplementation {
                repl_command: "ifconfig".to_string(),
                http_endpoint: Some("/api/interfaces".to_string()),
                fallback_ports: vec![8888, 8080, 8000],
            }),
            completion_provider: Some(CompletionProvider {
                static_completions: vec!["--all".to_string(), "--format".to_string()],
                dynamic_function: Some("complete_interfaces".to_string()),
                network_completion: true,
            }),
        });

        // Route listing command
        self.commands.insert("net.routes.list".to_string(), ReentrantCommand {
            name: "list".to_string(),
            description: "Show routing table".to_string(),
            execution_contexts: vec![
                ExecutionContext {
                    primary_strategy: ExecutionStrategy::DirectSyscall,
                    fallback_strategies: vec![
                        ExecutionStrategy::LegacyExec,
                        ExecutionStrategy::NetworkREPL,
                    ],
                    environment_constraints: vec![],
                    security_level: SecurityLevel::BestEffort,
                },
            ],
            syscall_impl: Some(SyscallImplementation {
                function_name: "get_routes_syscall".to_string(),
                required_capabilities: vec!["CAP_NET_ADMIN".to_string()],
                platform_variants: {
                    let mut variants = HashMap::new();
                    variants.insert("linux".to_string(), "netlink_route_dump".to_string());
                    variants.insert("macos".to_string(), "route_sysctl".to_string());
                    variants.insert("android".to_string(), "netlink_route_dump".to_string());
                    variants
                },
            }),
            legacy_impl: Some(LegacyImplementation {
                binary_name: "route".to_string(),
                arg_mapping: HashMap::new(),
                output_parser: Some("route_parser".to_string()),
            }),
            network_impl: Some(NetworkImplementation {
                repl_command: "route".to_string(),
                http_endpoint: Some("/api/routes".to_string()),
                fallback_ports: vec![8888, 8080],
            }),
            completion_provider: Some(CompletionProvider {
                static_completions: vec!["--ipv4".to_string(), "--ipv6".to_string()],
                dynamic_function: None,
                network_completion: false,
            }),
        });

        // Connection stats command
        self.commands.insert("net.stats.connections".to_string(), ReentrantCommand {
            name: "connections".to_string(),
            description: "Show network connections".to_string(),
            execution_contexts: vec![
                ExecutionContext {
                    primary_strategy: ExecutionStrategy::DirectSyscall,
                    fallback_strategies: vec![
                        ExecutionStrategy::LegacyExec,
                        ExecutionStrategy::NetworkREPL,
                        ExecutionStrategy::ShellFallback,
                    ],
                    environment_constraints: vec![],
                    security_level: SecurityLevel::BestEffort,
                },
                ExecutionContext {
                    primary_strategy: ExecutionStrategy::LegacyExec,
                    fallback_strategies: vec![ExecutionStrategy::ShellFallback],
                    environment_constraints: vec![EnvironmentConstraint::NoProcFS],
                    security_level: SecurityLevel::BestEffort,
                },
            ],
            syscall_impl: Some(SyscallImplementation {
                function_name: "get_connections_syscall".to_string(),
                required_capabilities: vec![],
                platform_variants: {
                    let mut variants = HashMap::new();
                    variants.insert("linux".to_string(), "proc_net_tcp".to_string());
                    variants.insert("macos".to_string(), "kinfo_proc".to_string());
                    variants.insert("android".to_string(), "proc_net_tcp".to_string());
                    variants
                },
            }),
            legacy_impl: Some(LegacyImplementation {
                binary_name: "netstat".to_string(),
                arg_mapping: {
                    let mut mapping = HashMap::new();
                    mapping.insert("--tcp".to_string(), "-t".to_string());
                    mapping.insert("--udp".to_string(), "-u".to_string());
                    mapping.insert("--listening".to_string(), "-l".to_string());
                    mapping
                },
                output_parser: Some("netstat_parser".to_string()),
            }),
            network_impl: Some(NetworkImplementation {
                repl_command: "netstat".to_string(),
                http_endpoint: Some("/api/connections".to_string()),
                fallback_ports: vec![8888, 8080],
            }),
            completion_provider: Some(CompletionProvider {
                static_completions: vec!["--tcp".to_string(), "--udp".to_string(), "--listening".to_string()],
                dynamic_function: None,
                network_completion: false,
            }),
        });

        // Proxy server command
        self.commands.insert("proxy.server".to_string(), ReentrantCommand {
            name: "server".to_string(),
            description: "Start proxy server".to_string(),
            execution_contexts: vec![
                ExecutionContext {
                    primary_strategy: ExecutionStrategy::LibraryCall,
                    fallback_strategies: vec![
                        ExecutionStrategy::NetworkREPL,
                        ExecutionStrategy::ShellFallback,
                    ],
                    environment_constraints: vec![],
                    security_level: SecurityLevel::NetworkAllowed,
                },
                ExecutionContext {
                    primary_strategy: ExecutionStrategy::NetworkREPL,
                    fallback_strategies: vec![ExecutionStrategy::SSHTunnel],
                    environment_constraints: vec![EnvironmentConstraint::RestrictedExec],
                    security_level: SecurityLevel::Stealth,
                },
            ],
            syscall_impl: None,
            legacy_impl: None,
            network_impl: Some(NetworkImplementation {
                repl_command: "proxy server".to_string(),
                http_endpoint: Some("/api/proxy/start".to_string()),
                fallback_ports: vec![8888, 8080, 8000, 9090],
            }),
            completion_provider: Some(CompletionProvider {
                static_completions: vec!["--port".to_string(), "--bind".to_string(), "--daemon".to_string()],
                dynamic_function: Some("complete_ports".to_string()),
                network_completion: false,
            }),
        });
    }

    /// Detect environment constraints and capabilities
    fn detect_environment(&mut self) {
        // Check for /proc filesystem
        if !std::path::Path::new("/proc").exists() {
            self.environment_profile.detected_constraints.push(EnvironmentConstraint::NoProcFS);
        }

        // Check for /sys filesystem
        if !std::path::Path::new("/sys").exists() {
            self.environment_profile.detected_constraints.push(EnvironmentConstraint::NoSysFS);
        }

        // Check for root privileges
        if unsafe { libc::getuid() } != 0 {
            self.environment_profile.detected_constraints.push(EnvironmentConstraint::NoRoot);
        }

        // Check for Android/Termux
        if std::env::var("ANDROID_ROOT").is_ok() || std::env::var("TERMUX_VERSION").is_ok() {
            self.environment_profile.detected_constraints.push(EnvironmentConstraint::AndroidTermux);
        }

        // Check for container environment
        if std::path::Path::new("/.dockerenv").exists() || 
           std::env::var("container").is_ok() {
            self.environment_profile.detected_constraints.push(EnvironmentConstraint::Containerized);
        }

        // Adapt preferred strategy based on constraints
        if self.environment_profile.detected_constraints.contains(&EnvironmentConstraint::NoProcFS) ||
           self.environment_profile.detected_constraints.contains(&EnvironmentConstraint::AndroidTermux) {
            self.environment_profile.preferred_strategy = ExecutionStrategy::DirectSyscall;
        }
    }

    /// Execute command with re-entrant fallback strategies
    pub fn execute_command(&mut self, command_path: &str, args: &[String]) -> Result<String, String> {
        // Get command and clone needed data to avoid borrow checker issues
        let command = self.commands.get(command_path)
            .ok_or_else(|| format!("Command not found: {}", command_path))?
            .clone();

        // Find the best execution context for current environment
        let context = self.find_best_execution_context(&command)?.clone();
        
        // Try primary strategy first
        let start_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        match self.try_execution_strategy(&command, &context.primary_strategy, args) {
            Ok(result) => {
                self.record_success(command_path, &context.primary_strategy, start_time);
                return Ok(result);
            }
            Err(e) => {
                self.record_failure(command_path, &context.primary_strategy, &e, start_time);
                
                // Try fallback strategies
                for fallback_strategy in &context.fallback_strategies {
                    let fallback_start = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64;

                    match self.try_execution_strategy(&command, fallback_strategy, args) {
                        Ok(result) => {
                            self.record_success(command_path, fallback_strategy, fallback_start);
                            return Ok(result);
                        }
                        Err(fallback_e) => {
                            self.record_failure(command_path, fallback_strategy, &fallback_e, fallback_start);
                            continue;
                        }
                    }
                }
            }
        }

        Err(format!("All execution strategies failed for command: {}", command_path))
    }

    /// Find the best execution context for current environment
    fn find_best_execution_context<'a>(&self, command: &'a ReentrantCommand) -> Result<&'a ExecutionContext, String> {
        // Score execution contexts based on environment constraints
        let mut best_context = None;
        let mut best_score = -1i32;

        for context in &command.execution_contexts {
            let mut score = 0i32;
            
            // Check if context is compatible with environment constraints
            let compatible = true;
            for constraint in &context.environment_constraints {
                if !self.environment_profile.detected_constraints.contains(constraint) {
                    // Context expects constraint that we don't have - might not be optimal
                    score -= 1;
                } else {
                    // Context handles a constraint we have - good
                    score += 2;
                }
            }

            // Prefer working strategies from history
            let strategy_key = format!("{:?}", context.primary_strategy);
            if let Some(working_strategies) = self.environment_profile.working_strategies.get(&strategy_key) {
                if !working_strategies.is_empty() {
                    score += 5;
                }
            }

            // Avoid failed strategies
            if let Some(failed_strategies) = self.environment_profile.failed_strategies.get(&strategy_key) {
                if !failed_strategies.is_empty() {
                    score -= 3;
                }
            }

            if compatible && score > best_score {
                best_score = score;
                best_context = Some(context);
            }
        }

        best_context.ok_or_else(|| "No compatible execution context found".to_string())
    }

    /// Try a specific execution strategy
    fn try_execution_strategy(
        &self,
        command: &ReentrantCommand,
        strategy: &ExecutionStrategy,
        args: &[String],
    ) -> Result<String, String> {
        match strategy {
            ExecutionStrategy::DirectSyscall => {
                if let Some(ref syscall_impl) = command.syscall_impl {
                    self.execute_syscall(syscall_impl, args)
                } else {
                    Err("No syscall implementation available".to_string())
                }
            }
            ExecutionStrategy::LegacyExec => {
                if let Some(ref legacy_impl) = command.legacy_impl {
                    self.execute_legacy(legacy_impl, args)
                } else {
                    Err("No legacy implementation available".to_string())
                }
            }
            ExecutionStrategy::NetworkREPL => {
                if let Some(ref network_impl) = command.network_impl {
                    self.execute_network_repl(network_impl, args)
                } else {
                    Err("No network implementation available".to_string())
                }
            }
            ExecutionStrategy::ShellFallback => {
                self.execute_shell_fallback(&command.name, args)
            }
            _ => Err(format!("Strategy not implemented: {:?}", strategy)),
        }
    }

    /// Execute via syscall
    fn execute_syscall(&self, impl_: &SyscallImplementation, args: &[String]) -> Result<String, String> {
        // Get platform-specific implementation
        let platform = std::env::consts::OS;
        let func_name = impl_.platform_variants.get(platform)
            .unwrap_or(&impl_.function_name);
        
        // For now, return placeholder - actual syscall implementation would go here
        Ok(format!("Syscall execution: {} with args {:?}", func_name, args))
    }

    /// Execute via legacy binary
    fn execute_legacy(&self, impl_: &LegacyImplementation, args: &[String]) -> Result<String, String> {
        use std::process::Command;
        
        // Map our arguments to legacy arguments
        let mut legacy_args = Vec::new();
        for arg in args {
            if let Some(mapped_arg) = impl_.arg_mapping.get(arg) {
                legacy_args.push(mapped_arg.clone());
            } else {
                legacy_args.push(arg.clone());
            }
        }

        // Execute legacy binary
        match Command::new(&impl_.binary_name).args(&legacy_args).output() {
            Ok(output) => {
                if output.status.success() {
                    Ok(String::from_utf8_lossy(&output.stdout).to_string())
                } else {
                    Err(String::from_utf8_lossy(&output.stderr).to_string())
                }
            }
            Err(e) => Err(format!("Failed to execute {}: {}", impl_.binary_name, e)),
        }
    }

    /// Execute via network REPL
    fn execute_network_repl(&self, impl_: &NetworkImplementation, args: &[String]) -> Result<String, String> {
        // Try different ports until one works
        for port in &impl_.fallback_ports {
            if let Ok(result) = self.try_network_repl_on_port(&impl_.repl_command, args, *port) {
                return Ok(result);
            }
        }
        Err("All network REPL connections failed".to_string())
    }

    /// Try network REPL on specific port
    fn try_network_repl_on_port(&self, command: &str, args: &[String], port: u16) -> Result<String, String> {
        // Placeholder for actual network REPL implementation
        Ok(format!("Network REPL execution on port {}: {} with args {:?}", port, command, args))
    }

    /// Execute via shell fallback
    fn execute_shell_fallback(&self, command: &str, args: &[String]) -> Result<String, String> {
        // Last resort - try to find any working implementation
        let shell_command = format!("{} {}", command, args.join(" "));
        Ok(format!("Shell fallback: {}", shell_command))
    }

    /// Record successful execution
    fn record_success(&mut self, command: &str, strategy: &ExecutionStrategy, start_time: u64) {
        let execution_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64 - start_time;

        let attempt = ExecutionAttempt {
            command: command.to_string(),
            strategy: strategy.clone(),
            success: true,
            error_message: None,
            execution_time_ms: execution_time,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        if let Ok(mut history) = self.execution_history.lock() {
            history.push(attempt);
        }

        // Update working strategies
        self.environment_profile.working_strategies
            .entry(command.to_string())
            .or_insert_with(Vec::new)
            .push(strategy.clone());
    }

    /// Record failed execution
    fn record_failure(&mut self, command: &str, strategy: &ExecutionStrategy, error: &str, start_time: u64) {
        let execution_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64 - start_time;

        let attempt = ExecutionAttempt {
            command: command.to_string(),
            strategy: strategy.clone(),
            success: false,
            error_message: Some(error.to_string()),
            execution_time_ms: execution_time,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        if let Ok(mut history) = self.execution_history.lock() {
            history.push(attempt);
        }

        // Update failed strategies
        self.environment_profile.failed_strategies
            .entry(command.to_string())
            .or_insert_with(Vec::new)
            .push(strategy.clone());
    }

    /// Get completion suggestions for command
    pub fn get_completions(&self, command_path: &str, current_word: &str) -> Vec<String> {
        if let Some(command) = self.commands.get(command_path) {
            if let Some(ref provider) = command.completion_provider {
                let mut completions = provider.static_completions.clone();
                
                // Add dynamic completions if available
                if let Some(ref func_name) = provider.dynamic_function {
                    completions.extend(self.call_dynamic_completion(func_name, current_word));
                }

                // Filter by current word
                completions.into_iter()
                    .filter(|comp| comp.starts_with(current_word))
                    .collect()
            } else {
                vec![]
            }
        } else {
            vec![]
        }
    }

    /// Call dynamic completion function
    fn call_dynamic_completion(&self, func_name: &str, _current_word: &str) -> Vec<String> {
        match func_name {
            "complete_interfaces" => {
                // Would call actual interface enumeration
                vec!["eth0".to_string(), "wlan0".to_string(), "lo".to_string()]
            }
            "complete_ports" => {
                vec!["22".to_string(), "80".to_string(), "443".to_string(), "8080".to_string(), "8888".to_string()]
            }
            _ => vec![],
        }
    }

    /// Generate execution strategy report
    pub fn generate_strategy_report(&self) -> String {
        let mut report = String::new();
        report.push_str("=== Re-entrant DSL Execution Strategy Report ===\n\n");

        report.push_str("Environment Profile:\n");
        report.push_str(&format!("  Preferred Strategy: {:?}\n", self.environment_profile.preferred_strategy));
        report.push_str("  Detected Constraints:\n");
        for constraint in &self.environment_profile.detected_constraints {
            report.push_str(&format!("    - {:?}\n", constraint));
        }

        if let Ok(history) = self.execution_history.lock() {
            report.push_str(&format!("\nExecution History ({} attempts):\n", history.len()));
            
            let mut success_count = 0;
            let mut strategy_stats: HashMap<String, (u32, u32)> = HashMap::new(); // (success, total)
            
            for attempt in history.iter() {
                if attempt.success {
                    success_count += 1;
                }
                
                let strategy_key = format!("{:?}", attempt.strategy);
                let (successes, total) = strategy_stats.entry(strategy_key).or_insert((0, 0));
                if attempt.success {
                    *successes += 1;
                }
                *total += 1;
            }

            report.push_str(&format!("  Overall Success Rate: {:.1}%\n", 
                (success_count as f64 / history.len() as f64) * 100.0));

            report.push_str("  Strategy Success Rates:\n");
            for (strategy, (successes, total)) in strategy_stats {
                let rate = (successes as f64 / total as f64) * 100.0;
                report.push_str(&format!("    {}: {:.1}% ({}/{})\n", strategy, rate, successes, total));
            }
        }

        report.push_str("\nAvailable Pathways by Command:\n");
        for (cmd_path, command) in &self.commands {
            report.push_str(&format!("  {}:\n", cmd_path));
            for context in &command.execution_contexts {
                report.push_str(&format!("    Primary: {:?}\n", context.primary_strategy));
                report.push_str(&format!("    Fallbacks: {:?}\n", context.fallback_strategies));
            }
        }

        report
    }
}
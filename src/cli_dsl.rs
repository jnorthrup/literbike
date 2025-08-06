//! Shared Domain-Specific Language for CLI command definitions
//! This DSL is used by both Rust code and bash completion generation

use std::collections::HashMap;

/// Command definition in the shared DSL
#[derive(Debug, Clone)]
pub struct CommandDef {
    pub name: String,
    pub description: String,
    pub subcommands: Vec<CommandDef>,
    pub options: Vec<OptionDef>,
    pub examples: Vec<String>,
    pub completion_hints: Vec<CompletionHint>,
}

/// Option definition for commands
#[derive(Debug, Clone)]
pub struct OptionDef {
    pub short: Option<char>,
    pub long: String,
    pub description: String,
    pub takes_value: bool,
    pub value_name: Option<String>,
    pub possible_values: Vec<String>,
    pub completion_hint: Option<CompletionHint>,
}

/// Completion hints for dynamic completion
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CompletionHint {
    Files,
    Directories, 
    Interfaces,  // Network interfaces
    Hosts,       // Host addresses
    Ports,       // Port numbers
    Protocols,   // Protocol types
    Custom(String), // Custom completion command
}

impl CommandDef {
    /// Create the root litebike command structure
    pub fn root() -> Self {
        CommandDef {
            name: "litebike".to_string(),
            description: "LiteBike network utility with proxy capabilities".to_string(),
            subcommands: vec![
                Self::net_command(),
                Self::proxy_command(), 
                Self::connect_command(),
                Self::completion_command(),
                Self::utils_command(),
                Self::client_command(),  // Add alias for /client
            ],
            options: vec![
                OptionDef {
                    short: Some('v'),
                    long: "verbose".to_string(),
                    description: "Enable verbose output".to_string(),
                    takes_value: false,
                    value_name: None,
                    possible_values: vec![],
                    completion_hint: None,
                },
                OptionDef {
                    short: Some('h'),
                    long: "help".to_string(),
                    description: "Show help information".to_string(),
                    takes_value: false,
                    value_name: None,
                    possible_values: vec![],
                    completion_hint: None,
                },
                OptionDef {
                    short: Some('V'),
                    long: "version".to_string(),
                    description: "Show version information".to_string(),
                    takes_value: false,
                    value_name: None,
                    possible_values: vec![],
                    completion_hint: None,
                },
            ],
            examples: vec![
                "litebike net interfaces".to_string(),
                "litebike proxy server --port 8080".to_string(),
                "litebike connect repl 192.168.1.1".to_string(),
            ],
            completion_hints: vec![],
        }
    }

    /// Network management commands
    fn net_command() -> Self {
        CommandDef {
            name: "net".to_string(),
            description: "Network interface and routing management".to_string(),
            subcommands: vec![
                CommandDef {
                    name: "interfaces".to_string(),
                    description: "List and manage network interfaces".to_string(),
                    subcommands: vec![
                        CommandDef {
                            name: "list".to_string(),
                            description: "List all network interfaces".to_string(),
                            subcommands: vec![],
                            options: vec![
                                OptionDef {
                                    short: Some('a'),
                                    long: "all".to_string(),
                                    description: "Show all interfaces including inactive".to_string(),
                                    takes_value: false,
                                    value_name: None,
                                    possible_values: vec![],
                                    completion_hint: None,
                                },
                                OptionDef {
                                    short: Some('f'),
                                    long: "format".to_string(),
                                    description: "Output format".to_string(),
                                    takes_value: true,
                                    value_name: Some("FORMAT".to_string()),
                                    possible_values: vec!["table".to_string(), "json".to_string(), "plain".to_string()],
                                    completion_hint: None,
                                },
                            ],
                            examples: vec![
                                "litebike net interfaces list".to_string(),
                                "litebike net interfaces list --all".to_string(),
                                "litebike net interfaces list --format json".to_string(),
                            ],
                            completion_hints: vec![],
                        },
                        CommandDef {
                            name: "show".to_string(),
                            description: "Show details for specific interface".to_string(),
                            subcommands: vec![],
                            options: vec![],
                            examples: vec![
                                "litebike net interfaces show eth0".to_string(),
                                "litebike net interfaces show wlan0".to_string(),
                            ],
                            completion_hints: vec![CompletionHint::Interfaces],
                        },
                    ],
                    options: vec![],
                    examples: vec![],
                    completion_hints: vec![],
                },
                CommandDef {
                    name: "routes".to_string(),
                    description: "Routing table operations".to_string(),
                    subcommands: vec![
                        CommandDef {
                            name: "list".to_string(),
                            description: "Show routing table".to_string(),
                            subcommands: vec![],
                            options: vec![
                                OptionDef {
                                    short: Some('4'),
                                    long: "ipv4".to_string(),
                                    description: "Show IPv4 routes only".to_string(),
                                    takes_value: false,
                                    value_name: None,
                                    possible_values: vec![],
                                    completion_hint: None,
                                },
                                OptionDef {
                                    short: Some('6'),
                                    long: "ipv6".to_string(),
                                    description: "Show IPv6 routes only".to_string(),
                                    takes_value: false,
                                    value_name: None,
                                    possible_values: vec![],
                                    completion_hint: None,
                                },
                            ],
                            examples: vec![
                                "litebike net routes list".to_string(),
                                "litebike net routes list --ipv4".to_string(),
                            ],
                            completion_hints: vec![],
                        },
                        CommandDef {
                            name: "test".to_string(),
                            description: "Test route to destination".to_string(),
                            subcommands: vec![],
                            options: vec![
                                OptionDef {
                                    short: Some('c'),
                                    long: "count".to_string(),
                                    description: "Number of ping packets".to_string(),
                                    takes_value: true,
                                    value_name: Some("COUNT".to_string()),
                                    possible_values: vec![],
                                    completion_hint: None,
                                },
                            ],
                            examples: vec![
                                "litebike net routes test 8.8.8.8".to_string(),
                                "litebike net routes test google.com --count 5".to_string(),
                            ],
                            completion_hints: vec![CompletionHint::Hosts],
                        },
                    ],
                    options: vec![],
                    examples: vec![],
                    completion_hints: vec![],
                },
                CommandDef {
                    name: "stats".to_string(),
                    description: "Network statistics and monitoring".to_string(),
                    subcommands: vec![
                        CommandDef {
                            name: "connections".to_string(),
                            description: "Show active network connections".to_string(),
                            subcommands: vec![],
                            options: vec![
                                OptionDef {
                                    short: Some('l'),
                                    long: "listening".to_string(),
                                    description: "Show only listening ports".to_string(),
                                    takes_value: false,
                                    value_name: None,
                                    possible_values: vec![],
                                    completion_hint: None,
                                },
                                OptionDef {
                                    short: Some('t'),
                                    long: "tcp".to_string(),
                                    description: "Show TCP connections only".to_string(),
                                    takes_value: false,
                                    value_name: None,
                                    possible_values: vec![],
                                    completion_hint: None,
                                },
                                OptionDef {
                                    short: Some('u'),
                                    long: "udp".to_string(),
                                    description: "Show UDP connections only".to_string(),
                                    takes_value: false,
                                    value_name: None,
                                    possible_values: vec![],
                                    completion_hint: None,
                                },
                            ],
                            examples: vec![
                                "litebike net stats connections".to_string(),
                                "litebike net stats connections --listening".to_string(),
                                "litebike net stats connections --tcp".to_string(),
                            ],
                            completion_hints: vec![],
                        },
                    ],
                    options: vec![],
                    examples: vec![],
                    completion_hints: vec![],
                },
                CommandDef {
                    name: "discover".to_string(),
                    description: "Network discovery and scanning".to_string(),
                    subcommands: vec![
                        CommandDef {
                            name: "hosts".to_string(),
                            description: "Discover active hosts on network".to_string(),
                            subcommands: vec![],
                            options: vec![
                                OptionDef {
                                    short: Some('r'),
                                    long: "range".to_string(),
                                    description: "IP range to scan".to_string(),
                                    takes_value: true,
                                    value_name: Some("CIDR".to_string()),
                                    possible_values: vec![],
                                    completion_hint: None,
                                },
                                OptionDef {
                                    short: Some('t'),
                                    long: "timeout".to_string(),
                                    description: "Timeout in milliseconds".to_string(),
                                    takes_value: true,
                                    value_name: Some("MS".to_string()),
                                    possible_values: vec![],
                                    completion_hint: None,
                                },
                            ],
                            examples: vec![
                                "litebike net discover hosts".to_string(),
                                "litebike net discover hosts --range 192.168.1.0/24".to_string(),
                            ],
                            completion_hints: vec![],
                        },
                    ],
                    options: vec![],
                    examples: vec![],
                    completion_hints: vec![],
                },
            ],
            options: vec![],
            examples: vec![],
            completion_hints: vec![],
        }
    }

    /// Proxy and tunnel commands
    fn proxy_command() -> Self {
        CommandDef {
            name: "proxy".to_string(),
            description: "Proxy server and client operations".to_string(),
            subcommands: vec![
                CommandDef {
                    name: "server".to_string(),
                    description: "Start proxy server".to_string(),
                    subcommands: vec![],
                    options: vec![
                        OptionDef {
                            short: Some('p'),
                            long: "port".to_string(),
                            description: "Server port".to_string(),
                            takes_value: true,
                            value_name: Some("PORT".to_string()),
                            possible_values: vec![],
                            completion_hint: Some(CompletionHint::Ports),
                        },
                        OptionDef {
                            short: Some('b'),
                            long: "bind".to_string(),
                            description: "Bind address".to_string(),
                            takes_value: true,
                            value_name: Some("ADDRESS".to_string()),
                            possible_values: vec!["0.0.0.0".to_string(), "127.0.0.1".to_string()],
                            completion_hint: Some(CompletionHint::Hosts),
                        },
                        OptionDef {
                            short: Some('d'),
                            long: "daemon".to_string(),
                            description: "Run as daemon".to_string(),
                            takes_value: false,
                            value_name: None,
                            possible_values: vec![],
                            completion_hint: None,
                        },
                    ],
                    examples: vec![
                        "litebike proxy server".to_string(),
                        "litebike proxy server --port 8080".to_string(),
                        "litebike proxy server --bind 0.0.0.0 --daemon".to_string(),
                    ],
                    completion_hints: vec![],
                },
                CommandDef {
                    name: "client".to_string(),
                    description: "Connect as proxy client".to_string(),
                    subcommands: vec![],
                    options: vec![
                        OptionDef {
                            short: Some('s'),
                            long: "server".to_string(),
                            description: "Proxy server address".to_string(),
                            takes_value: true,
                            value_name: Some("HOST:PORT".to_string()),
                            possible_values: vec![],
                            completion_hint: Some(CompletionHint::Hosts),
                        },
                        OptionDef {
                            short: Some('L'),
                            long: "local-port".to_string(),
                            description: "Local port for forwarding".to_string(),
                            takes_value: true,
                            value_name: Some("PORT".to_string()),
                            possible_values: vec![],
                            completion_hint: Some(CompletionHint::Ports),
                        },
                    ],
                    examples: vec![
                        "litebike proxy client --server 192.168.1.1:8080".to_string(),
                        "litebike proxy client -s 192.168.1.1:8080 -L 1080".to_string(),
                    ],
                    completion_hints: vec![CompletionHint::Hosts],
                },
                CommandDef {
                    name: "socks".to_string(),
                    description: "SOCKS proxy operations".to_string(),
                    subcommands: vec![
                        CommandDef {
                            name: "server".to_string(),
                            description: "Start SOCKS proxy server".to_string(),
                            subcommands: vec![],
                            options: vec![
                                OptionDef {
                                    short: Some('p'),
                                    long: "port".to_string(),
                                    description: "SOCKS server port".to_string(),
                                    takes_value: true,
                                    value_name: Some("PORT".to_string()),
                                    possible_values: vec![],
                                    completion_hint: Some(CompletionHint::Ports),
                                },
                                OptionDef {
                                    short: Some('v'),
                                    long: "version".to_string(),
                                    description: "SOCKS version".to_string(),
                                    takes_value: true,
                                    value_name: Some("VERSION".to_string()),
                                    possible_values: vec!["4".to_string(), "5".to_string()],
                                    completion_hint: None,
                                },
                            ],
                            examples: vec![
                                "litebike proxy socks server".to_string(),
                                "litebike proxy socks server --port 1080 --version 5".to_string(),
                            ],
                            completion_hints: vec![],
                        },
                    ],
                    options: vec![],
                    examples: vec![],
                    completion_hints: vec![],
                },
            ],
            options: vec![],
            examples: vec![],
            completion_hints: vec![],
        }
    }

    /// Connection management commands
    fn connect_command() -> Self {
        CommandDef {
            name: "connect".to_string(),
            description: "Connection management and testing".to_string(),
            subcommands: vec![
                CommandDef {
                    name: "repl".to_string(),
                    description: "Connect to LiteBike REPL server".to_string(),
                    subcommands: vec![],
                    options: vec![
                        OptionDef {
                            short: Some('p'),
                            long: "port".to_string(),
                            description: "REPL server port".to_string(),
                            takes_value: true,
                            value_name: Some("PORT".to_string()),
                            possible_values: vec![],
                            completion_hint: Some(CompletionHint::Ports),
                        },
                    ],
                    examples: vec![
                        "litebike connect repl 192.168.1.1".to_string(),
                        "litebike connect repl 192.168.1.1 --port 8888".to_string(),
                    ],
                    completion_hints: vec![CompletionHint::Hosts],
                },
                CommandDef {
                    name: "ssh".to_string(),
                    description: "SSH connection management".to_string(),
                    subcommands: vec![],
                    options: vec![
                        OptionDef {
                            short: Some('p'),
                            long: "port".to_string(),
                            description: "SSH port".to_string(),
                            takes_value: true,
                            value_name: Some("PORT".to_string()),
                            possible_values: vec![],
                            completion_hint: Some(CompletionHint::Ports),
                        },
                        OptionDef {
                            short: Some('u'),
                            long: "user".to_string(),
                            description: "SSH username".to_string(),
                            takes_value: true,
                            value_name: Some("USER".to_string()),
                            possible_values: vec![],
                            completion_hint: None,
                        },
                    ],
                    examples: vec![
                        "litebike connect ssh 192.168.1.1".to_string(),
                        "litebike connect ssh 192.168.1.1 --user u0_a471 --port 8022".to_string(),
                    ],
                    completion_hints: vec![CompletionHint::Hosts],
                },
                CommandDef {
                    name: "test".to_string(),
                    description: "Test network connectivity".to_string(),
                    subcommands: vec![],
                    options: vec![
                        OptionDef {
                            short: Some('p'),
                            long: "port".to_string(),
                            description: "Port to test".to_string(),
                            takes_value: true,
                            value_name: Some("PORT".to_string()),
                            possible_values: vec![],
                            completion_hint: Some(CompletionHint::Ports),
                        },
                        OptionDef {
                            short: Some('t'),
                            long: "timeout".to_string(),
                            description: "Connection timeout in seconds".to_string(),
                            takes_value: true,
                            value_name: Some("SECONDS".to_string()),
                            possible_values: vec![],
                            completion_hint: None,
                        },
                    ],
                    examples: vec![
                        "litebike connect test 8.8.8.8".to_string(),
                        "litebike connect test 192.168.1.1 --port 22".to_string(),
                    ],
                    completion_hints: vec![CompletionHint::Hosts],
                },
            ],
            options: vec![],
            examples: vec![],
            completion_hints: vec![],
        }
    }

    /// Bash completion commands
    fn completion_command() -> Self {
        CommandDef {
            name: "completion".to_string(),
            description: "Bash completion utilities".to_string(),
            subcommands: vec![
                CommandDef {
                    name: "generate".to_string(),
                    description: "Generate bash completion script".to_string(),
                    subcommands: vec![],
                    options: vec![
                        OptionDef {
                            short: Some('s'),
                            long: "shell".to_string(),
                            description: "Target shell".to_string(),
                            takes_value: true,
                            value_name: Some("SHELL".to_string()),
                            possible_values: vec!["bash".to_string(), "zsh".to_string(), "fish".to_string()],
                            completion_hint: None,
                        },
                    ],
                    examples: vec![
                        "litebike completion generate".to_string(),
                        "litebike completion generate --shell bash".to_string(),
                    ],
                    completion_hints: vec![],
                },
                CommandDef {
                    name: "install".to_string(),
                    description: "Install completion script to system".to_string(),
                    subcommands: vec![],
                    options: vec![],
                    examples: vec![
                        "litebike completion install".to_string(),
                    ],
                    completion_hints: vec![],
                },
            ],
            options: vec![],
            examples: vec![],
            completion_hints: vec![],
        }
    }

    /// Client command (alias for proxy client with auto-detection)
    fn client_command() -> Self {
        CommandDef {
            name: "/client".to_string(),
            description: "Client connection with auto-detection capabilities".to_string(),
            subcommands: vec![
                CommandDef {
                    name: "auto".to_string(),
                    description: "Auto-detect proxy servers and connect".to_string(),
                    subcommands: vec![],
                    options: vec![
                        OptionDef {
                            short: Some('p'),
                            long: "port".to_string(),
                            description: "Preferred port for proxy connection".to_string(),
                            takes_value: true,
                            value_name: Some("PORT".to_string()),
                            possible_values: vec![],
                            completion_hint: Some(CompletionHint::Ports),
                        },
                        OptionDef {
                            short: Some('t'),
                            long: "timeout".to_string(),
                            description: "Discovery timeout in seconds".to_string(),
                            takes_value: true,
                            value_name: Some("SECONDS".to_string()),
                            possible_values: vec![],
                            completion_hint: None,
                        },
                        OptionDef {
                            short: Some('r'),
                            long: "range".to_string(),
                            description: "IP range to scan for proxy servers".to_string(),
                            takes_value: true,
                            value_name: Some("CIDR".to_string()),
                            possible_values: vec![],
                            completion_hint: None,
                        },
                        OptionDef {
                            short: None,
                            long: "ssh-user".to_string(),
                            description: "SSH username for gateway connection".to_string(),
                            takes_value: true,
                            value_name: Some("USERNAME".to_string()),
                            possible_values: vec![],
                            completion_hint: None,
                        },
                        OptionDef {
                            short: None,
                            long: "ssh-key".to_string(),
                            description: "SSH private key file path".to_string(),
                            takes_value: true,
                            value_name: Some("PATH".to_string()),
                            possible_values: vec![],
                            completion_hint: Some(CompletionHint::Files),
                        },
                        OptionDef {
                            short: None,
                            long: "ssh-port".to_string(),
                            description: "SSH port (default: 22)".to_string(),
                            takes_value: true,
                            value_name: Some("PORT".to_string()),
                            possible_values: vec![],
                            completion_hint: Some(CompletionHint::Ports),
                        },
                        OptionDef {
                            short: None,
                            long: "ssh-tunnel".to_string(),
                            description: "Enable SSH tunneling through gateway".to_string(),
                            takes_value: false,
                            value_name: None,
                            possible_values: vec![],
                            completion_hint: None,
                        },
                        OptionDef {
                            short: None,
                            long: "ssh-local-port".to_string(),
                            description: "Local port for SSH tunnel".to_string(),
                            takes_value: true,
                            value_name: Some("PORT".to_string()),
                            possible_values: vec![],
                            completion_hint: Some(CompletionHint::Ports),
                        },
                    ],
                    examples: vec![
                        "litebike /client auto".to_string(),
                        "litebike /client auto --port 8080".to_string(),
                        "litebike /client auto --range 192.168.1.0/24".to_string(),
                        "litebike /client auto --ssh-tunnel --ssh-user root".to_string(),
                        "litebike /client auto --ssh-tunnel --ssh-user admin --ssh-key ~/.ssh/id_rsa".to_string(),
                        "litebike /client auto --ssh-tunnel --ssh-local-port 8081".to_string(),
                    ],
                    completion_hints: vec![],
                },
                CommandDef {
                    name: "connect".to_string(),
                    description: "Connect to specific proxy server".to_string(),
                    subcommands: vec![],
                    options: vec![
                        OptionDef {
                            short: Some('s'),
                            long: "server".to_string(),
                            description: "Proxy server address".to_string(),
                            takes_value: true,
                            value_name: Some("HOST:PORT".to_string()),
                            possible_values: vec![],
                            completion_hint: Some(CompletionHint::Hosts),
                        },
                    ],
                    examples: vec![
                        "litebike /client connect --server 192.168.1.1:8080".to_string(),
                    ],
                    completion_hints: vec![CompletionHint::Hosts],
                },
            ],
            options: vec![],
            examples: vec![],
            completion_hints: vec![],
        }
    }

    /// Legacy utility compatibility commands
    fn utils_command() -> Self {
        CommandDef {
            name: "utils".to_string(),
            description: "Legacy network utility compatibility".to_string(),
            subcommands: vec![
                CommandDef {
                    name: "ifconfig".to_string(),
                    description: "Interface configuration (ifconfig compatibility)".to_string(),
                    subcommands: vec![],
                    options: vec![
                        OptionDef {
                            short: Some('a'),
                            long: "all".to_string(),
                            description: "Show all interfaces".to_string(),
                            takes_value: false,
                            value_name: None,
                            possible_values: vec![],
                            completion_hint: None,
                        },
                    ],
                    examples: vec![
                        "litebike utils ifconfig".to_string(),
                        "litebike utils ifconfig --all".to_string(),
                        "litebike utils ifconfig eth0".to_string(),
                    ],
                    completion_hints: vec![CompletionHint::Interfaces],
                },
                CommandDef {
                    name: "netstat".to_string(),
                    description: "Network statistics (netstat compatibility)".to_string(),
                    subcommands: vec![],
                    options: vec![
                        OptionDef {
                            short: Some('l'),
                            long: "listening".to_string(),
                            description: "Show listening ports only".to_string(),
                            takes_value: false,
                            value_name: None,
                            possible_values: vec![],
                            completion_hint: None,
                        },
                        OptionDef {
                            short: Some('a'),
                            long: "all".to_string(),
                            description: "Show all connections".to_string(),
                            takes_value: false,
                            value_name: None,
                            possible_values: vec![],
                            completion_hint: None,
                        },
                    ],
                    examples: vec![
                        "litebike utils netstat".to_string(),
                        "litebike utils netstat --listening".to_string(),
                        "litebike utils netstat --all".to_string(),
                    ],
                    completion_hints: vec![],
                },
                CommandDef {
                    name: "route".to_string(),
                    description: "Routing table display (route compatibility)".to_string(),
                    subcommands: vec![],
                    options: vec![],
                    examples: vec![
                        "litebike utils route".to_string(),
                    ],
                    completion_hints: vec![],
                },
                CommandDef {
                    name: "ip".to_string(),
                    description: "IP configuration (ip command compatibility)".to_string(),
                    subcommands: vec![
                        CommandDef {
                            name: "addr".to_string(),
                            description: "Address management".to_string(),
                            subcommands: vec![],
                            options: vec![],
                            examples: vec![
                                "litebike utils ip addr".to_string(),
                            ],
                            completion_hints: vec![],
                        },
                        CommandDef {
                            name: "route".to_string(),
                            description: "Route management".to_string(),
                            subcommands: vec![],
                            options: vec![],
                            examples: vec![
                                "litebike utils ip route".to_string(),
                            ],
                            completion_hints: vec![],
                        },
                    ],
                    options: vec![],
                    examples: vec![],
                    completion_hints: vec![],
                },
            ],
            options: vec![],
            examples: vec![],
            completion_hints: vec![],
        }
    }

    /// Find a command by path (e.g., ["net", "interfaces", "list"])
    pub fn find_command(&self, path: &[String]) -> Option<&CommandDef> {
        if path.is_empty() {
            return Some(self);
        }

        let first = &path[0];
        if self.name == *first {
            if path.len() == 1 {
                return Some(self);
            }
            return self.find_command(&path[1..]);
        }

        for subcommand in &self.subcommands {
            if subcommand.name == *first {
                if path.len() == 1 {
                    return Some(subcommand);
                }
                return subcommand.find_command(&path[1..]);
            }
        }

        None
    }

    /// Get all possible completions for the given path
    pub fn get_completions(&self, path: &[String], current_word: &str) -> Vec<String> {
        if path.is_empty() {
            // Complete subcommands at root level
            return self.subcommands
                .iter()
                .filter(|cmd| cmd.name.starts_with(current_word))
                .map(|cmd| cmd.name.clone())
                .collect();
        }

        if let Some(cmd) = self.find_command(&path[..path.len()-1]) {
            let mut completions = Vec::new();
            
            // Add subcommand completions
            completions.extend(
                cmd.subcommands
                    .iter()
                    .filter(|subcmd| subcmd.name.starts_with(current_word))
                    .map(|subcmd| subcmd.name.clone())
            );

            // Add option completions
            completions.extend(
                cmd.options
                    .iter()
                    .filter_map(|opt| {
                        if current_word.starts_with("--") && opt.long.starts_with(&current_word[2..]) {
                            Some(format!("--{}", opt.long))
                        } else if current_word.starts_with("-") && current_word.len() == 2 {
                            if let Some(short) = opt.short {
                                if current_word.chars().nth(1) == Some(short) {
                                    Some(format!("-{}", short))
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    })
            );

            completions
        } else {
            vec![]
        }
    }

    /// Generate bash completion script
    pub fn generate_bash_completion() -> String {
        let _root = Self::root();
        format!(
            r#"#!/bin/bash
# LiteBike bash completion script
# Generated automatically - do not edit manually

_litebike_completion() {{
    local cur prev words cword
    _init_completion || return

    local cmd="$1"
    local subcmd=""
    local subsubcmd=""

    # Parse command structure
    if [[ ${{#words[@]}} -gt 2 ]]; then
        subcmd="${{words[2]}}"
    fi
    if [[ ${{#words[@]}} -gt 3 ]]; then
        subsubcmd="${{words[3]}}"
    fi

    # Call litebike for dynamic completion
    local completions
    completions=$($cmd completion _internal "${{words[@]}}")
    
    if [[ -n "$completions" ]]; then
        COMPREPLY=($(compgen -W "$completions" -- "$cur"))
    fi

    return 0
}}

# Register completion for litebike
complete -F _litebike_completion litebike

# Register completion for legacy utilities if they exist as symlinks
if [[ -L $(which ifconfig 2>/dev/null) ]]; then
    complete -F _litebike_completion ifconfig
fi
if [[ -L $(which netstat 2>/dev/null) ]]; then
    complete -F _litebike_completion netstat  
fi
if [[ -L $(which route 2>/dev/null) ]]; then
    complete -F _litebike_completion route
fi
if [[ -L $(which ip 2>/dev/null) ]]; then
    complete -F _litebike_completion ip
fi
"#
        )
    }
}

/// Registry for dynamic completion providers
pub struct CompletionRegistry {
    providers: HashMap<CompletionHint, Box<dyn Fn() -> Vec<String>>>,
}

impl CompletionRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            providers: HashMap::new(),
        };
        
        // Register default completion providers
        registry.register_default_providers();
        registry
    }

    fn register_default_providers(&mut self) {
        // Network interfaces completion - dynamically discover interfaces
        self.providers.insert(
            CompletionHint::Interfaces,
            Box::new(|| {
                use crate::syscall_netops::SyscallNetOps;
                if let Ok(interfaces) = SyscallNetOps::discover_interfaces() {
                    interfaces.into_iter()
                        .filter(|iface| iface.is_up && !iface.is_loopback)
                        .map(|iface| iface.name)
                        .collect()
                } else {
                    // Fallback to common interface names if syscall discovery fails
                    vec!["lo".to_string(), "lo0".to_string(), "eth0".to_string(), 
                         "wlan0".to_string(), "en0".to_string(), "swlan0".to_string()]
                }
            })
        );

        // Common hosts completion
        self.providers.insert(
            CompletionHint::Hosts,
            Box::new(|| {
                vec![
                    "localhost".to_string(),
                    "127.0.0.1".to_string(),
                    "192.168.1.1".to_string(),
                    "8.8.8.8".to_string(),
                ]
            })
        );

        // Common ports completion
        self.providers.insert(
            CompletionHint::Ports,
            Box::new(|| {
                vec![
                    "22".to_string(),    // SSH
                    "80".to_string(),    // HTTP
                    "443".to_string(),   // HTTPS
                    "1080".to_string(),  // SOCKS
                    "8080".to_string(),  // HTTP Alt
                    "8888".to_string(),  // LiteBike default
                ]
            })
        );
    }

    pub fn get_completions(&self, hint: &CompletionHint) -> Vec<String> {
        if let Some(provider) = self.providers.get(hint) {
            provider()
        } else {
            vec![]
        }
    }
}
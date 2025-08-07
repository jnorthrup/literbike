//! HTTP REPL Handler for Symmetric LiteBike Proxy
//! Provides remote access to network utilities via HTTP endpoints
//! Integrates with existing netutils implementation for consistency

use std::io::{self};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use log::{debug, warn};

/// REPL command executor that uses existing netutils
pub struct ReplHandler {
    netutils_path: String,
}

impl ReplHandler {
    pub fn new() -> Self {
        Self {
            // Use the existing netutils binary from the same package
            netutils_path: std::env::current_exe()
                .unwrap_or_else(|_| "netutils".into())
                .parent()
                .map(|p| p.join("netutils").to_string_lossy().to_string())
                .unwrap_or_else(|| "netutils".to_string()),
        }
    }
    
    /// Handle REPL HTTP request
    pub async fn handle_repl_request<S>(&self, mut stream: S, request_body: &str) -> io::Result<()>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        debug!("Processing REPL request: {}", request_body);
        
        // Parse JSON-like request (simple parser for our known format)
        let command_info = self.parse_command_request(request_body)?;
        let output = self.execute_command(&command_info.command, &command_info.args).await?;
        
        // Send HTTP response
        let response_body = output;
        let response = format!(
            "HTTP/1.1 200 OK\r\n\
             Content-Type: text/plain\r\n\
             Content-Length: {}\r\n\
             Connection: close\r\n\
             \r\n\
             {}",
            response_body.len(),
            response_body
        );
        
        stream.write_all(response.as_bytes()).await?;
        Ok(())
    }
    
    /// Execute network utility command
    async fn execute_command(&self, command: &str, args: &[String]) -> io::Result<String> {
        match command {
            "ping" => {
                Ok("PONG - LiteBike REPL is active".to_string())
            }
            "ifconfig" => {
                self.execute_netutil_command("ifconfig", args).await
            }
            "netstat" => {
                self.execute_netutil_command("netstat", args).await
            }
            "route" => {
                self.execute_netutil_command("route", args).await
            }
            "ip" => {
                self.execute_netutil_command("ip", args).await
            }
            "help" => {
                Ok(format!(
                    "LiteBike REPL Server Commands:\n\
                     ifconfig       - Show network interfaces (syscall-based)\n\
                     netstat        - Show network connections (limited)\n\
                     route          - Show routing table (syscall-based)\n\
                     ip [addr|route] - Show IP configuration (syscall-based)\n\
                     ping           - Test REPL connectivity\n\
                     help           - Show this help message\n\
                     \n\
                     Note: All commands use direct syscalls for Android/Termux compatibility"
                ))
            }
            _ => {
                Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("Unknown command: {}", command)
                ))
            }
        }
    }
    
    /// Execute netutils command using the existing binary
    async fn execute_netutil_command(&self, cmd: &str, args: &[String]) -> io::Result<String> {
        debug!("Executing netutil command: {} with args: {:?}", cmd, args);
        
        // Create symlink name based on command
        let netutils_dir = std::path::Path::new(&self.netutils_path)
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."));
        
        let cmd_path = netutils_dir.join(cmd);
        
        // Try to execute via symlink first, fallback to netutils with argv[0] manipulation
        let output = if cmd_path.exists() {
            // Use symlink if available
            tokio::process::Command::new(&cmd_path)
                .args(args)
                .output()
                .await?
        } else {
            // Fallback: execute netutils directly (it will detect command from argv[0])
            // Create a temporary symlink or use environment variable
            std::env::set_var("NETUTILS_CMD", cmd);
            tokio::process::Command::new(&self.netutils_path)
                .args(args)
                .output()
                .await?
        };
        
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("Command {} failed: {}", cmd, stderr);
            Ok(format!("Command failed: {}", stderr))
        }
    }
    
    /// Simple JSON-like parser for command requests
    fn parse_command_request(&self, body: &str) -> io::Result<CommandRequest> {
        // Simple parser for {"command": "cmd", "args": ["arg1", "arg2"]}
        let mut command = String::new();
        let mut args = Vec::new();
        
        // Find command
        if let Some(cmd_start) = body.find("\"command\"") {
            if let Some(colon_pos) = body[cmd_start..].find(':') {
                let after_colon = &body[cmd_start + colon_pos + 1..];
                if let Some(quote_start) = after_colon.find('"') {
                    let after_quote = &after_colon[quote_start + 1..];
                    if let Some(quote_end) = after_quote.find('"') {
                        command = after_quote[..quote_end].to_string();
                    }
                }
            }
        }
        
        // Find args array
        if let Some(args_start) = body.find("\"args\"") {
            if let Some(colon_pos) = body[args_start..].find(':') {
                let after_colon = &body[args_start + colon_pos + 1..];
                if let Some(bracket_start) = after_colon.find('[') {
                    if let Some(bracket_end) = after_colon.find(']') {
                        let args_content = &after_colon[bracket_start + 1..bracket_end];
                        // Simple parsing of quoted strings
                        let mut in_quote = false;
                        let mut current_arg = String::new();
                        let mut chars = args_content.chars().peekable();
                        
                        while let Some(ch) = chars.next() {
                            match ch {
                                '"' => {
                                    if in_quote {
                                        if !current_arg.is_empty() {
                                            args.push(current_arg.clone());
                                            current_arg.clear();
                                        }
                                        in_quote = false;
                                    } else {
                                        in_quote = true;
                                    }
                                }
                                '\\' if in_quote => {
                                    if let Some(next_ch) = chars.next() {
                                        current_arg.push(next_ch);
                                    }
                                }
                                ch if in_quote => {
                                    current_arg.push(ch);
                                }
                                _ => {} // Ignore whitespace and commas outside quotes
                            }
                        }
                    }
                }
            }
        }
        
        if command.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "No command found in request"
            ));
        }
        
        Ok(CommandRequest { command, args })
    }
}

/// Parsed command request
#[derive(Debug)]
struct CommandRequest {
    command: String,
    args: Vec<String>,
}

impl Default for ReplHandler {
    fn default() -> Self {
        Self::new()
    }
}

mod tests {
    use super::ReplHandler;
    
    #[test]
    fn test_parse_command_request() {
        let handler = ReplHandler::new();
        
        // Test simple command
        let result = handler.parse_command_request(r#"{"command": "ifconfig", "args": []}"#).unwrap();
        assert_eq!(result.command, "ifconfig");
        assert_eq!(result.args.len(), 0);
        
        // Test command with args
        let result = handler.parse_command_request(r#"{"command": "ip", "args": ["addr", "show"]}"#).unwrap();
        assert_eq!(result.command, "ip");
        assert_eq!(result.args, vec!["addr", "show"]);
        
        // Test ping command
        let result = handler.parse_command_request(r#"{"command": "ping", "args": []}"#).unwrap();
        assert_eq!(result.command, "ping");
        assert_eq!(result.args.len(), 0);
    }
}
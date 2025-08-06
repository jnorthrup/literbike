
#[cfg(feature = "auto-discovery")]
pub mod pac;
#[cfg(feature = "auto-discovery")]
pub mod bonjour;
#[cfg(feature = "upnp")]
pub mod upnp;
#[cfg(feature = "auto-discovery")]
pub mod auto_discovery;
pub mod types;
pub mod note20_features;
pub mod unified_handler;
pub mod universal_listener;
pub mod protocol_registry;
pub mod protocol_handlers;
pub mod simple_routing;
pub mod unified_protocol_manager;
pub mod detection_orchestrator;
pub mod socks5_channels;
pub mod socks5_channelized_handler;
// Testing and mock modules
pub mod protocol_mocks;
pub mod simple_torture_test;
pub mod abstractions;
pub mod stubs;
pub mod libc_socket_tune;
pub mod libc_listener;
pub mod libc_logger;
pub mod libc_random;
pub mod libc_base64;
pub mod repl_handler;
// CLI and DSL modules
pub mod cli_dsl;
pub mod cli_core;
pub mod reentrant_dsl;
// Syscall-based network operations
pub mod syscall_netops;
// SSH client functionality
pub mod ssh_client;
// Multi-egress backoff logic
pub mod egress_backoff;
pub mod egress_connector;
pub mod protocol_detector;
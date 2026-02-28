use crate::concurrency::ccek::ContextElement;
use crate::impl_context_element;
use crate::quic::tls::TlsTerminator;
use async_channel::{Receiver, Sender};
use std::sync::Arc;

/// Commands that can be sent over the TLS CCEK channel
pub enum TlsCommand {
    /// Retrieve the current TLS configuration
    GetConfig {
        reply_to: tokio::sync::oneshot::Sender<Arc<rustls::ServerConfig>>,
    },
    /// Provide a whole new terminator to swap config dynamically
    ReloadConfig {
        terminator: TlsTerminator,
        reply_to: tokio::sync::oneshot::Sender<Result<(), String>>,
    },
}

/// The CCek Channelized TLS configuration service
#[derive(Clone)]
pub struct TlsCcekService {
    pub config: Arc<rustls::ServerConfig>,
    pub sender: Sender<TlsCommand>,
    pub receiver: Receiver<TlsCommand>,
}

impl_context_element!(TlsCcekService, "TlsCcekService");

impl TlsCcekService {
    /// Create a new channelized TLS service from an existing terminator
    pub fn new(terminator: TlsTerminator, buffer: usize) -> Self {
        let (sender, receiver) = async_channel::bounded(buffer);
        Self {
            config: terminator.server_config(),
            sender,
            receiver,
        }
    }

    /// Helper to create a loop runner for handling commands asynchronously.
    /// In a real architecture, you'd spawn this on a tokio/async-std task.
    pub async fn run_command_loop(mut self) {
        while let Ok(cmd) = self.receiver.recv().await {
            match cmd {
                TlsCommand::GetConfig { reply_to } => {
                    let _ = reply_to.send(self.config.clone());
                }
                TlsCommand::ReloadConfig { terminator, reply_to } => {
                    self.config = terminator.server_config();
                    let _ = reply_to.send(Ok(()));
                }
            }
        }
    }
}

use crate::compat::ContextElement;
use crate::tls::TlsTerminator;
use async_channel::{Receiver, Sender};
use std::sync::Arc;

/// Commands that can be sent over the TLS CCEK channel
pub enum TlsCommand {
    GetConfig {
        reply_to: tokio::sync::oneshot::Sender<Arc<rustls::ServerConfig>>,
    },
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
    pub fn new(terminator: TlsTerminator, buffer: usize) -> Self {
        let (sender, receiver) = async_channel::bounded(buffer);
        Self {
            config: terminator.server_config(),
            sender,
            receiver,
        }
    }

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

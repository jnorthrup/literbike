use super::packet_protection::{QuicAeadAlgorithm, QuicCryptoState};
use super::secrets::{derive_initial_secrets, derive_packet_protection_keys};
use crate::quic::quic_crypto::{
    CryptoFrameDisposition, HandshakePhase, InboundHeaderProtectionContext,
    OutboundHeaderProtectionContext, QuicCryptoProvider,
};
use crate::quic::quic_error::{ProtocolError, QuicError};
use crate::quic::quic_protocol::{CryptoFrame, QuicConnectionState, QuicHeader};
use parking_lot::Mutex;
use ring::hkdf;
use rustls::quic::Connection;
use std::sync::Arc;

pub struct RustlsCryptoProvider {
    conn: Mutex<Connection>,
    
    // Derived states
    pub initial_state: Option<QuicCryptoState>,
    pub handshake_state: Mutex<Option<QuicCryptoState>>,
    pub onertt_state: Mutex<Option<QuicCryptoState>>,
    
    phase: Mutex<HandshakePhase>,
}

impl RustlsCryptoProvider {
    pub fn new_server(
        rustls_conn: Connection,
        client_dst_conn_id: &[u8],
    ) -> Result<Self, ring::error::Unspecified> {
        let (client_initial, server_initial) = derive_initial_secrets(client_dst_conn_id);
        
        // As a server, we read with client_initial and write with server_initial
        // But for QuicEngine, it currently just uses one engine. We need to store
        // both if we want to fully support read/write symmetrically in the provider,
        // but let's assume one QuicCryptoState for now (we'll expand if needed).
        let (key, iv, hp) = derive_packet_protection_keys(&server_initial);
        
        let initial_state = QuicCryptoState::new(
            QuicAeadAlgorithm::Aes128Gcm, // Initial always AES-128-GCM
            &key,
            iv,
            &hp,
        )?;

        Ok(Self {
            conn: Mutex::new(rustls_conn),
            initial_state: Some(initial_state),
            handshake_state: Mutex::new(None),
            onertt_state: Mutex::new(None),
            phase: Mutex::new(HandshakePhase::Initial),
        })
    }
}

impl QuicCryptoProvider for RustlsCryptoProvider {
    fn on_inbound_header(
        &self,
        header: &mut QuicHeader,
        ctx: &InboundHeaderProtectionContext,
    ) -> Result<(), QuicError> {
        // Here we will use the correct QuicCryptoState to remove header protection
        // depending on the packet type (Initial, Handshake, 1-RTT).
        Ok(())
    }

    fn on_outbound_header(
        &self,
        header: &mut QuicHeader,
        ctx: &OutboundHeaderProtectionContext,
    ) -> Result<(), QuicError> {
        // Apply header protection
        Ok(())
    }

    fn on_crypto_frame(
        &self,
        frame: &CryptoFrame,
        state: &QuicConnectionState,
    ) -> Result<CryptoFrameDisposition, QuicError> {
        let mut conn = self.conn.lock();
        
        // Pass the frame payload into rustls
        // (rustls::quic handles the TLS state machine inside CRYPTO frames)
        if let Err(e) = conn.read_hs(&frame.data) {
            return Err(QuicError::Protocol(ProtocolError::Crypto(
                format!("rustls error: {:?}", e),
                None,
            )));
        }
        
        let mut phase = self.phase.lock();
        if *phase == HandshakePhase::Initial {
            *phase = HandshakePhase::Handshaking;
        }

        Ok(CryptoFrameDisposition::ProgressedHandshake)
    }

    fn handshake_phase(&self) -> HandshakePhase {
        *self.phase.lock()
    }

    fn header_protection_ready(&self) -> bool {
        self.initial_state.is_some()
    }
}

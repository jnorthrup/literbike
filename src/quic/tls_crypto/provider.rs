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
use std::collections::BTreeMap;
use std::sync::Arc;

pub struct RustlsCryptoProvider {
    conn: Mutex<Connection>,

    // Derived states
    pub initial_state: Option<QuicCryptoState>,
    pub handshake_state: Mutex<Option<QuicCryptoState>>,
    pub onertt_state: Mutex<Option<QuicCryptoState>>,

    phase: Mutex<HandshakePhase>,

    /// Reassembly buffer for CRYPTO stream data: offset → bytes.
    /// Tracks which bytes have been delivered to rustls so retransmits and
    /// out-of-order frames are handled correctly.
    crypto_rx_buf: Mutex<BTreeMap<u64, Vec<u8>>>,
    /// Next byte offset expected by rustls (contiguous delivered so far).
    crypto_rx_next: Mutex<u64>,
    /// Set to true after rustls returns a fatal error; prevents further read_hs calls.
    crypto_failed: Mutex<bool>,
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
            crypto_rx_buf: Mutex::new(BTreeMap::new()),
            crypto_rx_next: Mutex::new(0),
            crypto_failed: Mutex::new(false),
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
        _state: &QuicConnectionState,
    ) -> Result<CryptoFrameDisposition, QuicError> {
        // Bail out immediately if rustls previously returned a fatal error.
        if *self.crypto_failed.lock() {
            return Ok(CryptoFrameDisposition::AckOnly);
        }

        // --- Reassembly ---
        // Insert this fragment into the offset-keyed buffer, then drain
        // all contiguous bytes starting at crypto_rx_next into rustls.
        {
            let mut buf = self.crypto_rx_buf.lock();
            let mut next = self.crypto_rx_next.lock();

            let frame_end = frame.offset + frame.data.len() as u64;

            // Skip frames whose data has been fully delivered already.
            if frame_end <= *next {
                return Ok(CryptoFrameDisposition::ProgressedHandshake);
            }

            // Insert (possibly trimming the already-delivered prefix).
            let trimmed_offset = frame.offset.max(*next);
            let trim = (trimmed_offset - frame.offset) as usize;
            buf.entry(trimmed_offset)
                .or_insert_with(|| frame.data[trim..].to_vec());

            // Drain contiguous range into rustls.
            let mut conn = self.conn.lock();
            while let Some((&off, _)) = buf.first_key_value() {
                if off != *next {
                    break; // gap — wait for missing fragment
                }
                let data = buf.pop_first().unwrap().1;
                let advance = data.len() as u64;
                if let Err(e) = conn.read_hs(&data) {
                    *self.crypto_failed.lock() = true;
                    return Err(QuicError::Protocol(ProtocolError::Crypto(
                        format!("rustls error: {:?}", e),
                        None,
                    )));
                }
                *next += advance;
            }
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

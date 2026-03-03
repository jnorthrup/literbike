#[cfg(feature = "quic-crypto")]
use super::quic_error::ProtocolError;
use super::quic_error::QuicError;
#[cfg(feature = "quic-crypto")]
use super::quic_protocol::ConnectionState;
use super::quic_protocol::{CryptoFrame, QuicConnectionState, QuicHeader};
#[cfg(feature = "quic-crypto")]
use parking_lot::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncryptionLevel {
    Initial,
    Handshake,
    OneRtt,
}

#[derive(Debug, Clone)]
pub struct CryptoWrite {
    pub level: EncryptionLevel,
    pub data: Vec<u8>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum HandshakePhase {
    Initial,
    Handshaking,
    OneRtt,
    Closed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InboundHeaderProtectionContext {
    pub expected_packet_number: u64,
    pub truncated_packet_number: u64,
    pub packet_number_len: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutboundHeaderProtectionContext {
    pub packet_number: u64,
    pub packet_number_len: usize,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CryptoFrameDisposition {
    AckOnly,
    ProgressedHandshake,
}

pub trait QuicCryptoProvider: Send + Sync {
    fn on_inbound_header(
        &self,
        _header: &mut QuicHeader,
        _ctx: &InboundHeaderProtectionContext,
    ) -> Result<(), QuicError> {
        Ok(())
    }

    fn on_outbound_header(
        &self,
        _header: &mut QuicHeader,
        _ctx: &OutboundHeaderProtectionContext,
    ) -> Result<(), QuicError> {
        Ok(())
    }

    fn on_crypto_frame(
        &self,
        _frame: &CryptoFrame,
        _level: EncryptionLevel,
        _state: &QuicConnectionState,
    ) -> Result<CryptoFrameDisposition, QuicError> {
        Ok(CryptoFrameDisposition::AckOnly)
    }

    fn handshake_phase(&self) -> HandshakePhase {
        HandshakePhase::Initial
    }

    fn header_protection_ready(&self) -> bool {
        false
    }

    /// Check if TLS handshake is complete (client Finished received, 1-RTT keys available)
    fn handshake_complete(&self) -> bool {
        matches!(self.handshake_phase(), HandshakePhase::OneRtt)
    }

    fn drain_crypto_writes(&self) -> Vec<CryptoWrite> {
        vec![]
    }

    fn encrypt_packet(
        &self,
        _level: EncryptionLevel,
        _pn: u64,
        _header: &[u8],
        _payload: &mut Vec<u8>,
    ) -> Result<(), QuicError> {
        Ok(())
    }

    fn apply_header_protection(
        &self,
        _level: EncryptionLevel,
        _sample: &[u8],
        _first: &mut u8,
        _pn_bytes: &mut [u8],
    ) -> Result<(), QuicError> {
        Ok(())
    }

    /// Remove header protection from an inbound packet (modifies first and pn_bytes in-place).
    fn remove_header_protection(
        &self,
        _level: EncryptionLevel,
        _sample: &[u8],
        _first: &mut u8,
        _pn_bytes: &mut [u8],
    ) -> Result<(), QuicError> {
        Err(QuicError::Protocol(crate::quic::quic_error::ProtocolError::Crypto(
            "remove_header_protection not supported".into(),
            None,
        )))
    }

    /// Decrypt an inbound packet payload in-place (AEAD).
    /// `ciphertext_and_tag` is consumed and plaintext bytes are returned.
    fn decrypt_packet(
        &self,
        _level: EncryptionLevel,
        _pn: u64,
        _aad: &[u8],
        _ciphertext_and_tag: &mut Vec<u8>,
    ) -> Result<Vec<u8>, QuicError> {
        Err(QuicError::Protocol(crate::quic::quic_error::ProtocolError::Crypto(
            "decrypt_packet not supported".into(),
            None,
        )))
    }

    /// Return the original client DCID used for key derivation (server only).
    fn client_dcid(&self) -> Option<Vec<u8>> {
        None
    }
}

#[derive(Default)]
pub struct NoopQuicCryptoProvider;

impl QuicCryptoProvider for NoopQuicCryptoProvider {}

#[cfg(feature = "quic-crypto")]
pub struct FeatureGatedCryptoProvider {
    phase: Mutex<HandshakePhase>,
    highest_crypto_end: Mutex<u64>,
}

#[cfg(feature = "quic-crypto")]
impl Default for FeatureGatedCryptoProvider {
    fn default() -> Self {
        Self {
            phase: Mutex::new(HandshakePhase::Initial),
            highest_crypto_end: Mutex::new(0),
        }
    }
}

#[cfg(feature = "quic-crypto")]
impl QuicCryptoProvider for FeatureGatedCryptoProvider {
    fn on_inbound_header(
        &self,
        _header: &mut QuicHeader,
        ctx: &InboundHeaderProtectionContext,
    ) -> Result<(), QuicError> {
        if !(1..=4).contains(&ctx.packet_number_len) {
            return Err(QuicError::Protocol(ProtocolError::InvalidPacket(
                "invalid packet number length for header protection hook".into(),
            )));
        }
        Ok(())
    }

    fn on_outbound_header(
        &self,
        _header: &mut QuicHeader,
        ctx: &OutboundHeaderProtectionContext,
    ) -> Result<(), QuicError> {
        if !(1..=4).contains(&ctx.packet_number_len) {
            return Err(QuicError::Protocol(ProtocolError::InvalidPacket(
                "invalid outbound packet number length for header protection hook".into(),
            )));
        }
        Ok(())
    }

    fn on_crypto_frame(
        &self,
        frame: &CryptoFrame,
        _level: EncryptionLevel,
        state: &QuicConnectionState,
    ) -> Result<CryptoFrameDisposition, QuicError> {
        let mut phase = self.phase.lock();
        if *phase == HandshakePhase::Closed || state.connection_state == ConnectionState::Closed {
            return Err(QuicError::Protocol(ProtocolError::Crypto(
                "received CRYPTO frame on closed connection".into(),
                None,
            )));
        }

        if *phase == HandshakePhase::Initial {
            *phase = HandshakePhase::Handshaking;
        }

        let mut highest_end = self.highest_crypto_end.lock();
        let frame_end = frame.offset.saturating_add(frame.data.len() as u64);
        if frame_end > *highest_end {
            *highest_end = frame_end;
        }

        if *highest_end > 0 {
            *phase = HandshakePhase::OneRtt;
            Ok(CryptoFrameDisposition::ProgressedHandshake)
        } else {
            Ok(CryptoFrameDisposition::AckOnly)
        }
    }

    fn handshake_phase(&self) -> HandshakePhase {
        *self.phase.lock()
    }

    fn header_protection_ready(&self) -> bool {
        matches!(self.handshake_phase(), HandshakePhase::OneRtt)
    }
}

#[cfg(all(test, feature = "quic-crypto"))]
mod tests {
    use super::*;
    use crate::quic::quic_protocol::{ConnectionId, TransportParameters};

    fn sample_state() -> QuicConnectionState {
        QuicConnectionState {
            local_connection_id: ConnectionId { bytes: vec![1; 8] },
            remote_connection_id: ConnectionId { bytes: vec![2; 8] },
            version: 1,
            transport_params: TransportParameters::default(),
            streams: Vec::new(),
            sent_packets: Vec::new(),
            received_packets: Vec::new(),
            next_packet_number: 0,
            next_stream_id: 0,
            congestion_window: 14720,
            bytes_in_flight: 0,
            rtt: 100,
            connection_state: ConnectionState::Handshaking,
        }
    }

    #[test]
    fn feature_crypto_provider_advances_handshake_phase() {
        let provider = FeatureGatedCryptoProvider::default();
        assert_eq!(provider.handshake_phase(), HandshakePhase::Initial);
        assert!(!provider.header_protection_ready());

        let frame = CryptoFrame {
            offset: 0,
            data: vec![1, 2, 3],
        };
        let state = sample_state();
        let disposition = provider.on_crypto_frame(&frame, &state).unwrap();
        assert_eq!(disposition, CryptoFrameDisposition::ProgressedHandshake);
        assert_eq!(provider.handshake_phase(), HandshakePhase::OneRtt);
        assert!(provider.header_protection_ready());
    }
}

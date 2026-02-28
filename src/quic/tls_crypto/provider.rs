use super::packet_protection::{QuicAeadAlgorithm, QuicCryptoState};
use super::secrets::{derive_initial_secrets, derive_packet_protection_keys};
use crate::quic::quic_crypto::{
    CryptoFrameDisposition, HandshakePhase, InboundHeaderProtectionContext,
    OutboundHeaderProtectionContext, QuicCryptoProvider,
};
use crate::quic::quic_error::{ProtocolError, QuicError};
use crate::quic::quic_protocol::{CryptoFrame, QuicConnectionState, QuicHeader};
use parking_lot::Mutex;
use rustls::quic::Connection;
use std::collections::BTreeMap;
use std::sync::Arc;

pub struct RustlsCryptoProvider {
    conn: Mutex<Connection>,

    pub initial_state: Option<QuicCryptoState>,
    pub handshake_state: Mutex<Option<QuicCryptoState>>,
    pub onertt_state: Mutex<Option<QuicCryptoState>>,

    phase: Mutex<HandshakePhase>,

    crypto_rx_buf_initial: Mutex<BTreeMap<u64, Vec<u8>>>,
    crypto_rx_next_initial: Mutex<u64>,
    crypto_rx_buf_handshake: Mutex<BTreeMap<u64, Vec<u8>>>,
    crypto_rx_next_handshake: Mutex<u64>,
    crypto_failed: Mutex<bool>,

    pending_initial_write: Mutex<Vec<u8>>,
    pending_handshake_write: Mutex<Vec<u8>>,
    handshake_local: Mutex<Option<rustls::quic::DirectionalKeys>>,
    handshake_remote: Mutex<Option<rustls::quic::DirectionalKeys>>,
    onertt_local: Mutex<Option<rustls::quic::DirectionalKeys>>,
    onertt_remote: Mutex<Option<rustls::quic::DirectionalKeys>>,
    client_dcid: Vec<u8>,
}

impl RustlsCryptoProvider {
    pub fn new_server(
        rustls_conn: Connection,
        client_dst_conn_id: &[u8],
    ) -> Result<Self, String> {
        let (_, server_initial) = derive_initial_secrets(client_dst_conn_id);
        let (key, iv, hp) = derive_packet_protection_keys(&server_initial);

        let initial_state = QuicCryptoState::new(
            QuicAeadAlgorithm::Aes128Gcm,
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
            crypto_rx_buf_initial: Mutex::new(BTreeMap::new()),
            crypto_rx_next_initial: Mutex::new(0),
            crypto_rx_buf_handshake: Mutex::new(BTreeMap::new()),
            crypto_rx_next_handshake: Mutex::new(0),
            crypto_failed: Mutex::new(false),
            pending_initial_write: Mutex::new(Vec::new()),
            pending_handshake_write: Mutex::new(Vec::new()),
            handshake_local: Mutex::new(None),
            handshake_remote: Mutex::new(None),
            onertt_local: Mutex::new(None),
            onertt_remote: Mutex::new(None),
            client_dcid: client_dst_conn_id.to_vec(),
        })
    }
}

impl QuicCryptoProvider for RustlsCryptoProvider {
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
        frame: &CryptoFrame,
        level: crate::quic::quic_crypto::EncryptionLevel,
        _state: &QuicConnectionState,
    ) -> Result<CryptoFrameDisposition, QuicError> {
        use crate::quic::quic_crypto::EncryptionLevel;
        if *self.crypto_failed.lock() {
            return Ok(CryptoFrameDisposition::AckOnly);
        }

        {
            let (mut buf, mut next) = match level {
                EncryptionLevel::Initial => (
                    self.crypto_rx_buf_initial.lock(),
                    self.crypto_rx_next_initial.lock(),
                ),
                EncryptionLevel::Handshake => (
                    self.crypto_rx_buf_handshake.lock(),
                    self.crypto_rx_next_handshake.lock(),
                ),
                EncryptionLevel::OneRtt => {
                    return Ok(CryptoFrameDisposition::ProgressedHandshake);
                }
            };

            let frame_end = frame.offset + frame.data.len() as u64;
            if frame_end <= *next {
                return Ok(CryptoFrameDisposition::ProgressedHandshake);
            }

            let trimmed_offset = frame.offset.max(*next);
            let trim = (trimmed_offset - frame.offset) as usize;
            buf.entry(trimmed_offset)
                .or_insert_with(|| frame.data[trim..].to_vec());

            let mut conn = self.conn.lock();
            while let Some((&off, _)) = buf.first_key_value() {
                if off != *next {
                    break;
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

            let mut initial_out = Vec::new();
            let maybe_kc = conn.write_hs(&mut initial_out);
            if !initial_out.is_empty() {
                self.pending_initial_write.lock().extend_from_slice(&initial_out);
            }
            if let Some(kc) = maybe_kc {
                match kc {
                    rustls::quic::KeyChange::Handshake { keys } => {
                        *self.handshake_remote.lock() = Some(keys.remote);
                        *self.handshake_local.lock() = Some(keys.local);
                        let mut hs_out = Vec::new();
                        let maybe_kc2 = conn.write_hs(&mut hs_out);
                        if !hs_out.is_empty() {
                            self.pending_handshake_write.lock().extend_from_slice(&hs_out);
                        }
                        if let Some(rustls::quic::KeyChange::OneRtt { keys, .. }) = maybe_kc2 {
                            *self.onertt_remote.lock() = Some(keys.remote);
                            *self.onertt_local.lock() = Some(keys.local);
                        }
                    }
                    rustls::quic::KeyChange::OneRtt { keys, .. } => {
                        *self.onertt_remote.lock() = Some(keys.remote);
                        *self.onertt_local.lock() = Some(keys.local);
                    }
                }
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

    fn drain_crypto_writes(&self) -> Vec<crate::quic::quic_crypto::CryptoWrite> {
        use crate::quic::quic_crypto::{CryptoWrite, EncryptionLevel};
        let mut out = Vec::new();

        let initial = { let mut g = self.pending_initial_write.lock(); let d = g.clone(); g.clear(); d };
        if !initial.is_empty() {
            out.push(CryptoWrite { level: EncryptionLevel::Initial, data: initial });
        }
        let hs = { let mut g = self.pending_handshake_write.lock(); let d = g.clone(); g.clear(); d };
        if !hs.is_empty() {
            out.push(CryptoWrite { level: EncryptionLevel::Handshake, data: hs });
        }
        out
    }

    fn encrypt_packet(
        &self,
        level: crate::quic::quic_crypto::EncryptionLevel,
        pn: u64,
        header: &[u8],
        payload: &mut Vec<u8>,
    ) -> Result<(), QuicError> {
        use crate::quic::quic_crypto::EncryptionLevel;
        match level {
            EncryptionLevel::Initial => {
                if let Some(state) = &self.initial_state {
                    state.encrypt_payload(pn, header, payload)
                        .map_err(|e| QuicError::Protocol(ProtocolError::Crypto(e, None)))
                } else {
                    Err(QuicError::Protocol(ProtocolError::Crypto("No initial state".into(), None)))
                }
            }
            EncryptionLevel::Handshake => {
                let guard = self.handshake_local.lock();
                if let Some(keys) = guard.as_ref() {
                    let tag = keys.packet.encrypt_in_place(pn, header, payload)
                        .map_err(|e| QuicError::Protocol(ProtocolError::Crypto(format!("{e:?}"), None)))?;
                    payload.extend_from_slice(tag.as_ref());
                    Ok(())
                } else {
                    Err(QuicError::Protocol(ProtocolError::Crypto("No handshake keys".into(), None)))
                }
            }
            EncryptionLevel::OneRtt => {
                let guard = self.onertt_local.lock();
                if let Some(keys) = guard.as_ref() {
                    let tag = keys.packet.encrypt_in_place(pn, header, payload)
                        .map_err(|e| QuicError::Protocol(ProtocolError::Crypto(format!("{e:?}"), None)))?;
                    payload.extend_from_slice(tag.as_ref());
                    Ok(())
                } else {
                    Err(QuicError::Protocol(ProtocolError::Crypto("No 1-RTT keys".into(), None)))
                }
            }
        }
    }

    fn remove_header_protection(
        &self,
        level: crate::quic::quic_crypto::EncryptionLevel,
        sample: &[u8],
        first: &mut u8,
        pn_bytes: &mut [u8],
    ) -> Result<(), QuicError> {
        use crate::quic::quic_crypto::EncryptionLevel;
        match level {
            EncryptionLevel::Initial => {
                if let Some(state) = &self.initial_state {
                    let mask = state.generate_header_protection_mask(sample)
                        .map_err(|e| QuicError::Protocol(ProtocolError::Crypto(e, None)))?;
                    *first ^= mask[0] & 0x0f;
                    for (i, b) in pn_bytes.iter_mut().enumerate() { *b ^= mask[1 + i]; }
                    Ok(())
                } else {
                    Err(QuicError::Protocol(ProtocolError::Crypto("No initial state".into(), None)))
                }
            }
            EncryptionLevel::Handshake => {
                let guard = self.handshake_remote.lock();
                if let Some(keys) = guard.as_ref() {
                    keys.header.decrypt_in_place(sample, first, pn_bytes)
                        .map_err(|e| QuicError::Protocol(ProtocolError::Crypto(format!("{e:?}"), None)))
                } else {
                    Err(QuicError::Protocol(ProtocolError::Crypto("No handshake HP keys".into(), None)))
                }
            }
            EncryptionLevel::OneRtt => {
                let guard = self.onertt_remote.lock();
                if let Some(keys) = guard.as_ref() {
                    keys.header.decrypt_in_place(sample, first, pn_bytes)
                        .map_err(|e| QuicError::Protocol(ProtocolError::Crypto(format!("{e:?}"), None)))
                } else {
                    Err(QuicError::Protocol(ProtocolError::Crypto("No 1-RTT HP keys".into(), None)))
                }
            }
        }
    }

    fn decrypt_packet(
        &self,
        level: crate::quic::quic_crypto::EncryptionLevel,
        pn: u64,
        aad: &[u8],
        ciphertext_and_tag: &mut Vec<u8>,
    ) -> Result<Vec<u8>, QuicError> {
        use crate::quic::quic_crypto::EncryptionLevel;
        match level {
            EncryptionLevel::Initial => {
                if let Some(state) = &self.initial_state {
                    state.decrypt_payload(pn, aad, ciphertext_and_tag)
                        .map(|pt| pt.to_vec())
                        .map_err(|e| QuicError::Protocol(ProtocolError::Crypto(e, None)))
                } else {
                    Err(QuicError::Protocol(ProtocolError::Crypto("No initial state".into(), None)))
                }
            }
            EncryptionLevel::Handshake => {
                let guard = self.handshake_remote.lock();
                if let Some(keys) = guard.as_ref() {
                    keys.packet.decrypt_in_place(pn, aad, ciphertext_and_tag)
                        .map(|pt| pt.to_vec())
                        .map_err(|e| QuicError::Protocol(ProtocolError::Crypto(format!("{e:?}"), None)))
                } else {
                    Err(QuicError::Protocol(ProtocolError::Crypto("No handshake remote keys".into(), None)))
                }
            }
            EncryptionLevel::OneRtt => {
                let guard = self.onertt_remote.lock();
                if let Some(keys) = guard.as_ref() {
                    keys.packet.decrypt_in_place(pn, aad, ciphertext_and_tag)
                        .map(|pt| pt.to_vec())
                        .map_err(|e| QuicError::Protocol(ProtocolError::Crypto(format!("{e:?}"), None)))
                } else {
                    Err(QuicError::Protocol(ProtocolError::Crypto("No 1-RTT remote keys".into(), None)))
                }
            }
        }
    }

    fn client_dcid(&self) -> Option<Vec<u8>> {
        Some(self.client_dcid.clone())
    }

    fn apply_header_protection(
        &self,
        level: crate::quic::quic_crypto::EncryptionLevel,
        sample: &[u8],
        first: &mut u8,
        pn_bytes: &mut [u8],
    ) -> Result<(), QuicError> {
        use crate::quic::quic_crypto::EncryptionLevel;
        match level {
            EncryptionLevel::Initial => {
                if let Some(state) = &self.initial_state {
                    let mask = state.generate_header_protection_mask(sample)
                        .map_err(|e| QuicError::Protocol(ProtocolError::Crypto(e, None)))?;
                    *first ^= mask[0] & 0x0f;
                    for (i, b) in pn_bytes.iter_mut().enumerate() { *b ^= mask[1 + i]; }
                    Ok(())
                } else {
                    Err(QuicError::Protocol(ProtocolError::Crypto("No initial state for HP".into(), None)))
                }
            }
            EncryptionLevel::Handshake => {
                let guard = self.handshake_local.lock();
                if let Some(keys) = guard.as_ref() {
                    keys.header.encrypt_in_place(sample, first, pn_bytes)
                        .map_err(|e| QuicError::Protocol(ProtocolError::Crypto(format!("{e:?}"), None)))
                } else {
                    Err(QuicError::Protocol(ProtocolError::Crypto("No handshake HP keys".into(), None)))
                }
            }
            EncryptionLevel::OneRtt => {
                let guard = self.onertt_local.lock();
                if let Some(keys) = guard.as_ref() {
                    keys.header.encrypt_in_place(sample, first, pn_bytes)
                        .map_err(|e| QuicError::Protocol(ProtocolError::Crypto(format!("{e:?}"), None)))
                } else {
                    Err(QuicError::Protocol(ProtocolError::Crypto("No 1-RTT HP keys".into(), None)))
                }
            }
        }
    }
}

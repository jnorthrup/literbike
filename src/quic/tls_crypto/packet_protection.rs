//! QUIC Packet Protection (AEAD Payload and Header Protection)
use ring::aead;
use ring::aead::quic::HeaderProtectionKey;

pub enum QuicAeadAlgorithm {
    Aes128Gcm,
    Aes256Gcm,
    ChaCha20Poly1305,
}

impl QuicAeadAlgorithm {
    fn get_ring_algo(&self) -> &'static aead::Algorithm {
        match self {
            Self::Aes128Gcm => &aead::AES_128_GCM,
            Self::Aes256Gcm => &aead::AES_256_GCM,
            Self::ChaCha20Poly1305 => &aead::CHACHA20_POLY1305,
        }
    }
    
    fn get_ring_hp_algo(&self) -> &'static aead::quic::Algorithm {
        match self {
            Self::Aes128Gcm => &aead::quic::AES_128,
            Self::Aes256Gcm => &aead::quic::AES_256,
            Self::ChaCha20Poly1305 => &aead::quic::CHACHA20,
        }
    }
}

/// A fully initialized cryptographic state for a single QUIC encryption level 
/// (Initial, Handshake, 1-RTT, etc.)
pub struct QuicCryptoState {
    algorithm: QuicAeadAlgorithm,
    key: aead::LessSafeKey,
    iv: Vec<u8>,
    hp_key: HeaderProtectionKey,
}

impl QuicCryptoState {
    pub fn new(algorithm: QuicAeadAlgorithm, key_bytes: &[u8], iv: Vec<u8>, hp_key_bytes: &[u8]) -> Result<Self, ring::error::Unspecified> {
        let unbound_key = aead::UnboundKey::new(algorithm.get_ring_algo(), key_bytes)?;
        let key = aead::LessSafeKey::new(unbound_key);
        let hp_key = HeaderProtectionKey::new(algorithm.get_ring_hp_algo(), hp_key_bytes)?;
        
        Ok(Self {
            algorithm,
            key,
            iv,
            hp_key,
        })
    }
    
    /// Computes the nonce for a given packet number. 
    /// Nonce = IV XOR Packet_Number (padded to IV length)
    fn compute_nonce(&self, packet_number: u64) -> aead::Nonce {
        let mut nonce_bytes = self.iv.clone();
        let pn_bytes = packet_number.to_be_bytes();
        
        // IV is typically 12 bytes, so we XOR the last 8 bytes with the 64-bit packet number
        let iv_len = nonce_bytes.len();
        for i in 0..8 {
            nonce_bytes[iv_len - 8 + i] ^= pn_bytes[i];
        }
        
        aead::Nonce::try_assume_unique_for_key(&nonce_bytes).unwrap()
    }
    
    /// Encrypts the packet payload in place.
    /// `payload_and_tag` must have enough capacity for the authentication tag (16 bytes).
    /// `aad` (Additional Authenticated Data) should be the unprotected packet header.
    pub fn encrypt_payload(
        &self,
        packet_number: u64,
        aad: &[u8],
        payload_and_tag: &mut Vec<u8>,
    ) -> Result<(), ring::error::Unspecified> {
        let nonce = self.compute_nonce(packet_number);
        let aad = aead::Aad::from(aad);
        
        // Try sealing in place
        // The payload_and_tag is assumed to already contain the plaintext payload.
        // It will append the authentication tag to it.
        self.key.seal_in_place_append_tag(nonce, aad, payload_and_tag)?;
        
        Ok(())
    }
    
    /// Decrypts the packet payload in place.
    /// Returns the decrypted slice inside `ciphertext_and_tag`.
    pub fn decrypt_payload<'a>(
        &self,
        packet_number: u64,
        aad: &[u8],
        ciphertext_and_tag: &'a mut [u8],
    ) -> Result<&'a mut [u8], ring::error::Unspecified> {
        let nonce = self.compute_nonce(packet_number);
        let aad = aead::Aad::from(aad);
        
        self.key.open_in_place(nonce, aad, ciphertext_and_tag)
    }
    
    /// Generates the Header Protection mask.
    /// `sample` must be 16 bytes taken from the ciphertext.
    pub fn generate_header_protection_mask(&self, sample: &[u8]) -> Result<[u8; 5], ring::error::Unspecified> {
        let mask = self.hp_key.new_mask(sample)?;
        Ok(mask)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quic_aead_roundtrip() {
        let key = vec![0u8; 16];
        let iv = vec![0u8; 12];
        let hp = vec![0u8; 16];
        
        let state = QuicCryptoState::new(QuicAeadAlgorithm::Aes128Gcm, &key, iv, &hp).unwrap();
        
        // 10 byte header (AAD), 5 byte payload
        let aad = b"header1234";
        let payload = b"hello";
        
        // payload + 16 bytes for auth tag
        let mut buffer = Vec::new();
        buffer.extend_from_slice(payload);
        
        state.encrypt_payload(42, aad, &mut buffer).unwrap();
        
        // buffer should now be 5 + 16 = 21 bytes long
        assert_eq!(buffer.len(), 21);
        
        // Decrypt
        let decrypted = state.decrypt_payload(42, aad, &mut buffer).unwrap();
        
        assert_eq!(decrypted, b"hello");
    }
}

//! QUIC Packet Protection (AEAD Payload and Header Protection) — OpenSSL
use openssl::symm::{decrypt_aead, encrypt_aead, Cipher};

pub enum QuicAeadAlgorithm {
    Aes128Gcm,
    Aes256Gcm,
    ChaCha20Poly1305,
}

/// A fully initialized cryptographic state for a single QUIC encryption level.
pub struct QuicCryptoState {
    algorithm: QuicAeadAlgorithm,
    key: Vec<u8>,
    iv: Vec<u8>,
    hp_key: Vec<u8>,
}

impl QuicCryptoState {
    pub fn new(
        algorithm: QuicAeadAlgorithm,
        key_bytes: &[u8],
        iv: Vec<u8>,
        hp_key_bytes: &[u8],
    ) -> Result<Self, String> {
        Ok(Self {
            algorithm,
            key: key_bytes.to_vec(),
            iv,
            hp_key: hp_key_bytes.to_vec(),
        })
    }

    fn aead_cipher(&self) -> Cipher {
        match self.algorithm {
            QuicAeadAlgorithm::Aes128Gcm => Cipher::aes_128_gcm(),
            QuicAeadAlgorithm::Aes256Gcm => Cipher::aes_256_gcm(),
            QuicAeadAlgorithm::ChaCha20Poly1305 => {
                // openssl::symm doesn't expose chacha20-poly1305 as Cipher directly;
                // this branch is unused for Initial packets.
                panic!("ChaCha20Poly1305 not yet supported via openssl::symm")
            }
        }
    }

    /// Computes the nonce for a given packet number (IV XOR packet_number).
    fn compute_nonce(&self, packet_number: u64) -> Vec<u8> {
        let mut nonce = self.iv.clone();
        let pn_bytes = packet_number.to_be_bytes();
        let iv_len = nonce.len();
        for i in 0..8 {
            nonce[iv_len - 8 + i] ^= pn_bytes[i];
        }
        nonce
    }

    /// Encrypts the packet payload; appends 16-byte authentication tag.
    pub fn encrypt_payload(
        &self,
        packet_number: u64,
        aad: &[u8],
        payload_and_tag: &mut Vec<u8>,
    ) -> Result<(), String> {
        let nonce = self.compute_nonce(packet_number);
        let cipher = self.aead_cipher();
        let plaintext = payload_and_tag.clone();
        let mut tag = vec![0u8; 16];
        let ciphertext = encrypt_aead(cipher, &self.key, Some(&nonce), aad, &plaintext, &mut tag)
            .map_err(|e| format!("encrypt_aead: {e}"))?;
        *payload_and_tag = ciphertext;
        payload_and_tag.extend_from_slice(&tag);
        Ok(())
    }

    /// Decrypts ciphertext_and_tag in place; returns plaintext slice length.
    pub fn decrypt_payload<'a>(
        &self,
        packet_number: u64,
        aad: &[u8],
        ciphertext_and_tag: &'a mut [u8],
    ) -> Result<&'a mut [u8], String> {
        if ciphertext_and_tag.len() < 16 {
            return Err("ciphertext too short for AES-GCM tag".into());
        }
        let tag_start = ciphertext_and_tag.len() - 16;
        let tag = ciphertext_and_tag[tag_start..].to_vec();
        let ciphertext = &ciphertext_and_tag[..tag_start];
        let nonce = self.compute_nonce(packet_number);
        let cipher = self.aead_cipher();
        let plaintext = decrypt_aead(cipher, &self.key, Some(&nonce), aad, ciphertext, &tag)
            .map_err(|e| format!("decrypt_aead: {e}"))?;
        let pt_len = plaintext.len();
        ciphertext_and_tag[..pt_len].copy_from_slice(&plaintext);
        Ok(&mut ciphertext_and_tag[..pt_len])
    }

    /// Generates the 5-byte header protection mask via AES-ECB on a 16-byte sample.
    pub fn generate_header_protection_mask(&self, sample: &[u8]) -> Result<[u8; 5], String> {
        let cipher = Cipher::aes_128_ecb();
        let encrypted = openssl::symm::encrypt(cipher, &self.hp_key, None, sample)
            .map_err(|e| format!("AES-ECB HP: {e}"))?;
        let mut mask = [0u8; 5];
        mask.copy_from_slice(&encrypted[..5]);
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

        let aad = b"header1234";
        let payload = b"hello";

        let mut buffer = payload.to_vec();
        state.encrypt_payload(42, aad, &mut buffer).unwrap();
        assert_eq!(buffer.len(), 5 + 16);

        let decrypted = state.decrypt_payload(42, aad, &mut buffer).unwrap();
        assert_eq!(decrypted, b"hello");
    }
}

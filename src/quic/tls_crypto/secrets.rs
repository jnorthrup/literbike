//! QUIC TLS 1.3 Secrets Generation (RFC 9001)
use ring::hkdf;
use ring::hmac;

/// The QUIC v1 Initial Salt as defined in RFC 9001 Section 5.2.
/// This salt is used to derive the Initial secrets from the Original Destination Connection ID.
const QUIC_V1_INITIAL_SALT: &[u8] = &[
    0x38, 0x76, 0x2c, 0xf7, 0xf5, 0x59, 0x34, 0xb3, 0x4d, 0x17, 0x9a, 0xe6, 0xa4, 0xc8, 0x0c, 0xad,
    0xcc, 0xbb, 0x7f, 0x0a,
];

/// RFC 9001 Section 5.1: HKDF-Expand-Label
/// `HKDF-Expand-Label(Secret, Label, Context, Length)`
pub fn hkdf_expand_label(
    secret: &hkdf::Prk,
    label_bytes: &[u8],
    context: &[u8],
    len: usize,
) -> Vec<u8> {
    // Construct the HkdfLabel
    // struct HkdfLabel {
    //     uint16 length = Length;
    //     opaque label<7..255> = "tls13 " + Label;
    //     opaque context<0..255> = Context;
    // }
    let mut hkdf_label = Vec::with_capacity(2 + 1 + 6 + label_bytes.len() + 1 + context.len());
    
    // length (uint16)
    hkdf_label.push((len >> 8) as u8);
    hkdf_label.push(len as u8);
    
    // label (length-prefixed opaque string "tls13 " + Label)
    let full_label = [b"tls13 ", label_bytes].concat();
    hkdf_label.push(full_label.len() as u8);
    hkdf_label.extend_from_slice(&full_label);
    
    // context (length-prefixed opaque string)
    hkdf_label.push(context.len() as u8);
    hkdf_label.extend_from_slice(context);

    // Expand
    let info = [hkdf_label.as_slice()];
    let info_arr = [hkdf_label.as_slice()];
    
    let mut out = vec![0u8; len];
    secret.expand(&info_arr, MyOkmLen(len)).unwrap().fill(&mut out).unwrap();
    out
}

/// Helper for dynamic OKM length
struct MyOkmLen(usize);
impl hkdf::KeyType for MyOkmLen {
    fn len(&self) -> usize {
        self.0
    }
}

/// Extracts initial secrets based on the Destination Connection ID.
/// Returns (client_initial_secret, server_initial_secret)
pub fn derive_initial_secrets(client_dst_conn_id: &[u8]) -> (hkdf::Prk, hkdf::Prk) {
    let salt = hkdf::Salt::new(hkdf::HKDF_SHA256, QUIC_V1_INITIAL_SALT);
    let initial_secret = salt.extract(client_dst_conn_id);
    
    // RFC 9001 Section 5.2
    // client_initial_secret = HKDF-Expand-Label(initial_secret, "client in", "", Hash.length)
    let client_secret_bytes = hkdf_expand_label(&initial_secret, b"client in", &[], 32); // SHA-256 hash length is 32
    let client_initial_secret = hkdf::Prk::new_less_safe(hkdf::HKDF_SHA256, &client_secret_bytes);
    
    // server_initial_secret = HKDF-Expand-Label(initial_secret, "server in", "", Hash.length)
    let server_secret_bytes = hkdf_expand_label(&initial_secret, b"server in", &[], 32);
    let server_initial_secret = hkdf::Prk::new_less_safe(hkdf::HKDF_SHA256, &server_secret_bytes);
    
    (client_initial_secret, server_initial_secret)
}

/// Derives the AEAD key and IV from a given secret (like client_initial_secret or server_initial_secret)
/// Returns (key, iv, hp_key)
pub fn derive_packet_protection_keys(secret: &hkdf::Prk) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    // For Initial packets, the AEAD is always AEAD_AES_128_GCM
    // Which means key length is 16 bytes, IV length is 12 bytes, HP key length is 16 bytes
    let key = hkdf_expand_label(secret, b"quic key", &[], 16);
    let iv = hkdf_expand_label(secret, b"quic iv", &[], 12);
    let hp_key = hkdf_expand_label(secret, b"quic hp", &[], 16);
    
    (key, iv, hp_key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rfc9001_initial_secrets() {
        // Test vectors from RFC 9001 Appendix A.1
        let cid = hex::decode("8394c8f03e515708").unwrap();
        
        // Let's derive!
        let (client_secret_prk, server_secret_prk) = derive_initial_secrets(&cid);
        
        // Since ring's Prk doesn't expose raw bytes, we derive the keys to verify them
        let (client_key, client_iv, client_hp) = derive_packet_protection_keys(&client_secret_prk);
        let (server_key, server_iv, server_hp) = derive_packet_protection_keys(&server_secret_prk);
        
        let expected_client_key = hex::decode("1f369613dd76d5467730efcbe3b1a22d").unwrap();
        let expected_client_iv = hex::decode("fa044b2f42a3fd3b46fb255c").unwrap();
        let expected_client_hp = hex::decode("9f50449e04a0e810283a1e9933adedd2").unwrap();
        
        let expected_server_key = hex::decode("cf3a5331653c364c88f0f379b6067e37").unwrap();
        let expected_server_iv = hex::decode("0ac1493ca1905853b0bba03e").unwrap();
        let expected_server_hp = hex::decode("c206b8d9b9f0f37644430b490eeaa314").unwrap();

        assert_eq!(client_key, expected_client_key);
        assert_eq!(client_iv, expected_client_iv);
        assert_eq!(client_hp, expected_client_hp);
        
        assert_eq!(server_key, expected_server_key);
        assert_eq!(server_iv, expected_server_iv);
        assert_eq!(server_hp, expected_server_hp);
    }
}

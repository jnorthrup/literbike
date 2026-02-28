//! QUIC TLS 1.3 Secrets Generation (RFC 9001) — OpenSSL HMAC-SHA256 HKDF
use openssl::hash::MessageDigest;
use openssl::pkey::PKey;
use openssl::sign::Signer;

/// The QUIC v1 Initial Salt as defined in RFC 9001 Section 5.2.
const QUIC_V1_INITIAL_SALT: &[u8] = &[
    0x38, 0x76, 0x2c, 0xf7, 0xf5, 0x59, 0x34, 0xb3, 0x4d, 0x17, 0x9a, 0xe6, 0xa4, 0xc8, 0x0c, 0xad,
    0xcc, 0xbb, 0x7f, 0x0a,
];

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    let pkey = PKey::hmac(key).expect("HMAC key");
    let mut signer = Signer::new(MessageDigest::sha256(), &pkey).expect("Signer");
    signer.update(data).expect("update");
    signer.sign_to_vec().expect("sign")
}

/// HKDF-Extract(salt, IKM) → PRK
fn hkdf_extract(salt: &[u8], ikm: &[u8]) -> Vec<u8> {
    hmac_sha256(salt, ikm)
}

/// HKDF-Expand(PRK, info, L) → OKM
fn hkdf_expand(prk: &[u8], info: &[u8], len: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(len);
    let mut t: Vec<u8> = Vec::new();
    let mut counter: u8 = 1;
    while out.len() < len {
        let mut data = t.clone();
        data.extend_from_slice(info);
        data.push(counter);
        t = hmac_sha256(prk, &data);
        out.extend_from_slice(&t);
        counter += 1;
    }
    out.truncate(len);
    out
}

/// RFC 9001 Section 5.1: HKDF-Expand-Label
pub fn hkdf_expand_label(prk: &[u8], label_bytes: &[u8], context: &[u8], len: usize) -> Vec<u8> {
    let mut hkdf_label = Vec::new();
    // length (uint16)
    hkdf_label.push((len >> 8) as u8);
    hkdf_label.push(len as u8);
    // label = "tls13 " + label_bytes (length-prefixed)
    let full_label = [b"tls13 ", label_bytes].concat();
    hkdf_label.push(full_label.len() as u8);
    hkdf_label.extend_from_slice(&full_label);
    // context (length-prefixed)
    hkdf_label.push(context.len() as u8);
    hkdf_label.extend_from_slice(context);

    hkdf_expand(prk, &hkdf_label, len)
}

/// Extracts initial secrets based on the Destination Connection ID.
/// Returns (client_initial_secret_bytes, server_initial_secret_bytes)
pub fn derive_initial_secrets(client_dst_conn_id: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let initial_secret = hkdf_extract(QUIC_V1_INITIAL_SALT, client_dst_conn_id);
    let client_secret = hkdf_expand_label(&initial_secret, b"client in", &[], 32);
    let server_secret = hkdf_expand_label(&initial_secret, b"server in", &[], 32);
    (client_secret, server_secret)
}

/// Derives the AEAD key, IV, and HP key from a secret (raw bytes).
/// Returns (key, iv, hp_key)
pub fn derive_packet_protection_keys(secret: &[u8]) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
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
        let cid = hex::decode("8394c8f03e515708").unwrap();
        let (client_secret, server_secret) = derive_initial_secrets(&cid);
        let (client_key, client_iv, client_hp) = derive_packet_protection_keys(&client_secret);
        let (server_key, server_iv, server_hp) = derive_packet_protection_keys(&server_secret);

        assert_eq!(client_key, hex::decode("1f369613dd76d5467730efcbe3b1a22d").unwrap());
        assert_eq!(client_iv, hex::decode("fa044b2f42a3fd3b46fb255c").unwrap());
        assert_eq!(client_hp, hex::decode("9f50449e04a0e810283a1e9933adedd2").unwrap());
        assert_eq!(server_key, hex::decode("cf3a5331653c364c88f0f379b6067e37").unwrap());
        assert_eq!(server_iv, hex::decode("0ac1493ca1905853b0bba03e").unwrap());
        assert_eq!(server_hp, hex::decode("c206b8d9b9f0f37644430b490eeaa314").unwrap());
    }
}

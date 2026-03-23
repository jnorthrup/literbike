//! HTX Protocol Implementation
//!
//! Provides constant-time ticket verification for access control using X25519
//! key agreement and HKDF-SHA256 based ticket derivation.
//!
//! This module was extracted from betanet-htx during the unification process.

use hkdf::Hkdf;
use sha2::{Digest, Sha256};
use subtle::{Choice, ConstantTimeEq};
use x25519_dalek::{PublicKey, StaticSecret};

const TICKET_V1_CONTEXT: &[u8] = b"liteyear-ticket-v1";
const TICKET_LEN: usize = 32;

/// Verifies an HTX access ticket in constant time.
pub fn verify_access_ticket(
    server_priv_key: &StaticSecret,
    client_pub_key: &PublicKey,
    ticket_key_id: &[u8; 8],
    received_ticket: &[u8; TICKET_LEN],
    current_hour: u64,
) -> Choice {
    let shared_secret = server_priv_key.diffie_hellman(client_pub_key);

    let expected_ticket_h =
        compute_ticket_for_hour(shared_secret.as_bytes(), ticket_key_id, current_hour);
    let expected_ticket_h_minus_1 =
        compute_ticket_for_hour(shared_secret.as_bytes(), ticket_key_id, current_hour - 1);
    let expected_ticket_h_plus_1 =
        compute_ticket_for_hour(shared_secret.as_bytes(), ticket_key_id, current_hour + 1);

    let match_h = received_ticket.ct_eq(&expected_ticket_h);
    let match_h_minus_1 = received_ticket.ct_eq(&expected_ticket_h_minus_1);
    let match_h_plus_1 = received_ticket.ct_eq(&expected_ticket_h_plus_1);

    match_h | match_h_minus_1 | match_h_plus_1
}

fn compute_ticket_for_hour(
    shared_secret: &[u8],
    ticket_key_id: &[u8; 8],
    hour: u64,
) -> [u8; TICKET_LEN] {
    let mut salt_builder = Sha256::new();
    salt_builder.update(TICKET_V1_CONTEXT);
    salt_builder.update(ticket_key_id);
    salt_builder.update(&hour.to_be_bytes());
    let salt = salt_builder.finalize();

    let hk = Hkdf::<Sha256>::new(Some(&salt), shared_secret);
    let mut okm = [0u8; TICKET_LEN];
    hk.expand(b"", &mut okm)
        .expect("32 bytes is a valid length for HKDF-SHA256");
    okm
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::OsRng;

    #[test]
    fn test_ticket_current_hour() {
        let mut csprng = OsRng;
        let server_priv_key = StaticSecret::new(&mut csprng);
        let server_pub_key = PublicKey::from(&server_priv_key);
        let client_priv_key = StaticSecret::new(&mut csprng);
        let client_pub_key = PublicKey::from(&client_priv_key);

        let ticket_key_id = b"key_0001";
        let current_hour = 1_000_000;

        let shared_secret_client = client_priv_key.diffie_hellman(&server_pub_key);
        let valid_ticket =
            compute_ticket_for_hour(shared_secret_client.as_bytes(), ticket_key_id, current_hour);

        let result = verify_access_ticket(
            &server_priv_key,
            &client_pub_key,
            ticket_key_id,
            &valid_ticket,
            current_hour,
        );
        assert_eq!(result.unwrap_u8(), 1);
    }

    #[test]
    fn test_ticket_invalid() {
        let mut csprng = OsRng;
        let server_priv_key = StaticSecret::new(&mut csprng);
        let server_pub_key = PublicKey::from(&server_priv_key);
        let client_priv_key = StaticSecret::new(&mut csprng);
        let client_pub_key = PublicKey::from(&client_priv_key);

        let ticket_key_id = b"key_0001";
        let current_hour = 1_000_000;

        let shared_secret_client = client_priv_key.diffie_hellman(&server_pub_key);
        let valid_ticket =
            compute_ticket_for_hour(shared_secret_client.as_bytes(), ticket_key_id, current_hour);

        let mut invalid_ticket = valid_ticket;
        invalid_ticket[0] ^= 0xff;

        let result = verify_access_ticket(
            &server_priv_key,
            &client_pub_key,
            ticket_key_id,
            &invalid_ticket,
            current_hour,
        );
        assert_eq!(result.unwrap_u8(), 0);
    }
}

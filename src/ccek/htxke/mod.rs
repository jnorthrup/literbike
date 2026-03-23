//! HTXKE Assembly - Cryptographic access tickets
//!
//! Hierarchical structure (matches Kotlin CCEK):
//! ```text
//! HtxKey
//!   └── HtxElement    (base)
//! ```
//!
//! Provides constant-time ticket verification for access control using X25519
//! key agreement and HKDF-SHA256 based ticket derivation.
//!
//! This is the CCEK assembly for HTX tickets. The name HTXKE means "HTX Key Element"
//! to distinguish from HAProxy's HTX (HTTP message abstraction layer).
//!
//! Code reuse via shared ccek-core.

use ccek_core::{Element, Key};
use hkdf::Hkdf;
use sha2::{Digest, Sha256};
use std::any::{Any, TypeId};
use std::sync::atomic::{AtomicU64, Ordering};
use subtle::{Choice, ConstantTimeEq};
use x25519_dalek::{PublicKey, StaticSecret};

const TICKET_V1_CONTEXT: &[u8] = b"liteyear-ticket-v1";
const TICKET_LEN: usize = 32;

pub struct HtxKey;

impl HtxKey {
    pub const TICKET_VERSION: u8 = 1;
    pub const TICKET_LENGTH: usize = TICKET_LEN;
}

impl Key for HtxKey {
    type Element = HtxElement;
    const FACTORY: fn() -> Self::Element = || HtxElement::new();
}

pub struct HtxElement {
    pub tickets_verified: AtomicU64,
    pub tickets_valid: AtomicU64,
    pub tickets_invalid: AtomicU64,
}

impl HtxElement {
    pub fn new() -> Self {
        Self {
            tickets_verified: AtomicU64::new(0),
            tickets_valid: AtomicU64::new(0),
            tickets_invalid: AtomicU64::new(0),
        }
    }

    pub fn tickets_verified(&self) -> u64 {
        self.tickets_verified.load(Ordering::Relaxed)
    }

    pub fn tickets_valid(&self) -> u64 {
        self.tickets_valid.load(Ordering::Relaxed)
    }

    pub fn tickets_invalid(&self) -> u64 {
        self.tickets_invalid.load(Ordering::Relaxed)
    }

    pub fn verify_access_ticket(
        &self,
        server_priv_key: &StaticSecret,
        client_pub_key: &PublicKey,
        ticket_key_id: &[u8; 8],
        received_ticket: &[u8; TICKET_LEN],
        current_hour: u64,
    ) -> Choice {
        self.tickets_verified.fetch_add(1, Ordering::Relaxed);

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

        let result = match_h | match_h_minus_1 | match_h_plus_1;

        if result.into() {
            self.tickets_valid.fetch_add(1, Ordering::Relaxed);
        } else {
            self.tickets_invalid.fetch_add(1, Ordering::Relaxed);
        }

        result
    }

    pub fn compute_ticket(
        client_priv_key: &StaticSecret,
        server_pub_key: &PublicKey,
        ticket_key_id: &[u8; 8],
        hour: u64,
    ) -> [u8; TICKET_LEN] {
        let shared_secret = client_priv_key.diffie_hellman(server_pub_key);
        compute_ticket_for_hour(shared_secret.as_bytes(), ticket_key_id, hour)
    }
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

impl Element for HtxElement {
    fn key_type(&self) -> TypeId {
        TypeId::of::<HtxKey>()
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ccek_core::Context;
    use rand::rngs::OsRng;

    #[test]
    fn test_htx_key_factory() {
        let elem = HtxKey::FACTORY();
        assert_eq!(elem.tickets_verified(), 0);
    }

    #[test]
    fn test_htx_context() {
        let ctx = Context::new().plus(HtxKey::FACTORY());
        let elem = ctx.get::<HtxKey>().unwrap();
        let e = elem.as_any().downcast_ref::<HtxElement>().unwrap();
        assert_eq!(e.tickets_verified(), 0);
    }

    #[test]
    fn test_ticket_current_hour() {
        let mut csprng = OsRng;
        let server_priv_key = StaticSecret::random_from_rng(&mut csprng);
        let server_pub_key = PublicKey::from(&server_priv_key);
        let client_priv_key = StaticSecret::random_from_rng(&mut csprng);
        let client_pub_key = PublicKey::from(&client_priv_key);

        let ticket_key_id = b"key_0001";
        let current_hour = 1_000_000;

        let valid_ticket = HtxElement::compute_ticket(
            &client_priv_key,
            &server_pub_key,
            ticket_key_id,
            current_hour,
        );

        let ctx = Context::new().plus(HtxKey::FACTORY());
        let elem = ctx.get::<HtxKey>().unwrap();
        let e = elem.as_any().downcast_ref::<HtxElement>().unwrap();

        let result = e.verify_access_ticket(
            &server_priv_key,
            &client_pub_key,
            ticket_key_id,
            &valid_ticket,
            current_hour,
        );
        assert_eq!(result.unwrap_u8(), 1);
        assert_eq!(e.tickets_verified(), 1);
        assert_eq!(e.tickets_valid(), 1);
    }

    #[test]
    fn test_ticket_invalid() {
        let mut csprng = OsRng;
        let server_priv_key = StaticSecret::random_from_rng(&mut csprng);
        let server_pub_key = PublicKey::from(&server_priv_key);
        let client_priv_key = StaticSecret::random_from_rng(&mut csprng);
        let client_pub_key = PublicKey::from(&client_priv_key);

        let ticket_key_id = b"key_0001";
        let current_hour = 1_000_000;

        let valid_ticket = HtxElement::compute_ticket(
            &client_priv_key,
            &server_pub_key,
            ticket_key_id,
            current_hour,
        );

        let mut invalid_ticket = valid_ticket;
        invalid_ticket[0] ^= 0xff;

        let ctx = Context::new().plus(HtxKey::FACTORY());
        let elem = ctx.get::<HtxKey>().unwrap();
        let e = elem.as_any().downcast_ref::<HtxElement>().unwrap();

        let result = e.verify_access_ticket(
            &server_priv_key,
            &client_pub_key,
            ticket_key_id,
            &invalid_ticket,
            current_hour,
        );
        assert_eq!(result.unwrap_u8(), 0);
        assert_eq!(e.tickets_verified(), 1);
        assert_eq!(e.tickets_invalid(), 1);
    }
}

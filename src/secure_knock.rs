//! Implements a secure knock mechanism using HMAC-SHA256.

use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::time::{SystemTime, UNIX_EPOCH};
use std::convert::TryInto;

type HmacSha256 = Hmac<Sha256>;

const TIMESTAMP_SIZE: usize = 8;
const SIGNATURE_SIZE: usize = 32;
pub const KNOCK_PACKET_SIZE: usize = TIMESTAMP_SIZE + SIGNATURE_SIZE;
const VALID_TIME_WINDOW_SECS: u64 = 60; // Knock is valid for 60 seconds

/// Creates a secure knock packet.
/// The packet is a concatenation of the current unix timestamp and its HMAC signature.
pub fn create_knock(psk: &[u8]) -> Vec<u8> {
    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs();

    let timestamp_bytes = current_time.to_be_bytes();

    let mut mac = HmacSha256::new_from_slice(psk)
        .expect("HMAC can take key of any size");
    mac.update(&timestamp_bytes);
    let signature = mac.finalize().into_bytes();

    let mut packet = Vec::with_capacity(KNOCK_PACKET_SIZE);
    packet.extend_from_slice(&timestamp_bytes);
    packet.extend_from_slice(&signature);
    packet
}

/// Verifies a secure knock packet.
/// It checks the timestamp's freshness and the signature's validity.
pub fn verify_knock(psk: &[u8], packet: &[u8]) -> bool {
    if packet.len() != KNOCK_PACKET_SIZE {
        return false;
    }

    let timestamp_bytes: [u8; TIMESTAMP_SIZE] = packet[0..TIMESTAMP_SIZE]
        .try_into()
        .expect("Packet size is checked");
    let signature = &packet[TIMESTAMP_SIZE..];

    let received_timestamp = u64::from_be_bytes(timestamp_bytes);
    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs();

    // Check if the timestamp is within the valid window to prevent replay attacks.
    if received_timestamp > current_time || current_time - received_timestamp > VALID_TIME_WINDOW_SECS {
        return false;
    }

    let mut mac = HmacSha256::new_from_slice(psk)
        .expect("HMAC can take key of any size");
    mac.update(&timestamp_bytes);
    
    mac.verify_slice(signature).is_ok()
}
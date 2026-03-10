//! HTTP/2 Client using curl and h2 for QUIC server testing
//!
//! This module provides HTTP/2 client functionality using curl-sys with HTTP/2 support
//! and the h2 library for low-level HTTP/2 protocol handling.
//!
//! # Example
//!
//! ```rust,no_run
//! use literbike::curl_h2::H2Client;
//!
//! let client = H2Client::new();
//! let response = client.get("https://localhost:8888/").unwrap();
//! println!("Status: {}", response.status);
//! ```

pub mod client;
pub mod error;
pub mod request;
pub mod response;

pub use client::H2Client;
pub use error::H2Error;
pub use request::H2Request;
pub use response::H2Response;

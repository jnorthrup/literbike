//! HTTP-HTX - HTTP message normalization using HTX internal format
//!
//! This crate provides:
//! - HTTP/1, HTTP/2, HTTP/3 parsing
//! - HTX internal message representation (blocks: start-line, headers, data, trailers)
//! - Protocol detection and speculative matching
//! - Event-driven processing pipeline
//!
//! Design follows HAProxy's HTX concept: an internal representation that
//! normalizes all HTTP versions to a common structured format.
//!
//! This is the ROOT of the hierarchy. Only public exports here.
//! Internal modules are private to this crate.

// Private modules - internal implementation
mod protocol;

#[cfg(feature = "matcher")]
mod matcher;

#[cfg(feature = "listener")]
mod listener;

#[cfg(feature = "reactor")]
mod reactor;

#[cfg(feature = "timer")]
mod timer;

#[cfg(feature = "handler")]
mod handler;

// Public exports - controlled API surface
pub use protocol::{
    normalize_to_htx, parse_htx, HtxBlock, HtxBlockType, HtxElement, HtxKey, HtxMessage,
    HtxStartLine,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_htx_key_factory() {
        let elem = HtxKey::FACTORY();
        assert_eq!(elem.version, 1);
    }

    #[test]
    fn test_htx_block_types() {
        assert_eq!(HtxBlockType::ReqSl as u8, 0);
        assert_eq!(HtxBlockType::ResSl as u8, 1);
        assert_eq!(HtxBlockType::Hdr as u8, 2);
        assert_eq!(HtxBlockType::Eoh as u8, 3);
        assert_eq!(HtxBlockType::Data as u8, 4);
        assert_eq!(HtxBlockType::Tlr as u8, 5);
        assert_eq!(HtxBlockType::Eot as u8, 6);
        assert_eq!(HtxBlockType::Unused as u8, 15);
    }
}

// Feature-gated public exports
#[cfg(feature = "matcher")]
pub use matcher::{Confidence, MatchResult, SpeculativeMatcher};

#[cfg(feature = "listener")]
pub use listener::{ListenerElement, ListenerKey};

#[cfg(feature = "reactor")]
pub use reactor::{InterestSet, ReactorElement, ReactorKey, ReadyEvent};

#[cfg(feature = "timer")]
pub use timer::{TimerElement, TimerId, TimerKey};

#[cfg(feature = "handler")]
pub use handler::{HandlerElement, HandlerKey, HandlerStats};

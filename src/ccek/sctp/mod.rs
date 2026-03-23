//! SCTP Assembly - Stream Control Transmission Protocol
//!
//! Hierarchical structure (matches Kotlin CCEK):
//! ```text
//! SctpKey
//!   ├── SctpCoreElement    (base)
//!   ├── AssociationKey     (feature: association)
//!   │     └── AssociationElement
//!   ├── StreamKey         (feature: stream)
//!   │     └── StreamElement
//!   └── ChunkKey          (feature: chunk)
//!         └── ChunkElement
//! ```
//!
//! Code reuse via shared ccek-core.

pub mod association;
pub mod chunk;
pub mod stream;

pub use core::{SctpElement, SctpKey};

pub mod core {
    use ccek_core::{Context, Element, Key};
    use std::any::{Any, TypeId};
    use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

    /// SctpKey - SCTP protocol root
    pub struct SctpKey;

    impl SctpKey {
        pub const MAX_ASSOCIATIONS: u32 = 100;
        pub const MAX_STREAMS: u32 = 65535;
        pub const DEFAULT_PORT: u16 = 9899;
    }

    impl Key for SctpKey {
        type Element = SctpElement;
        const FACTORY: fn() -> Self::Element = || SctpElement::new();
    }

    /// SctpElement - base SCTP state
    pub struct SctpElement {
        pub associations: AtomicU32,
        pub streams: AtomicU32,
        pub packets_sent: AtomicU64,
        pub packets_recv: AtomicU64,
    }

    impl SctpElement {
        pub fn new() -> Self {
            Self {
                associations: AtomicU32::new(0),
                streams: AtomicU32::new(0),
                packets_sent: AtomicU64::new(0),
                packets_recv: AtomicU64::new(0),
            }
        }

        pub fn associations(&self) -> u32 {
            self.associations.load(Ordering::Relaxed)
        }

        pub fn increment_associations(&self) {
            self.associations.fetch_add(1, Ordering::Relaxed);
        }
    }

    impl Element for SctpElement {
        fn key_type(&self) -> TypeId {
            TypeId::of::<SctpKey>()
        }
        fn as_any(&self) -> &dyn Any {
            self
        }
    }

    /// SCTP chunk types
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ChunkType {
        Data,
        Init,
        InitAck,
        Sack,
        Heartbeat,
        HeartbeatAck,
        Abort,
        Shutdown,
        ShutdownAck,
        Error,
        CookieEcho,
        CookieAck,
        ShutdownComplete,
    }

    /// SCTP port
    #[derive(Debug, Clone, Copy)]
    pub struct Port(pub u16);

    impl Port {
        pub fn new(v: u16) -> Self {
            Self(v)
        }
    }

    /// Verification tag
    #[derive(Debug, Clone, Copy)]
    pub struct VerificationTag(pub u32);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ccek_core::{Context, Key};

    #[test]
    fn test_sctp_key_factory() {
        let elem = SctpKey::FACTORY();
        assert_eq!(elem.associations(), 0);
    }

    #[test]
    fn test_sctp_context() {
        let ctx = Context::new().plus(SctpKey::FACTORY());
        let elem = ctx.get::<SctpKey>().unwrap();
        let e = elem.as_any().downcast_ref::<SctpElement>().unwrap();
        assert_eq!(e.associations(), 0);
    }
}

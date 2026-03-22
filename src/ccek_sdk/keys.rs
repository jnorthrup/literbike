//! CCEK Keys - One Key per linkage/channel
//!
//! Each key represents a unique linkage point between components.
//! Keys are compile-time type-level identifiers.

use super::elements::*;
use super::CcekElement;

// Implement CcekKey for all keys via macro
macro_rules! define_ckey {
    ($($name:ident => $elem:ty),*) => {
        $(
            pub struct $name;
            impl CcekKey for $name {
                type Element = $elem;
            }
        )*
    }
}

// HTX verification keys
define_ckey!(
    HtxInputKey => HtxElement,
    HtxOutputKey => HtxElement,
    HtxSessionKey => HtxElement
);

// QUIC protocol keys
define_ckey!(
    QuicPacketsInKey => QuicElement,
    QuicPacketsOutKey => QuicElement,
    QuicSessionKey => QuicElement,
    QuicConnectionKey => QuicElement
);

// NIO reactor keys
define_ckey!(
    NioReadReadyKey => NioElement,
    NioWriteReadyKey => NioElement,
    NioSubmittedKey => NioElement,
    NioCompletedKey => NioElement,
    NioSessionKey => NioElement
);

// HTTP handler keys
define_ckey!(
    HttpRequestKey => HttpElement,
    HttpResponseKey => HttpElement,
    HttpSessionKey => HttpElement
);

// SCTP keys
define_ckey!(
    SctpInKey => SctpElement,
    SctpOutKey => SctpElement,
    SctpSessionKey => SctpElement,
    SctpAssociationKey => SctpElement
);

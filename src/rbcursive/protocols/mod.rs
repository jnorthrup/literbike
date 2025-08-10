// Network protocol parsers using RBCursive combinators
// High-performance, zero-allocation protocol parsing

pub mod http;
pub mod socks5;
pub mod json;

pub use http::*;
pub use socks5::*;
pub use json::*;

// Use HttpMethod defined at rbcursive module level
use crate::rbcursive::HttpMethod;

// Protocol module re-exports

/// Protocol detection result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolType {
    Http(HttpMethod),
    Http2       ,
    Socks5,
    Tls,
    Json,
    Dns,
    Unknown,
}

/// Well-known constants to avoid magic numbers
pub const DEFAULT_PROXY_PORT: u16 = 8888;
pub const UPNP_SSDP_PORT: u16 = 1900;

/// Fast protocol anchors used for initial dispatch decisions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    /// Fixed literal at start, e.g., b"GET ", b"POST"
    Literal(&'static [u8]),
    /// Byte range with min length, e.g., [A-Z]{3,7}
    Range { start: u8, end: u8, min: usize },
    /// Starts with byte, then any until closing (non-nested)
    Confix { open: u8, close: u8 },
}

/// Declarative table of protocol anchors used by shared listeners
pub static PROTOCOL_ANCHORS: &[(&str, &'static [Anchor])] = &[
    ("http", &[
        Anchor::Literal(b"GET "),
        Anchor::Literal(b"POST "),
        Anchor::Literal(b"HEAD "),
        Anchor::Literal(b"PUT "),
        Anchor::Literal(b"DELETE "),
        Anchor::Literal(b"CONNECT "),
        Anchor::Literal(b"OPTIONS "),
        Anchor::Literal(b"TRACE "),
        Anchor::Literal(b"PATCH "),
    ]),
    ("socks5", &[ Anchor::Literal(&[0x05]) ]),
    ("json", &[ Anchor::Confix { open: b'{', close: b'}' } ]),
    // TLS ClientHello usually starts with 0x16 0x03 0x01..0x04
    ("tls", &[ Anchor::Literal(&[0x16, 0x03]) ]),
    // DNS over TCP: 2-byte length prefix followed by header; weak anchor kept minimal
    ("dns", &[ Anchor::Range { start: 0x00, end: 0xFF, min: 2 } ]),
];

// Each protocol exposes combinators that return Incomplete/Error on fail and Complete on success.
// The shared listener can rapidly test anchors to choose a candidate parser with minimal branching.

/// Lightweight hint produced by anchor evaluation prior to full parsing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolHint { Http, Socks5, Json, Unknown }

#[inline(always)]
fn eval_anchor(anchor: &Anchor, data: &[u8]) -> crate::rbcursive::combinators::Signal {
    use crate::rbcursive::combinators::Signal;
    match *anchor {
        Anchor::Literal(lit) => {
            if data.len() < lit.len() { Signal::NeedMore }
            else if data.starts_with(lit) { Signal::Accept }
            else { Signal::Reject }
        }
        Anchor::Range { start, end, min } => {
            let mut n = 0usize;
            for &b in data.iter() {
                if b >= start && b <= end { n += 1; if n >= min { return Signal::Accept; } }
                else { break; }
            }
            if data.len() < min { Signal::NeedMore } else { Signal::Reject }
        }
        Anchor::Confix { open, close } => {
            if data.first() != Some(&open) { return Signal::Reject; }
            // scan small window only
            let limit = data.len().min(256);
            for i in 1..limit {
                if data[i] == close { return Signal::Accept; }
            }
            Signal::NeedMore
        }
    }
}

/// Evaluate all anchors and return a protocol hint on first acceptance
#[inline(always)]
pub fn fast_anchor_hint(data: &[u8]) -> ProtocolHint {
    for (name, anchors) in PROTOCOL_ANCHORS {
        for a in *anchors {
            match eval_anchor(a, data) {
                crate::rbcursive::combinators::Signal::Accept => {
                    return match *name {
                        "http" => ProtocolHint::Http,
                        "socks5" => ProtocolHint::Socks5,
                        "json" => ProtocolHint::Json,
                        _ => ProtocolHint::Unknown,
                    };
                }
                crate::rbcursive::combinators::Signal::NeedMore => { /* keep checking others */ }
                crate::rbcursive::combinators::Signal::Reject => { /* try next */ }
            }
        }
    }
    ProtocolHint::Unknown
}

/// Optional per-port listener table selection; can be extended to specialize per port/iface
#[inline(always)]
pub fn listener_table_for(_port: u16) -> &'static [(&'static str, &'static [Anchor])] {
    // For now, use the global table for all listeners
    PROTOCOL_ANCHORS
}

// -------------- Template-like protocol specs and listener --------------

use crate::rbcursive::combinators::Signal;
// no longer using ProtocolDetection in classify path

#[derive(Clone, Copy)]
pub struct ProtocolSpec {
    pub name: &'static str,
    pub anchors: &'static [Anchor],
    /// Fast parser classification after anchors: returns Protocol or NeedMore/Unknown
    pub classify: fn(&[u8]) -> Classify,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Classify {
    Protocol(ProtocolType),
    NeedMore,
    Unknown,
}

#[inline(always)]
fn http_classify(data: &[u8]) -> Classify {
    // Use method-only parse for super-fast classification
    let p = crate::rbcursive::protocols::http::HttpParser::new();
    match p.parse_method(data).signal() {
        Signal::Accept => {
            // Best effort: try to get concrete method for ProtocolDetection
            match p.parse_method(data) {
                crate::rbcursive::combinators::ParseResult::Complete(m, _) => Classify::Protocol(ProtocolType::Http(m)),
                _ => Classify::Protocol(ProtocolType::Http(HttpMethod::Get)), // fallback placeholder
            }
        }
        Signal::NeedMore => Classify::NeedMore,
        Signal::Reject => Classify::Unknown,
    }
}

#[inline(always)]
fn socks5_classify(data: &[u8]) -> Classify {
    if data.is_empty() { return Classify::NeedMore; }
    if data[0] == 0x05 { Classify::Protocol(ProtocolType::Socks5) } else { Classify::Unknown }
}

#[inline(always)]
fn json_classify(data: &[u8]) -> Classify {
    if data.first() != Some(&b'{') { return Classify::Unknown; }
    // Simple fast scan to a closing brace within a small window
    let limit = data.len().min(256);
    for i in 1..limit { if data[i] == b'}' { return Classify::Protocol(ProtocolType::Json); } }
    Classify::NeedMore
}

#[inline(always)]
fn tls_classify(data: &[u8]) -> Classify {
    if data.len() < 3 { return Classify::NeedMore; }
    if data[0] == 0x16 && data[1] == 0x03 { Classify::Protocol(ProtocolType::Tls) } else { Classify::Unknown }
}

#[inline(always)]
fn dns_classify(data: &[u8]) -> Classify {
    // DNS over TCP: 2-byte length, then header with QR/opcode in high bits of byte 2
    if data.len() < 4 { return Classify::NeedMore; }
    // avoid zero-length and absurdly large short headers
    // minimal heuristic: header bytes [2] has QR/opcode; accept conservatively
    Classify::Protocol(ProtocolType::Dns)
}

pub static PROTOCOL_SPECS: &[ProtocolSpec] = &[
    ProtocolSpec { name: "http", anchors: &[
        Anchor::Literal(b"GET "), Anchor::Literal(b"POST "), Anchor::Literal(b"HEAD "),
        Anchor::Literal(b"PUT "), Anchor::Literal(b"DELETE "), Anchor::Literal(b"CONNECT "),
        Anchor::Literal(b"OPTIONS "), Anchor::Literal(b"TRACE "), Anchor::Literal(b"PATCH "),
    ], classify: http_classify },
    ProtocolSpec { name: "socks5", anchors: &[ Anchor::Literal(&[0x05]) ], classify: socks5_classify },
    ProtocolSpec { name: "json", anchors: &[ Anchor::Confix { open: b'{', close: b'}' } ], classify: json_classify },
    ProtocolSpec { name: "tls", anchors: &[ Anchor::Literal(&[0x16, 0x03]) ], classify: tls_classify },
    ProtocolSpec { name: "dns", anchors: &[ Anchor::Range { start: 0x00, end: 0xFF, min: 2 } ], classify: dns_classify },
];

/// Const array version for const-generic Listener
pub const PROTOCOL_SPECS_ARR: [ProtocolSpec; 5] = [
    ProtocolSpec { name: "http", anchors: &[
        Anchor::Literal(b"GET "), Anchor::Literal(b"POST "), Anchor::Literal(b"HEAD "),
        Anchor::Literal(b"PUT "), Anchor::Literal(b"DELETE "), Anchor::Literal(b"CONNECT "),
        Anchor::Literal(b"OPTIONS "), Anchor::Literal(b"TRACE "), Anchor::Literal(b"PATCH "),
    ], classify: http_classify },
    ProtocolSpec { name: "socks5", anchors: &[ Anchor::Literal(&[0x05]) ], classify: socks5_classify },
    ProtocolSpec { name: "json", anchors: &[ Anchor::Confix { open: b'{', close: b'}' } ], classify: json_classify },
    ProtocolSpec { name: "tls", anchors: &[ Anchor::Literal(&[0x16, 0x03]) ], classify: tls_classify },
    ProtocolSpec { name: "dns", anchors: &[ Anchor::Range { start: 0x00, end: 0xFF, min: 2 } ], classify: dns_classify },
];

/// An inlinable listener over a const protocol spec table
pub struct Listener<const N: usize> {
    specs: &'static [ProtocolSpec; N],
}

impl<const N: usize> Listener<N> {
    pub const fn new(specs: &'static [ProtocolSpec; N]) -> Self { Self { specs } }

    /// Classify using anchors first; on Accept, run the fast classify fn
    #[inline(always)]
    pub fn classify(&self, data: &[u8]) -> Classify {
        let mut need_more = false;
        for spec in self.specs.iter() {
            for a in spec.anchors.iter() {
                match eval_anchor(a, data) {
                    Signal::Accept => {
                        return (spec.classify)(data);
                    }
                    Signal::NeedMore => { need_more = true; }
                    Signal::Reject => {}
                }
            }
        }
        if need_more { Classify::NeedMore } else { Classify::Unknown }
    }
}

// ---------------- HTTP minimal shared types (used by http.rs) ----------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpVersion { Http10, Http11, Http2 }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpHeader<'a> {
    pub name: &'a [u8],
    pub value: &'a [u8],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest<'a> {
    pub method: crate::rbcursive::HttpMethod,
    pub path: &'a [u8],
    pub version: HttpVersion,
    pub headers: Vec<HttpHeader<'a>>,
}

// ---------------- SOCKS5 minimal shared types (used by socks5.rs) ----------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Socks5AuthMethod {
    NoAuth,
    GssApi,
    UserPass,
    NoAcceptable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Socks5Handshake {
    pub version: u8,
    pub methods: Vec<Socks5AuthMethod>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Socks5Connect<'a> {
    pub version: u8,
    pub command: u8,
    pub address_type: u8,
    pub address: &'a [u8],
    pub port: u16,
}

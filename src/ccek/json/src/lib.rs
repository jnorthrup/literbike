//! CCEK JSON - TrikeShed Port
//!
//! Based on ~/work/TrikeShed/commonMain/kotlin/borg/trikeshed/parse/json/*
//! CCEK-only: Element/Key traits, no serde_json, pure Rust stdlib

use ccek_core::{Element, Key};
use std::any::{Any, TypeId};
use std::sync::atomic::{AtomicU64, Ordering};

pub mod bitmap;
pub mod error;
pub mod index;
pub mod parser;
pub mod pool;
pub mod value;

pub use bitmap::JsonBitmap;
pub use error::JsonError;
pub use index::JsonIndex;
pub use parser::{JsonParser, JsElement, JsIndex, JsContext};
pub use pool::{AtomicPool, Pooled};
pub use value::JsonValue;

/// JsonKey - CCEK Key for JSON parsing
pub struct JsonKey;

impl JsonKey {
    pub const FACTORY: fn() -> JsonElement = JsonElement::new;
}

/// JsonElement - CCEK Element tracking JSON parse stats
pub struct JsonElement {
    bytes_parsed: AtomicU64,
    values_reified: AtomicU64,
}

impl JsonElement {
    pub fn new() -> Self {
        Self {
            bytes_parsed: AtomicU64::new(0),
            values_reified: AtomicU64::new(0),
        }
    }

    pub fn record_parse(&self, bytes: usize) {
        self.bytes_parsed.fetch_add(bytes as u64, Ordering::Relaxed);
    }

    pub fn record_reify(&self) {
        self.values_reified.fetch_add(1, Ordering::Relaxed);
    }
}

impl Element for JsonElement {
    fn key_type(&self) -> TypeId {
        TypeId::of::<JsonKey>()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Key for JsonKey {
    type Element = JsonElement;
    const FACTORY: fn() -> Self::Element = JsonElement::new;
}

/// Parse JSON string (CCEK entry point)
pub fn parse(json: &str) -> JsonValue {
    JsonParser::parse(json)
}

/// Create structural bitmap from JSON bytes
pub fn encode_bitmap(input: &[u8]) -> Vec<u8> {
    JsonBitmap::encode(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ccek_json_element() {
        let elem = JsonKey::FACTORY();
        elem.record_parse(100);
        elem.record_reify();
        assert_eq!(elem.bytes_parsed.load(Ordering::Relaxed), 100);
        assert_eq!(elem.values_reified.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_parse_object() {
        let json = r#"{"name": "test", "value": 42}"#;
        let value = parse(json);
        match value {
            JsonValue::Object(map) => {
                assert_eq!(map.len(), 2);
            }
            _ => panic!("Expected object"),
        }
    }

    #[test]
    fn test_bitmap_encode() {
        let json = b"{}[]:,\"test\"";
        let bitmap = encode_bitmap(json);
        assert!(!bitmap.is_empty());
    }
}

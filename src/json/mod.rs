/// JSON Parser Module - Thread-safe replacement for Bun's HashMapPool
///
/// This module provides a fast, memory-safe JSON parser with the following features:
/// - Lock-free pool management using crossbeam queues
/// - SIMD-accelerated parsing via simd-json (optional)
/// - JSON5 support (comments, trailing commas, unquoted keys)
/// - Bun-compatible AST and error types
///
/// # Architecture
///
/// - `parser` - Core JSON parsing logic
/// - `pool` - Thread-safe object pool for reuse
/// - `error` - Error types with position tracking
///
/// # Example
///
/// ```rust
/// use literbike::json::FastJsonParser;
///
/// let json = r#"{"name": "value"}"#;
/// let parser = FastJsonParser::new();
/// let ast = parser.parse(json)?;
/// ```
///
/// # Thread Safety
///
/// All public functions are thread-safe and can be called concurrently
/// from multiple threads without external synchronization.

pub mod error;
pub mod parser;
pub mod pool;

pub use error::JsonError;
pub use parser::FastJsonParser;
pub use pool::AtomicPool;

// Re-export commonly used types
pub use serde_json::Value;

use serde::{Deserialize, Serialize};

/// JSON AST node compatible with Bun's Expr type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Expr {
    /// Object literal: `{"key": "value"}`
    Object {
        properties: Vec<Property>,
        loc: Option<SourceLocation>,
    },
    /// Array literal: `[1, 2, 3]`
    Array {
        elements: Vec<Expr>,
        loc: Option<SourceLocation>,
    },
    /// String literal: `"hello"`
    String {
        value: String,
        loc: Option<SourceLocation>,
    },
    /// Number literal: `42` or `3.14`
    Number {
        value: f64,
        loc: Option<SourceLocation>,
    },
    /// Boolean literal: `true` or `false`
    Boolean {
        value: bool,
        loc: Option<SourceLocation>,
    },
    /// Null literal: `null`
    Null {
        loc: Option<SourceLocation>,
    },
}

/// Object property with key, value, and optional location
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Property {
    pub key: String,
    pub value: Expr,
    pub loc: Option<SourceLocation>,
}

/// Source location for error reporting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceLocation {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
}

impl SourceLocation {
    pub fn new(start: usize, end: usize, line: usize, column: usize) -> Self {
        Self {
            start,
            end,
            line,
            column,
        }
    }
}

/// Parse a JSON string into an AST
///
/// # Example
///
/// ```rust
/// use literbike::json::parse_json;
///
/// let json = r#"{"hello": "world"}"#;
/// let ast = parse_json(json)?;
/// ```
pub fn parse_json(input: &str) -> Result<Expr, JsonError> {
    let parser = FastJsonParser::new();
    parser.parse(input)
}

/// Parse a JSON5 string (with comments, trailing commas, etc.)
///
/// # Example
///
/// ```rust
/// use literbike::json::parse_json5;
///
/// let json5 = r#"// comment
/// {"hello": "world",}"#;
/// let ast = parse_json5(json5)?;
/// ```
#[cfg(feature = "json5")]
pub fn parse_json5(input: &str) -> Result<Expr, JsonError> {
    let parser = FastJsonParser::new();
    parser.parse_json5(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_object() {
        let json = r#"{"name": "value"}"#;
        let ast = parse_json(json).unwrap();
        match ast {
            Expr::Object { properties, .. } => {
                assert_eq!(properties.len(), 1);
                assert_eq!(properties[0].key, "name");
            }
            _ => panic!("Expected object"),
        }
    }

    #[test]
    fn test_parse_array() {
        let json = r#"[1, 2, 3]"#;
        let ast = parse_json(json).unwrap();
        match ast {
            Expr::Array { elements, .. } => {
                assert_eq!(elements.len(), 3);
            }
            _ => panic!("Expected array"),
        }
    }

    #[test]
    fn test_parse_string() {
        let json = r#""hello""#;
        let ast = parse_json(json).unwrap();
        match ast {
            Expr::String { value, .. } => {
                assert_eq!(value, "hello");
            }
            _ => panic!("Expected string"),
        }
    }

    #[test]
    fn test_parse_number() {
        let json = r#"42"#;
        let ast = parse_json(json).unwrap();
        match ast {
            Expr::Number { value, .. } => {
                assert_eq!(value, 42.0);
            }
            _ => panic!("Expected number"),
        }
    }

    #[test]
    fn test_parse_boolean() {
        let json = r#"true"#;
        let ast = parse_json(json).unwrap();
        match ast {
            Expr::Boolean { value, .. } => {
                assert_eq!(value, true);
            }
            _ => panic!("Expected boolean"),
        }
    }

    #[test]
    fn test_parse_null() {
        let json = r#"null"#;
        let ast = parse_json(json).unwrap();
        match ast {
            Expr::Null { .. } => {}
            _ => panic!("Expected null"),
        }
    }
}

//! JsonValue - JSON value types (CCEK-only, no serde)

use std::collections::HashMap;

/// JSON value types - all owned, no lifetimes
#[derive(Debug, Clone, PartialEq)]
pub enum JsonValue {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<JsonValue>),
    Object(HashMap<String, JsonValue>),
}

impl JsonValue {
    pub fn is_null(&self) -> bool {
        matches!(self, JsonValue::Null)
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            JsonValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_number(&self) -> Option<f64> {
        match self {
            JsonValue::Number(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            JsonValue::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn get(&self, key: &str) -> Option<&JsonValue> {
        match self {
            JsonValue::Object(map) => map.get(key),
            _ => None,
        }
    }

    pub fn get_index(&self, idx: usize) -> Option<&JsonValue> {
        match self {
            JsonValue::Array(arr) => arr.get(idx),
            _ => None,
        }
    }
}

impl Default for JsonValue {
    fn default() -> Self {
        JsonValue::Null
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_types() {
        assert!(JsonValue::Null.is_null());
        assert_eq!(JsonValue::Bool(true).as_bool(), Some(true));
        assert_eq!(JsonValue::Number(42.0).as_number(), Some(42.0));
        assert_eq!(JsonValue::String("test".to_string()).as_str(), Some("test"));
    }

    #[test]
    fn test_object_access() {
        let mut map = HashMap::new();
        map.insert("key".to_string(), JsonValue::String("value".to_string()));
        let obj = JsonValue::Object(map);

        assert_eq!(obj.get("key").unwrap().as_str(), Some("value"));
        assert!(obj.get("missing").is_none());
    }

    #[test]
    fn test_array_access() {
        let arr = JsonValue::Array(vec![
            JsonValue::Number(1.0),
            JsonValue::Number(2.0),
        ]);

        assert_eq!(arr.get_index(0).unwrap().as_number(), Some(1.0));
        assert!(arr.get_index(10).is_none());
    }
}

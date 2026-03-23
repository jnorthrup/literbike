//! JsonParser - Port of TrikeShed Json.kt
//!
//! JsElement: (openIdx, closeIdx) × commaIndices
//! JsIndex: bounds × source chars
//! JsContext: JsElement × source chars

use crate::value::JsonValue;

/// JsElement: (openIdx, closeIdx) with comma indices
#[derive(Debug, Clone)]
pub struct JsElement {
    pub open_idx: usize,
    pub close_idx: usize,
    pub comma_idxs: Vec<usize>,
}

impl JsElement {
    pub fn new(open_idx: usize, close_idx: usize, comma_idxs: Vec<usize>) -> Self {
        Self {
            open_idx,
            close_idx,
            comma_idxs,
        }
    }
}

/// JsIndex: bounds with source reference
#[derive(Debug, Clone)]
pub struct JsIndex<'a> {
    pub start: usize,
    pub end: usize,
    pub src: &'a str,
}

impl<'a> JsIndex<'a> {
    pub fn new(start: usize, end: usize, src: &'a str) -> Self {
        Self { start, end, src }
    }

    /// Get segment as string slice
    pub fn as_str(&self) -> &str {
        &self.src[self.start..self.end]
    }
}

/// JsContext: Element indices with source
#[derive(Debug, Clone)]
pub struct JsContext<'a> {
    pub element: JsElement,
    pub src: &'a str,
}

impl<'a> JsContext<'a> {
    pub fn new(element: JsElement, src: &'a str) -> Self {
        Self { element, src }
    }

    /// Get segments (delimiter-exclusive)
    pub fn segments(&self) -> Vec<JsIndex<'a>> {
        let mut boundaries = vec![self.element.open_idx];
        boundaries.extend(&self.element.comma_idxs);
        boundaries.push(self.element.close_idx);

        boundaries
            .windows(2)
            .map(|w| JsIndex::new(w[0] + 1, w[1], self.src))
            .collect()
    }
}

/// JsonParser - Indexes and reifies JSON
pub struct JsonParser;

impl JsonParser {
    /// Parse JSON string to value
    pub fn parse(json: &str) -> JsonValue {
        let trimmed = json.trim();
        Self::reify(trimmed)
    }

    /// Index JSON: find braces and commas
    pub fn index(src: &str) -> JsElement {
        let chars: Vec<char> = src.chars().collect();
        let mut depth = 0;
        let mut open_idx = 0;
        let mut close_idx = 0;
        let mut comma_idxs = Vec::new();
        let mut inside_quote = false;
        let mut escape_next = false;

        for (i, &c) in chars.iter().enumerate() {
            if inside_quote {
                if escape_next {
                    escape_next = false;
                } else if c == '\\' {
                    escape_next = true;
                } else if c == '"' {
                    inside_quote = false;
                }
            } else {
                match c {
                    '{' | '[' => {
                        depth += 1;
                        if depth == 1 {
                            open_idx = i;
                        }
                    }
                    '}' | ']' => {
                        depth -= 1;
                        if depth == 0 {
                            close_idx = i;
                            break;
                        }
                    }
                    ',' if depth == 1 => {
                        comma_idxs.push(i);
                    }
                    '"' => {
                        inside_quote = true;
                    }
                    _ => {}
                }
            }
        }

        JsElement::new(open_idx, close_idx, comma_idxs)
    }

    /// Reify JSON: convert to Rust values
    fn reify(src: &str) -> JsonValue {
        let trimmed = src.trim();
        let first_char = trimmed.chars().next();

        match first_char {
            Some('{') => Self::reify_object(trimmed),
            Some('[') => Self::reify_array(trimmed),
            Some('"') => Self::reify_string(trimmed),
            Some('t') | Some('f') => JsonValue::Bool(trimmed.starts_with('t')),
            Some('n') => JsonValue::Null,
            Some(_) => Self::reify_number(trimmed),
            None => JsonValue::Null,
        }
    }

    fn reify_object(src: &str) -> JsonValue {
        let element = Self::index(src);
        let ctx = JsContext::new(element, src);
        let mut map = std::collections::HashMap::new();

        for segment in ctx.segments() {
            let seg_str = segment.as_str().trim();
            if seg_str.is_empty() {
                continue;
            }

            // Parse key:value pair
            if let Some(colon_idx) = Self::find_colon_in_obj(seg_str) {
                let key_part = seg_str[..colon_idx].trim();
                let value_part = seg_str[colon_idx + 1..].trim();

                // Extract key (remove quotes)
                let key = Self::extract_key(key_part);
                let value = Self::reify(value_part);
                map.insert(key, value);
            }
        }

        JsonValue::Object(map)
    }

    fn reify_array(src: &str) -> JsonValue {
        let element = Self::index(src);
        let ctx = JsContext::new(element, src);
        let mut values = Vec::new();

        for segment in ctx.segments() {
            let seg_str = segment.as_str().trim();
            if !seg_str.is_empty() {
                values.push(Self::reify(seg_str));
            }
        }

        JsonValue::Array(values)
    }

    fn reify_string(src: &str) -> JsonValue {
        // Remove surrounding quotes
        if src.len() >= 2 && src.starts_with('"') && src.ends_with('"') {
            JsonValue::String(src[1..src.len() - 1].to_string())
        } else {
            JsonValue::String(src.to_string())
        }
    }

    fn reify_number(src: &str) -> JsonValue {
        if let Ok(n) = src.parse::<f64>() {
            JsonValue::Number(n)
        } else {
            JsonValue::Null
        }
    }

    fn find_colon_in_obj(seg: &str) -> Option<usize> {
        let mut in_quote = false;
        let mut escape = false;

        for (i, c) in seg.char_indices() {
            if in_quote {
                if escape {
                    escape = false;
                } else if c == '\\' {
                    escape = true;
                } else if c == '"' {
                    in_quote = false;
                }
            } else if c == '"' {
                in_quote = true;
            } else if c == ':' {
                return Some(i);
            }
        }
        None
    }

    fn extract_key(key_part: &str) -> String {
        let trimmed = key_part.trim();
        if trimmed.len() >= 2 && trimmed.starts_with('"') && trimmed.ends_with('"') {
            trimmed[1..trimmed.len() - 1].to_string()
        } else {
            trimmed.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_index_object() {
        let json = r#"{"a": 1, "b": 2}"#;
        let element = JsonParser::index(json);
        assert_eq!(element.open_idx, 0);
        assert_eq!(element.comma_idxs.len(), 1);
    }

    #[test]
    fn test_index_array() {
        let json = r#"[1, 2, 3]"#;
        let element = JsonParser::index(json);
        assert_eq!(element.open_idx, 0);
        assert_eq!(element.comma_idxs.len(), 2);
    }

    #[test]
    fn test_segments() {
        let json = r#"[0, 1, 2]"#;
        let element = JsonParser::index(json);
        let ctx = JsContext::new(element, json);
        let segments = ctx.segments();
        assert_eq!(segments.len(), 3);
        assert_eq!(segments[0].as_str().trim(), "0");
        assert_eq!(segments[1].as_str().trim(), "1");
        assert_eq!(segments[2].as_str().trim(), "2");
    }

    #[test]
    fn test_reify_primitives() {
        assert!(matches!(JsonParser::parse("null"), JsonValue::Null));
        assert!(matches!(JsonParser::parse("true"), JsonValue::Bool(true)));
        assert!(matches!(JsonParser::parse("false"), JsonValue::Bool(false)));
        assert!(matches!(JsonParser::parse("42"), JsonValue::Number(42.0)));
    }

    #[test]
    fn test_reify_string() {
        let val = JsonParser::parse(r#""hello world""#);
        assert!(matches!(val, JsonValue::String(s) if s == "hello world"));
    }

    #[test]
    fn test_reify_object() {
        let json = r#"{"name": "test", "value": 42}"#;
        let val = JsonParser::parse(json);
        match val {
            JsonValue::Object(map) => {
                assert_eq!(map.len(), 2);
                assert!(matches!(map.get("name"), Some(JsonValue::String(s)) if s == "test"));
                assert!(matches!(map.get("value"), Some(JsonValue::Number(42.0))));
            }
            _ => panic!("Expected object"),
        }
    }

    #[test]
    fn test_reify_nested() {
        let json = r#"{"outer": {"inner": [1, 2]}}"#;
        let val = JsonParser::parse(json);
        match val {
            JsonValue::Object(map) => {
                assert!(map.contains_key("outer"));
            }
            _ => panic!("Expected object"),
        }
    }
}

// JSON parser using RBCursive combinators (for PAC files)
// SIMD-accelerated JSON parsing for proxy auto-configuration

use crate::rbcursive::{
    scanner::SimdScanner,
    combinators::*,
    simd::create_optimal_scanner,
};

/// JSON parser for PAC (Proxy Auto-Configuration) files
pub struct JsonParser {
    scanner: Box<dyn SimdScanner>,
}

impl JsonParser {
    pub fn new() -> Self {
        Self {
            scanner: create_optimal_scanner(),
        }
    }
    
    /// Parse JSON value (simplified for PAC file needs)
    pub fn parse_value<'a>(&self, input: &'a [u8]) -> ParseResult< JsonValue<'a>, ParseError> {
        self.skip_whitespace_and_parse(input)
    }
    
    /// Parse JSON object
    pub fn parse_object<'a>(&self, input: &'a [u8]) -> ParseResult< JsonObject<'a>, ParseError> {
        let mut input = input;
        let mut consumed = 0;
        
        // Skip whitespace
        let (remaining, ws_consumed) = self.skip_whitespace(input);
        input = remaining;
        consumed += ws_consumed;
        
        // Expect opening brace
        if input.is_empty() || input[0] != b'{' {
            return ParseResult::Error(ParseError::InvalidInput, consumed);
        }
        input = &input[1..];
        consumed += 1;
        
        let mut pairs = Vec::new();
        
        // Skip whitespace after opening brace
        let (remaining, ws_consumed) = self.skip_whitespace(input);
        input = remaining;
        consumed += ws_consumed;
        
        // Check for empty object
        if !input.is_empty() && input[0] == b'}' {
            return ParseResult::Complete(JsonObject { pairs }, consumed + 1);
        }
        
        loop {
            // Parse key (must be string)
            let (key, key_consumed) = match self.parse_string(input) {
                ParseResult::Complete(s, c) => (s, c),
                ParseResult::Incomplete(c) => return ParseResult::Incomplete(consumed + c),
                ParseResult::Error(e, c) => return ParseResult::Error(e, consumed + c),
            };
            input = &input[key_consumed..];
            consumed += key_consumed;
            
            // Skip whitespace and expect colon
            let (remaining, ws_consumed) = self.skip_whitespace(input);
            input = remaining;
            consumed += ws_consumed;
            
            if input.is_empty() || input[0] != b':' {
                return ParseResult::Error(ParseError::InvalidInput, consumed);
            }
            input = &input[1..];
            consumed += 1;
            
            // Skip whitespace after colon
            let (remaining, ws_consumed) = self.skip_whitespace(input);
            input = remaining;
            consumed += ws_consumed;
            
            // Parse value
            let (value, value_consumed) = match self.parse_value(input) {
                ParseResult::Complete(v, c) => (v, c),
                ParseResult::Incomplete(c) => return ParseResult::Incomplete(consumed + c),
                ParseResult::Error(e, c) => return ParseResult::Error(e, consumed + c),
            };
            input = &input[value_consumed..];
            consumed += value_consumed;
            
            pairs.push(JsonPair { key, value });
            
            // Skip whitespace and check for comma or closing brace
            let (remaining, ws_consumed) = self.skip_whitespace(input);
            input = remaining;
            consumed += ws_consumed;
            
            if input.is_empty() {
                return ParseResult::Incomplete(consumed);
            }
            
            match input[0] {
                b',' => {
                    input = &input[1..];
                    consumed += 1;
                    // Continue to next pair
                }
                b'}' => {
                    return ParseResult::Complete(JsonObject { pairs }, consumed + 1);
                }
                _ => {
                    return ParseResult::Error(ParseError::InvalidInput, consumed);
                }
            }
        }
    }
    
    /// Parse JSON string using SIMD for quote detection
    pub fn parse_string<'a>(&self, input: &'a [u8]) -> ParseResult< &'a [u8], ParseError> {
        if input.is_empty() || input[0] != b'"' {
            return ParseResult::Error(ParseError::InvalidInput, 0);
        }
        
        // Use SIMD to find all quote positions
        let quote_positions = self.scanner.scan_quotes(input);
        
        // First quote should be at position 0
        if quote_positions.is_empty() || quote_positions[0] != 0 {
            return ParseResult::Error(ParseError::InvalidInput, 0);
        }
        
        // Find closing quote, handling escapes
        for &quote_pos in quote_positions.iter().skip(1) {
            if quote_pos >= input.len() {
                continue;
            }
            
            // Check if this quote is escaped
            let mut _escaped = false;
            let mut backslashes = 0;
            
            // Count consecutive backslashes before this quote
            for i in (1..quote_pos).rev() {
                if input[i] == b'\\' {
                    backslashes += 1;
                } else {
                    break;
                }
            }
            
            // If odd number of backslashes, the quote is escaped
            _escaped = backslashes % 2 == 1;
            
            if !_escaped {
                // Found unescaped closing quote
                let string_content = &input[1..quote_pos];
                return ParseResult::Complete(string_content, quote_pos + 1);
            }
        }
        
        // No closing quote found
        ParseResult::Incomplete(input.len())
    }
    
    /// Parse JSON array
    pub fn parse_array<'a>(&self, input: &'a [u8]) -> ParseResult< JsonArray<'a>, ParseError> {
        let mut input = input;
        let mut consumed = 0;
        
        // Skip whitespace
        let (remaining, ws_consumed) = self.skip_whitespace(input);
        input = remaining;
        consumed += ws_consumed;
        
        // Expect opening bracket
        if input.is_empty() || input[0] != b'[' {
            return ParseResult::Error(ParseError::InvalidInput, consumed);
        }
        input = &input[1..];
        consumed += 1;
        
        let mut values = Vec::new();
        
        // Skip whitespace after opening bracket
        let (remaining, ws_consumed) = self.skip_whitespace(input);
        input = remaining;
        consumed += ws_consumed;
        
        // Check for empty array
        if !input.is_empty() && input[0] == b']' {
            return ParseResult::Complete(JsonArray { values }, consumed + 1);
        }
        
        loop {
            // Parse value
            let (value, value_consumed) = match self.parse_value(input) {
                ParseResult::Complete(v, c) => (v, c),
                ParseResult::Incomplete(c) => return ParseResult::Incomplete(consumed + c),
                ParseResult::Error(e, c) => return ParseResult::Error(e, consumed + c),
            };
            input = &input[value_consumed..];
            consumed += value_consumed;
            
            values.push(value);
            
            // Skip whitespace and check for comma or closing bracket
            let (remaining, ws_consumed) = self.skip_whitespace(input);
            input = remaining;
            consumed += ws_consumed;
            
            if input.is_empty() {
                return ParseResult::Incomplete(consumed);
            }
            
            match input[0] {
                b',' => {
                    input = &input[1..];
                    consumed += 1;
                    // Continue to next value
                }
                b']' => {
                    return ParseResult::Complete(JsonArray { values }, consumed + 1);
                }
                _ => {
                    return ParseResult::Error(ParseError::InvalidInput, consumed);
                }
            }
        }
    }
    
    /// Skip whitespace using SIMD acceleration
    fn skip_whitespace<'a>(&self, input: &'a [u8]) -> (&'a [u8], usize) {
        let mut consumed = 0;
        
        while consumed < input.len() {
            match input[consumed] {
                b' ' | b'\t' | b'\r' | b'\n' => consumed += 1,
                _ => break,
            }
        }
        
        (&input[consumed..], consumed)
    }
    
    /// Skip whitespace and parse value
    fn skip_whitespace_and_parse<'a>(&self, input: &'a [u8]) -> ParseResult< JsonValue<'a>, ParseError> {
        let (input, consumed) = self.skip_whitespace(input);
        
        if input.is_empty() {
            return ParseResult::Incomplete(consumed);
        }
        
        let result = match input[0] {
            b'"' => {
                // String
                match self.parse_string(input) {
                    ParseResult::Complete(s, c) => ParseResult::Complete(JsonValue::String(s), c),
                    ParseResult::Incomplete(c) => ParseResult::Incomplete(c),
                    ParseResult::Error(e, c) => ParseResult::Error(e, c),
                }
            }
            b'{' => {
                // Object
                match self.parse_object(input) {
                    ParseResult::Complete(obj, c) => ParseResult::Complete(JsonValue::Object(obj), c),
                    ParseResult::Incomplete(c) => ParseResult::Incomplete(c),
                    ParseResult::Error(e, c) => ParseResult::Error(e, c),
                }
            }
            b'[' => {
                // Array
                match self.parse_array(input) {
                    ParseResult::Complete(arr, c) => ParseResult::Complete(JsonValue::Array(arr), c),
                    ParseResult::Incomplete(c) => ParseResult::Incomplete(c),
                    ParseResult::Error(e, c) => ParseResult::Error(e, c),
                }
            }
            b't' | b'f' => {
                // Boolean
                if input.starts_with(b"true") {
                    ParseResult::Complete(JsonValue::Boolean(true), 4)
                } else if input.starts_with(b"false") {
                    ParseResult::Complete(JsonValue::Boolean(false), 5)
                } else {
                    ParseResult::Error(ParseError::InvalidInput, 0)
                }
            }
            b'n' => {
                // Null
                if input.starts_with(b"null") {
                    ParseResult::Complete(JsonValue::Null, 4)
                } else {
                    ParseResult::Error(ParseError::InvalidInput, 0)
                }
            }
            b'-' | b'0'..=b'9' => {
                // Number (simplified)
                self.parse_number(input)
            }
            _ => ParseResult::Error(ParseError::InvalidInput, 0),
        };
        
        // Add consumed whitespace to result
        match result {
            ParseResult::Complete(val, c) => ParseResult::Complete(val, consumed + c),
            ParseResult::Incomplete(c) => ParseResult::Incomplete(consumed + c),
            ParseResult::Error(e, c) => ParseResult::Error(e, consumed + c),
        }
    }
    
    /// Parse JSON number (simplified)
    fn parse_number<'a>(&self, input: &'a [u8]) -> ParseResult< JsonValue<'a>, ParseError> {
        let mut consumed = 0;
        
        // Optional minus
        if consumed < input.len() && input[consumed] == b'-' {
            consumed += 1;
        }
        
        if consumed >= input.len() {
            return ParseResult::Incomplete(consumed);
        }
        
        // Digits
        if !input[consumed].is_ascii_digit() {
            return ParseResult::Error(ParseError::InvalidInput, consumed);
        }
        
        while consumed < input.len() && input[consumed].is_ascii_digit() {
            consumed += 1;
        }
        
        // Optional decimal part
        if consumed < input.len() && input[consumed] == b'.' {
            consumed += 1;
            
            if consumed >= input.len() || !input[consumed].is_ascii_digit() {
                return ParseResult::Error(ParseError::InvalidInput, consumed);
            }
            
            while consumed < input.len() && input[consumed].is_ascii_digit() {
                consumed += 1;
            }
        }
        
        let number_slice = &input[..consumed];
        ParseResult::Complete(JsonValue::Number(number_slice), consumed)
    }
}

/// JSON value types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsonValue<'a> {
    String(&'a [u8]),
    Number(&'a [u8]),
    Boolean(bool),
    Null,
    Object(JsonObject<'a>),
    Array(JsonArray<'a>),
}

/// JSON object
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonObject<'a> {
    pub pairs: Vec<JsonPair<'a>>,
}

/// JSON key-value pair
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonPair<'a> {
    pub key: &'a [u8],
    pub value: JsonValue<'a>,
}

/// JSON array
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonArray<'a> {
    pub values: Vec<JsonValue<'a>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_string_parsing() {
        let parser = JsonParser::new(ScanStrategy::Scalar);
        
        let input = b"\"hello world\"";
        match parser.parse_string(input) {
            ParseResult::Complete(result, consumed) => {
                assert_eq!(result, b"hello world");
                assert_eq!(consumed, 13);
            }
            other => panic!("Expected successful string parse, got {:?}", other),
        }
    }

    #[test]
    fn test_json_string_with_escapes() {
        let parser = JsonParser::new(ScanStrategy::Scalar);
        
        let input = b"\"hello \\\"world\\\"\"";
        match parser.parse_string(input) {
            ParseResult::Complete(result, consumed) => {
                assert_eq!(result, b"hello \\\"world\\\"");
                assert_eq!(consumed, input.len());
            }
            other => panic!("Expected successful escaped string parse, got {:?}", other),
        }
    }

    #[test]
    fn test_json_object_parsing() {
        let parser = JsonParser::new(ScanStrategy::Scalar);
        
        let input = b"{\"key\": \"value\", \"number\": 42}";
        match parser.parse_object(input) {
            ParseResult::Complete(object, consumed) => {
                assert_eq!(object.pairs.len(), 2);
                assert_eq!(object.pairs[0].key, b"key");
                assert!(matches!(object.pairs[0].value, JsonValue::String(b"value")));
                assert_eq!(consumed, input.len());
            }
            other => panic!("Expected successful object parse, got {:?}", other),
        }
    }

    #[test]
    fn test_json_array_parsing() {
        let parser = JsonParser::new(ScanStrategy::Scalar);
        
        let input = b"[\"item1\", \"item2\", 123]";
        match parser.parse_array(input) {
            ParseResult::Complete(array, consumed) => {
                assert_eq!(array.values.len(), 3);
                assert!(matches!(array.values[0], JsonValue::String(b"item1")));
                assert!(matches!(array.values[1], JsonValue::String(b"item2")));
                assert!(matches!(array.values[2], JsonValue::Number(b"123")));
                assert_eq!(consumed, input.len());
            }
            other => panic!("Expected successful array parse, got {:?}", other),
        }
    }

    #[test]
    fn test_json_value_parsing() {
        let parser = JsonParser::new(ScanStrategy::Scalar);
        
        // Test boolean
        let input = b"true";
        match parser.parse_value(input) {
            ParseResult::Complete(JsonValue::Boolean(true), 4) => {}
            other => panic!("Expected boolean true, got {:?}", other),
        }
        
        // Test null
        let input = b"null";
        match parser.parse_value(input) {
            ParseResult::Complete(JsonValue::Null, 4) => {}
            other => panic!("Expected null, got {:?}", other),
        }
        
        // Test number
        let input = b"123.45";
        match parser.parse_value(input) {
            ParseResult::Complete(JsonValue::Number(b"123.45"), 6) => {}
            other => panic!("Expected number, got {:?}", other),
        }
    }

    #[test]
    fn test_json_whitespace_handling() {
        let parser = JsonParser::new(ScanStrategy::Scalar);
        
        let input = b"  \t\n  \"test\"  ";
        match parser.parse_value(input) {
            ParseResult::Complete(JsonValue::String(b"test"), consumed) => {
                // Should consume whitespace before the string
                assert!(consumed > 6); // More than just the string length
            }
            other => panic!("Expected successful whitespace handling, got {:?}", other),
        }
    }

    #[test]
    fn test_json_pac_file_example() {
        let parser = JsonParser::new(ScanStrategy::Scalar);
        
        // Simplified PAC-style JSON
        let input = b"{\"proxy\": \"PROXY proxy.example.com:8080\", \"direct\": \"DIRECT\"}";
        
        match parser.parse_object(input) {
            ParseResult::Complete(object, _) => {
                assert_eq!(object.pairs.len(), 2);
                
                // Check proxy entry
                let proxy_pair = &object.pairs[0];
                assert_eq!(proxy_pair.key, b"proxy");
                assert!(matches!(proxy_pair.value, JsonValue::String(b"PROXY proxy.example.com:8080")));
                
                // Check direct entry
                let direct_pair = &object.pairs[1];
                assert_eq!(direct_pair.key, b"direct");
                assert!(matches!(direct_pair.value, JsonValue::String(b"DIRECT")));
            }
            other => panic!("Expected successful PAC JSON parse, got {:?}", other),
        }
    }
}
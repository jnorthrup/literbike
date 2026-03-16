/// JSON Error types with position tracking
///
/// Provides detailed error information for JSON parsing failures,
/// compatible with Bun's error reporting format.
use std::fmt;

/// JSON parsing error
#[derive(Debug, Clone)]
pub enum JsonError {
    /// Syntax error at specific position
    Syntax {
        message: String,
        line: usize,
        column: usize,
        offset: usize,
    },

    /// Invalid number format
    InvalidNumber {
        value: String,
        line: usize,
        column: usize,
        offset: usize,
    },

    /// Unterminated string
    UnterminatedString {
        line: usize,
        column: usize,
        offset: usize,
    },

    /// Unexpected character
    UnexpectedCharacter {
        character: char,
        expected: String,
        line: usize,
        column: usize,
        offset: usize,
    },

    /// Trailing data after JSON value
    TrailingData { offset: usize },

    /// Duplicate key in object (JSON5 spec violation)
    DuplicateKey {
        key: String,
        line: usize,
        column: usize,
        offset: usize,
    },

    /// Invalid escape sequence
    InvalidEscape {
        sequence: String,
        line: usize,
        column: usize,
        offset: usize,
    },

    /// Stack overflow (deeply nested structures)
    StackOverflow,

    /// Out of memory
    OutOfMemory,

    /// IO error
    Io { message: String },
}

impl JsonError {
    /// Get the line number where the error occurred
    pub fn line(&self) -> Option<usize> {
        match self {
            JsonError::Syntax { line, .. }
            | JsonError::InvalidNumber { line, .. }
            | JsonError::UnterminatedString { line, .. }
            | JsonError::UnexpectedCharacter { line, .. }
            | JsonError::DuplicateKey { line, .. }
            | JsonError::InvalidEscape { line, .. } => Some(*line),
            JsonError::TrailingData { .. } => None,
            JsonError::StackOverflow | JsonError::OutOfMemory | JsonError::Io { .. } => None,
        }
    }

    /// Get the column number where the error occurred
    pub fn column(&self) -> Option<usize> {
        match self {
            JsonError::Syntax { column, .. }
            | JsonError::InvalidNumber { column, .. }
            | JsonError::UnterminatedString { column, .. }
            | JsonError::UnexpectedCharacter { column, .. }
            | JsonError::DuplicateKey { column, .. }
            | JsonError::InvalidEscape { column, .. } => Some(*column),
            JsonError::TrailingData { .. } => None,
            JsonError::StackOverflow | JsonError::OutOfMemory | JsonError::Io { .. } => None,
        }
    }

    /// Get the byte offset where the error occurred
    pub fn offset(&self) -> Option<usize> {
        match self {
            JsonError::Syntax { offset, .. }
            | JsonError::InvalidNumber { offset, .. }
            | JsonError::UnterminatedString { offset, .. }
            | JsonError::UnexpectedCharacter { offset, .. }
            | JsonError::DuplicateKey { offset, .. }
            | JsonError::InvalidEscape { offset, .. }
            | JsonError::TrailingData { offset } => Some(*offset),
            JsonError::StackOverflow | JsonError::OutOfMemory | JsonError::Io { .. } => None,
        }
    }

    /// Create a syntax error at a specific position
    pub fn syntax(message: impl Into<String>, line: usize, column: usize, offset: usize) -> Self {
        JsonError::Syntax {
            message: message.into(),
            line,
            column,
            offset,
        }
    }

    /// Create an invalid number error
    pub fn invalid_number(
        value: impl Into<String>,
        line: usize,
        column: usize,
        offset: usize,
    ) -> Self {
        JsonError::InvalidNumber {
            value: value.into(),
            line,
            column,
            offset,
        }
    }

    /// Create an unterminated string error
    pub fn unterminated_string(line: usize, column: usize, offset: usize) -> Self {
        JsonError::UnterminatedString {
            line,
            column,
            offset,
        }
    }

    /// Create an unexpected character error
    pub fn unexpected_character(
        character: char,
        expected: impl Into<String>,
        line: usize,
        column: usize,
        offset: usize,
    ) -> Self {
        JsonError::UnexpectedCharacter {
            character,
            expected: expected.into(),
            line,
            column,
            offset,
        }
    }

    /// Create a trailing data error
    pub fn trailing_data(offset: usize) -> Self {
        JsonError::TrailingData { offset }
    }

    /// Create a duplicate key error
    pub fn duplicate_key(
        key: impl Into<String>,
        line: usize,
        column: usize,
        offset: usize,
    ) -> Self {
        JsonError::DuplicateKey {
            key: key.into(),
            line,
            column,
            offset,
        }
    }

    /// Create an invalid escape error
    pub fn invalid_escape(
        sequence: impl Into<String>,
        line: usize,
        column: usize,
        offset: usize,
    ) -> Self {
        JsonError::InvalidEscape {
            sequence: sequence.into(),
            line,
            column,
            offset,
        }
    }
}

impl fmt::Display for JsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JsonError::Syntax {
                message,
                line,
                column,
                ..
            } => {
                write!(
                    f,
                    "Syntax error at line {}, column {}: {}",
                    line, column, message
                )
            }
            JsonError::InvalidNumber {
                value,
                line,
                column,
                ..
            } => {
                write!(
                    f,
                    "Invalid number '{}' at line {}, column {}",
                    value, line, column
                )
            }
            JsonError::UnterminatedString { line, column, .. } => {
                write!(f, "Unterminated string at line {}, column {}", line, column)
            }
            JsonError::UnexpectedCharacter {
                character,
                expected,
                line,
                column,
                ..
            } => {
                write!(
                    f,
                    "Unexpected character '{}' at line {}, column {} (expected {})",
                    character, line, column, expected
                )
            }
            JsonError::TrailingData { offset } => {
                write!(f, "Trailing data after JSON value at offset {}", offset)
            }
            JsonError::DuplicateKey {
                key, line, column, ..
            } => {
                write!(
                    f,
                    "Duplicate key '{}' at line {}, column {}",
                    key, line, column
                )
            }
            JsonError::InvalidEscape {
                sequence,
                line,
                column,
                ..
            } => {
                write!(
                    f,
                    "Invalid escape sequence '{}' at line {}, column {}",
                    sequence, line, column
                )
            }
            JsonError::StackOverflow => {
                write!(f, "Stack overflow: JSON structure too deeply nested")
            }
            JsonError::OutOfMemory => {
                write!(f, "Out of memory")
            }
            JsonError::Io { message } => {
                write!(f, "IO error: {}", message)
            }
        }
    }
}

impl std::error::Error for JsonError {}

/// Convert from serde_json errors
impl From<serde_json::Error> for JsonError {
    fn from(err: serde_json::Error) -> Self {
        if err.is_syntax() {
            JsonError::syntax(err.to_string(), 0, 0, 0)
        } else if err.is_io() {
            JsonError::Io {
                message: err.to_string(),
            }
        } else {
            JsonError::syntax(err.to_string(), 0, 0, 0)
        }
    }
}

/// Convert from std::io::Error
impl From<std::io::Error> for JsonError {
    fn from(err: std::io::Error) -> Self {
        JsonError::Io {
            message: err.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = JsonError::syntax("Unexpected token", 1, 5, 10);
        assert_eq!(
            err.to_string(),
            "Syntax error at line 1, column 5: Unexpected token"
        );
    }

    #[test]
    fn test_invalid_number() {
        let err = JsonError::invalid_number("123abc", 1, 4, 3);
        match err {
            JsonError::InvalidNumber {
                value,
                line,
                column,
                offset,
            } => {
                assert_eq!(value, "123abc");
                assert_eq!(line, 1);
                assert_eq!(column, 4);
                assert_eq!(offset, 3);
            }
            _ => panic!("Expected InvalidNumber error"),
        }
    }

    #[test]
    fn test_unexpected_character() {
        let err = JsonError::unexpected_character('}', '"', 1, 5, 4);
        match err {
            JsonError::UnexpectedCharacter {
                character,
                expected,
                ..
            } => {
                assert_eq!(character, '}');
                assert_eq!(expected, "\"");
            }
            _ => panic!("Expected UnexpectedCharacter error"),
        }
    }

    #[test]
    fn test_duplicate_key() {
        let err = JsonError::duplicate_key("name", 1, 7, 6);
        match err {
            JsonError::DuplicateKey {
                key, line, column, ..
            } => {
                assert_eq!(key, "name");
                assert_eq!(line, 1);
                assert_eq!(column, 7);
            }
            _ => panic!("Expected DuplicateKey error"),
        }
    }
}

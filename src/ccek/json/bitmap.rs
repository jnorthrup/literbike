//! JsonBitmap - Port of TrikeShed JsonBitmap.kt
//!
//! 4-bit per byte encoding: 2 bits lexer state + 2 bits JS state

/// JsonBitmap - Structural bitmap for JSON parsing
pub struct JsonBitmap;

/// JS State Events (2 bits)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum JsStateEvent {
    Unchanged = 0,
    ScopeOpen = 1,  // { [
    ScopeClose = 2, // } ]
    ValueDelim = 3, // ,
}

/// Lexer Events (2 bits)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum LexerEvents {
    Unchanged = 0,
    QuoteIncrement = 1,            // "
    EscapeIncrement = 2,           // \
    UtfInitiatorOrContinuation = 3,// >= 0x80
}

impl JsonBitmap {
    /// Encode input bytes to 4-bit bitmap
    /// Output: 2 bytes per input byte (4 bits each, packed)
    pub fn encode(input: &[u8]) -> Vec<u8> {
        let output_size = (input.len() + 1) / 2;
        let mut output = vec![0u8; output_size];

        for (i, &byte) in input.iter().enumerate() {
            let js_state = Self::test_js_state(byte);
            let lexer_event = Self::test_lexer_event(byte);
            let packed = (js_state as u8) | ((lexer_event as u8) << 2);

            let out_idx = i / 2;
            if i % 2 == 0 {
                // High nibble
                output[out_idx] = packed << 4;
            } else {
                // Low nibble
                output[out_idx] |= packed;
            }
        }

        output
    }

    /// Decode bitmap with quote/escape state machine
    /// Returns filtered structural events (only outside quotes)
    pub fn decode(bitmap: &[u8], input_size: usize) -> Vec<u8> {
        let mut result = vec![0u8; (input_size + 3) / 4]; // 2 bits per byte
        let mut quote_counter: u32 = 0;
        let mut escape_counter: u32 = 0;

        for i in 0..input_size {
            let (js_state, lexer_event) = Self::unpack(bitmap, i);

            // Update state machine
            if quote_counter % 2 != 0 {
                // Inside quotes
                if escape_counter % 2 != 0 {
                    escape_counter = 0;
                } else if lexer_event == LexerEvents::EscapeIncrement {
                    escape_counter = 1;
                } else if lexer_event == LexerEvents::QuoteIncrement {
                    quote_counter += 1;
                }
            } else {
                // Outside quotes
                if lexer_event == LexerEvents::QuoteIncrement {
                    quote_counter += 1;
                }
            }

            // Mask JS state if inside quotes
            let masked_state = if quote_counter % 2 != 0 {
                JsStateEvent::Unchanged as u8
            } else {
                js_state as u8
            };

            // Pack 2-bit result (4 results per byte)
            let out_idx = i / 4;
            let shift = (3 - (i % 4)) * 2;
            result[out_idx] |= masked_state << shift;
        }

        result
    }

    /// Test byte for JS state event
    fn test_js_state(byte: u8) -> JsStateEvent {
        match byte {
            b'{' | b'[' => JsStateEvent::ScopeOpen,
            b'}' | b']' => JsStateEvent::ScopeClose,
            b',' => JsStateEvent::ValueDelim,
            _ => JsStateEvent::Unchanged,
        }
    }

    /// Test byte for lexer event
    fn test_lexer_event(byte: u8) -> LexerEvents {
        match byte {
            b'"' => LexerEvents::QuoteIncrement,
            b'\\' => LexerEvents::EscapeIncrement,
            b if b >= 0x80 => LexerEvents::UtfInitiatorOrContinuation,
            _ => LexerEvents::Unchanged,
        }
    }

    /// Unpack 4-bit value from bitmap at position
    fn unpack(bitmap: &[u8], pos: usize) -> (JsStateEvent, LexerEvents) {
        let byte_idx = pos / 2;
        let is_high = pos % 2 == 0;

        let nibble = if is_high {
            bitmap[byte_idx] >> 4
        } else {
            bitmap[byte_idx] & 0x0F
        };

        let js_state = match nibble & 0x03 {
            1 => JsStateEvent::ScopeOpen,
            2 => JsStateEvent::ScopeClose,
            3 => JsStateEvent::ValueDelim,
            _ => JsStateEvent::Unchanged,
        };

        let lexer_event = match (nibble >> 2) & 0x03 {
            1 => LexerEvents::QuoteIncrement,
            2 => LexerEvents::EscapeIncrement,
            3 => LexerEvents::UtfInitiatorOrContinuation,
            _ => LexerEvents::Unchanged,
        };

        (js_state, lexer_event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode() {
        let input = b"{}[]:,\"";
        let bitmap = JsonBitmap::encode(input);
        assert_eq!(bitmap.len(), 4); // 7 bytes -> 4 bytes
    }

    #[test]
    fn test_js_state_detection() {
        assert_eq!(JsonBitmap::test_js_state(b'{'), JsStateEvent::ScopeOpen);
        assert_eq!(JsonBitmap::test_js_state(b'}'), JsStateEvent::ScopeClose);
        assert_eq!(JsonBitmap::test_js_state(b','), JsStateEvent::ValueDelim);
        assert_eq!(JsonBitmap::test_js_state(b'a'), JsStateEvent::Unchanged);
    }

    #[test]
    fn test_lexer_detection() {
        assert_eq!(JsonBitmap::test_lexer_event(b'"'), LexerEvents::QuoteIncrement);
        assert_eq!(JsonBitmap::test_lexer_event(b'\\'), LexerEvents::EscapeIncrement);
        assert_eq!(JsonBitmap::test_lexer_event(0x80), LexerEvents::UtfInitiatorOrContinuation);
    }
}

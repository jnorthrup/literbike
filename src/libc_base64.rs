use std::io;

const BASE64_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const PAD: u8 = b'=';

const DECODE_TABLE: [u8; 256] = [
    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 62,  255, 255, 255, 63,
    52,  53,  54,  55,  56,  57,  58,  59,  60,  61,  255, 255, 255, 64,  255, 255,
    255, 0,   1,   2,   3,   4,   5,   6,   7,   8,   9,   10,  11,  12,  13,  14,
    15,  16,  17,  18,  19,  20,  21,  22,  23,  24,  25,  255, 255, 255, 255, 255,
    255, 26,  27,  28,  29,  30,  31,  32,  33,  34,  35,  36,  37,  38,  39,  40,
    41,  42,  43,  44,  45,  46,  47,  48,  49,  50,  51,  255, 255, 255, 255, 255,
    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
];

pub fn encode(input: &[u8]) -> String {
    let mut output = Vec::with_capacity((input.len() + 2) / 3 * 4);
    
    let chunks = input.chunks_exact(3);
    let remainder = chunks.remainder();
    
    // Process complete 3-byte chunks
    for chunk in chunks {
        let b1 = chunk[0];
        let b2 = chunk[1];
        let b3 = chunk[2];
        
        output.push(BASE64_CHARS[(b1 >> 2) as usize]);
        output.push(BASE64_CHARS[(((b1 & 0x03) << 4) | (b2 >> 4)) as usize]);
        output.push(BASE64_CHARS[(((b2 & 0x0f) << 2) | (b3 >> 6)) as usize]);
        output.push(BASE64_CHARS[(b3 & 0x3f) as usize]);
    }
    
    // Handle remaining bytes
    match remainder.len() {
        1 => {
            let b1 = remainder[0];
            output.push(BASE64_CHARS[(b1 >> 2) as usize]);
            output.push(BASE64_CHARS[((b1 & 0x03) << 4) as usize]);
            output.push(PAD);
            output.push(PAD);
        }
        2 => {
            let b1 = remainder[0];
            let b2 = remainder[1];
            output.push(BASE64_CHARS[(b1 >> 2) as usize]);
            output.push(BASE64_CHARS[(((b1 & 0x03) << 4) | (b2 >> 4)) as usize]);
            output.push(BASE64_CHARS[((b2 & 0x0f) << 2) as usize]);
            output.push(PAD);
        }
        _ => {}
    }
    
    // SAFETY: We only pushed valid ASCII bytes
    unsafe { String::from_utf8_unchecked(output) }
}

pub fn decode(input: &str) -> io::Result<Vec<u8>> {
    decode_bytes(input.as_bytes())
}

pub fn decode_bytes(input: &[u8]) -> io::Result<Vec<u8>> {
    // Skip whitespace and calculate actual length
    let input: Vec<u8> = input.iter()
        .filter(|&&b| !b.is_ascii_whitespace())
        .copied()
        .collect();
    
    if input.is_empty() {
        return Ok(Vec::new());
    }
    
    // Check for invalid length
    if input.len() % 4 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid base64 length",
        ));
    }
    
    // Calculate output size
    let mut output_len = input.len() / 4 * 3;
    if input.len() >= 2 && input[input.len() - 1] == PAD {
        output_len -= 1;
        if input[input.len() - 2] == PAD {
            output_len -= 1;
        }
    }
    
    let mut output = Vec::with_capacity(output_len);
    let mut i = 0;
    
    while i < input.len() {
        let b1 = DECODE_TABLE[input[i] as usize];
        let b2 = DECODE_TABLE[input[i + 1] as usize];
        let b3 = if i + 2 < input.len() && input[i + 2] != PAD {
            DECODE_TABLE[input[i + 2] as usize]
        } else {
            0
        };
        let b4 = if i + 3 < input.len() && input[i + 3] != PAD {
            DECODE_TABLE[input[i + 3] as usize]
        } else {
            0
        };
        
        // Check for invalid characters
        if b1 == 255 || b2 == 255 || (input[i + 2] != PAD && b3 == 255) || (input[i + 3] != PAD && b4 == 255) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid base64 character",
            ));
        }
        
        output.push((b1 << 2) | (b2 >> 4));
        
        if i + 2 < input.len() && input[i + 2] != PAD {
            output.push((b2 << 4) | (b3 >> 2));
            
            if i + 3 < input.len() && input[i + 3] != PAD {
                output.push((b3 << 6) | b4);
            }
        }
        
        i += 4;
    }
    
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_encode() {
        assert_eq!(encode(b""), "");
        assert_eq!(encode(b"f"), "Zg==");
        assert_eq!(encode(b"fo"), "Zm8=");
        assert_eq!(encode(b"foo"), "Zm9v");
        assert_eq!(encode(b"foob"), "Zm9vYg==");
        assert_eq!(encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(encode(b"foobar"), "Zm9vYmFy");
    }
    
    #[test]
    fn test_decode() {
        assert_eq!(decode("").unwrap(), b"");
        assert_eq!(decode("Zg==").unwrap(), b"f");
        assert_eq!(decode("Zm8=").unwrap(), b"fo");
        assert_eq!(decode("Zm9v").unwrap(), b"foo");
        assert_eq!(decode("Zm9vYg==").unwrap(), b"foob");
        assert_eq!(decode("Zm9vYmE=").unwrap(), b"fooba");
        assert_eq!(decode("Zm9vYmFy").unwrap(), b"foobar");
    }
    
    #[test]
    fn test_decode_with_whitespace() {
        assert_eq!(decode("Zm9v\nYmFy").unwrap(), b"foobar");
        assert_eq!(decode("  Zm9v  YmFy  ").unwrap(), b"foobar");
    }
}
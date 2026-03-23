// densifier insight: MLIR mock for TDD — lightweight compile + fallback interpreter

/// A tiny mock that pretends to "compile" a matching function from MLIR source.
/// For TDD we treat MLIR source containing the token `compile_ok` as compilable.

#[derive(Debug)]
pub enum MlirError {
    CompileError(String),
}

pub struct CompiledMatcher {
    // simple boxed matcher for tests
    matcher: Box<dyn Fn(&[u8]) -> bool + Send + Sync + 'static>,
}

impl CompiledMatcher {
    pub fn run(&self, data: &[u8]) -> bool {
        (self.matcher)(data)
    }
}

/// Try to compile MLIR text into a `CompiledMatcher`. In this mock, if the text
/// contains `compile_ok` we return a fast matcher that checks for a u64 pattern
/// embedded as hex literal `0x1122334455667788` inside the source. Otherwise fail.
pub fn compile_mlir(src: &str) -> Result<CompiledMatcher, MlirError> {
    if src.contains("compile_ok") {
        // extract a pattern from a hex literal if present, else use default
        let default: u64 = 0x1122_3344_5566_7788;
        let pat = if let Some(idx) = src.find("0x") {
            let snippet = &src[idx..];
            // parse until non-hex
            let mut end = 2;
            for c in snippet[2..].chars() {
                if c.is_ascii_hexdigit() {
                    end += 1;
                } else {
                    break;
                }
            }
            let lit = &snippet[..end];
            u64::from_str_radix(&lit[2..], 16).unwrap_or(default)
        } else {
            default
        };

        let m = move |data: &[u8]| {
            if data.len() < 8 {
                return false;
            }
            let bytes = &data[..8];
            let w = u64::from_be_bytes([
                bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
            ]);
            w == pat
        };

        Ok(CompiledMatcher { matcher: Box::new(m) })
    } else {
        Err(MlirError::CompileError("mock compile failure".into()))
    }
}

/// Interpreter fallback: a very small interpreter that looks for a hex literal in the
/// source and scans `data` for that u64 pattern; returns whether it was found.
pub fn interpret_mlir(src: &str, data: &[u8]) -> bool {
    let default: u64 = 0x1122_3344_5566_7788;
    let pat = if let Some(idx) = src.find("0x") {
        let snippet = &src[idx..];
        let mut end = 2;
        for c in snippet[2..].chars() {
            if c.is_ascii_hexdigit() {
                end += 1;
            } else {
                break;
            }
        }
        let lit = &snippet[..end];
        u64::from_str_radix(&lit[2..], 16).unwrap_or(default)
    } else {
        default
    };

    if data.len() < 8 {
        return false;
    }
    data.windows(8).any(|w| u64::from_be_bytes([w[0], w[1], w[2], w[3], w[4], w[5], w[6], w[7]]) == pat)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compile_success_and_run() {
        let src = "// mlir: compile_ok pattern 0x1122334455667788";
        let compiled = compile_mlir(src).expect("should compile");
        let data = vec![0x11,0x22,0x33,0x44,0x55,0x66,0x77,0x88];
        assert!(compiled.run(&data));
    }

    #[test]
    fn compile_failure_fallback_interpreter() {
        let src = "// mlir: some_uncompilable_code pattern 0x1122334455667788";
        assert!(compile_mlir(src).is_err());
        let data = vec![0x11,0x22,0x33,0x44,0x55,0x66,0x77,0x88];
        assert!(interpret_mlir(src, &data));
    }

    #[test]
    fn interpreter_no_pattern() {
        let src = "// mlir: nopattern";
        let data = vec![0u8; 4];
        assert!(!interpret_mlir(src, &data));
    }
}

//! Gemini API Format

use crate::types::*;

pub struct GeminiClient;
impl GeminiClient {
    pub fn new() -> Self { Self }
    pub fn base_url() -> &'static str { Provider::Gemini.base_url() }
}

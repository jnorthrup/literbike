//! DeepSeek API Format

use crate::api_translation::types::*;

pub struct DeepSeekClient;
impl DeepSeekClient {
    pub fn new() -> Self { Self }
    pub fn base_url() -> &'static str { Provider::DeepSeek.base_url() }
}

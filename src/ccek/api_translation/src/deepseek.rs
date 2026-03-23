//! DeepSeek API Format

use crate::types::*;

pub struct DeepSeekClient;
impl DeepSeekClient {
    pub fn new() -> Self { Self }
    pub fn base_url() -> &'static str { Provider::DeepSeek.base_url() }
}

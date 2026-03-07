//! Anthropic API Format

use crate::api_translation::types::*;

pub struct AnthropicClient;
impl AnthropicClient {
    pub fn new() -> Self { Self }
    pub fn base_url() -> &'static str { Provider::Anthropic.base_url() }
}

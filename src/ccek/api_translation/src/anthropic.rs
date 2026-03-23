//! Anthropic API Format

use crate::types::*;

pub struct AnthropicClient;
impl AnthropicClient {
    pub fn new() -> Self { Self }
    pub fn base_url() -> &'static str { Provider::Anthropic.base_url() }
}

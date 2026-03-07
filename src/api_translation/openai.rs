//! OpenAI API Format

use crate::api_translation::types::*;

pub struct OpenAIClient;
impl OpenAIClient {
    pub fn new() -> Self { Self }
    pub fn base_url() -> &'static str { Provider::OpenAI.base_url() }
}

//! OpenAI API Format

use crate::types::*;

pub struct OpenAIClient;
impl OpenAIClient {
    pub fn new() -> Self { Self }
    pub fn base_url() -> &'static str { Provider::OpenAI.base_url() }
}

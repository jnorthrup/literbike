//! Type definitions for the Universal Model Facade

use serde::{Deserialize, Serialize};

/// Model information for /v1/models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub owned_by: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<WebModelCard>,
}

/// Specialized agent metadata ("Web Model Card")
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebModelCard {
    pub tags: Vec<String>,
    pub context_window: u64,
    pub pricing: Option<Pricing>,
    pub reasoning_depth: u8, // 1-10
    pub code_native: bool,
}

/// Model pricing info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pricing {
    pub prompt: f64,
    pub completion: f64,
    pub unit: String, // e.g. "1M tokens"
}

/// Model identifier parsed from `/provider/model` syntax
#[derive(Debug, Clone)]
pub struct ModelId {
    pub provider: String,
    pub model: String,
}

impl ModelId {
    pub fn parse(s: &str) -> Option<Self> {
        let mut parts = s.splitn(2, '/');
        let provider = parts.next()?.trim().to_string();
        let model = parts.next()?.trim().to_string();

        if provider.is_empty() || model.is_empty() {
            return None;
        }

        Some(Self { provider, model })
    }
}

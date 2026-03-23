pub mod models;
pub mod transform;

pub use models::*;
pub use transform::*;

use serde::{Deserialize, Serialize};

/// Model mapping configuration for provider-specific translations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMapping {
    pub haiku_model: Option<String>,
    pub sonnet_model: Option<String>,
    pub opus_model: Option<String>,
    pub default_model: Option<String>,
}

impl ModelMapping {
    pub fn map_model(&self, original_model: &str) -> String {
        let lower = original_model.to_lowercase();

        if lower.contains("haiku") {
            if let Some(m) = &self.haiku_model {
                return m.clone();
            }
        }
        if lower.contains("sonnet") {
            if let Some(m) = &self.sonnet_model {
                return m.clone();
            }
        }
        if lower.contains("opus") {
            if let Some(m) = &self.opus_model {
                return m.clone();
            }
        }

        self.default_model
            .clone()
            .unwrap_or_else(|| original_model.to_string())
    }
}

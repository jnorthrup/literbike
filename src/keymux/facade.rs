//! Universal Model Facade implementation

use crate::keymux::cards::ModelCardStore;
use crate::keymux::types::{ModelId, ModelInfo};
use crate::keymux::dsel::{DSELBuilder, RuleEngine};
use chrono::Utc;
use std::sync::Arc;

pub struct ModelFacade {
    model_cards: Arc<ModelCardStore>,
    rule_engine: RuleEngine,
}

impl ModelFacade {
    pub fn new(model_cards: Arc<ModelCardStore>) -> Self {
        // Initialize DSEL engine with default quotas for active providers
        // This gives us priority-based routing and quota-aware selection
        let mut rule_engine = DSELBuilder::new()
            .with_quota("global_pool", 10_000_000)
            .with_provider("openai", 2_000_000, 1, 10.0, false)
            .with_provider("anthropic", 2_000_000, 1, 15.0, false)
            .with_provider("google", 5_000_000, 2, 5.0, false)
            .with_free_provider("kilo_code", 1_000_000, 3, 100_000, 3_000_000, 0)
            .build_with_rule_engine()
            .unwrap_or_else(|_| RuleEngine::new());
            
        Self { 
            model_cards,
            rule_engine
        }
    }

    /// Aggregate models from all providers, check quotas via DSEL, and enrich with metadata
    pub fn handle_models(&mut self, active_providers: Vec<String>) -> Vec<ModelInfo> {
        let mut models = Vec::new();
        let all_known_models = self.model_cards.get_all_models();

        // if no providers are supplied, or tagging is disabled via env var,
        // just return every cached model without prefix filtering.  this
        // lets the emulator work with dynamically loaded models without
        // needing explicit "tags" for each provider.
        let ignore_tags = std::env::var("MODELMUX_IGNORE_TAGS")
            .map(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
            .unwrap_or(false);

        // 1. Quota-Aware Selection & Priority Routing
        // We filter out providers that are out of quota
        let mut eligible_providers = Vec::new();
        for provider in &active_providers {
            if self.rule_engine.has_sufficient_quota(provider, 100) {
                eligible_providers.push(provider.clone());
            }
        }

        // If we have no eligible providers or tags are ignored, include all models
        if eligible_providers.is_empty() || ignore_tags {
            for m_id in all_known_models {
                let provider = m_id
                    .split('/')
                    .next()
                    .unwrap_or("unknown")
                    .to_string();
                let metadata = self.model_cards.get_card(&m_id);
                models.push(ModelInfo {
                    id: m_id.clone(),
                    object: "model".to_string(),
                    created: Utc::now().timestamp(),
                    owned_by: provider.clone(),
                    metadata,
                });
            }
            return models;
        }

        // 2. DSEL-Driven Discovery
        // Instead of hardcoded match blocks, we dynamically discover models 
        // that start with the provider prefix from our ModelCardStore registry.
        for provider in eligible_providers {
            let prefix = format!("{}/", provider);
            
            let provider_models: Vec<String> = all_known_models
                .iter()
                .filter(|m| m.starts_with(&prefix))
                .cloned()
                .collect();

            for m_id in provider_models {
                let metadata = self.model_cards.get_card(&m_id);
                models.push(ModelInfo {
                    id: m_id.clone(),
                    object: "model".to_string(),
                    created: Utc::now().timestamp(),
                    owned_by: provider.clone(),
                    metadata,
                });
            }
        }

        // Sort by some logic if needed (e.g. priority from RuleEngine)
        // For now, they are grouped by provider.
        models
    }
}

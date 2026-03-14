//! Web Model Cards - Specialized metadata cache for agent reasoning

use crate::keymux::types::{Pricing, WebModelCard};
use crate::modelmux::cache::CachedModel;
use parking_lot::RwLock;
use std::collections::HashMap;

pub struct ModelCardStore {
    cards: RwLock<HashMap<String, WebModelCard>>,
}

impl ModelCardStore {
    pub fn new() -> Self {
        Self {
            cards: RwLock::new(HashMap::new()),
        }
    }

    pub fn upsert(&self, id: String, card: WebModelCard) {
        self.cards.write().insert(id, card);
    }

    /// Bulk-populate cards from CachedModel data, inferring tags from model names.
    pub fn populate_from_cached(&self, models: &[CachedModel]) {
        let mut cards = self.cards.write();
        for m in models {
            let name_lower = m.name.to_ascii_lowercase();
            let id_lower = m.id.to_ascii_lowercase();
            let haystack = format!("{} {}", name_lower, id_lower);

            let mut tags: Vec<String> = Vec::new();

            // Capability tags inferred from name
            if haystack.contains("vision") || haystack.contains("4o") || haystack.contains("gemini")
            {
                tags.push("vision".to_string());
            }
            if haystack.contains("r1")
                || haystack.contains("thinking")
                || haystack.contains("reasoning")
                || haystack.contains("o1")
                || haystack.contains("o3")
                || haystack.contains("o4")
            {
                tags.push("reasoning".to_string());
            }
            if haystack.contains("coder")
                || haystack.contains("code")
                || haystack.contains("codestral")
                || haystack.contains("deepseek-coder")
                || haystack.contains("qwen2.5-coder")
            {
                tags.push("coding".to_string());
            }
            if haystack.contains("flash")
                || haystack.contains("mini")
                || haystack.contains("nano")
                || haystack.contains("haiku")
                || haystack.contains("8b")
                || haystack.contains("instant")
            {
                tags.push("fast".to_string());
            }
            if m.supports_tools {
                tags.push("tools".to_string());
            }
            if haystack.contains("claude")
                || haystack.contains("gpt")
                || haystack.contains("gemini")
                || haystack.contains("llama")
                || haystack.contains("sonnet")
                || haystack.contains("opus")
            {
                tags.push("general".to_string());
            }

            // Infer reasoning_depth: 1-10 scale
            let reasoning_depth = if tags.contains(&"reasoning".to_string()) {
                8
            } else if haystack.contains("opus")
                || haystack.contains("405b")
                || haystack.contains("sonnet")
            {
                9
            } else if tags.contains(&"fast".to_string()) {
                4
            } else {
                6
            };

            // code_native: true for known coding-centric models or large general models
            let code_native = tags.contains(&"coding".to_string())
                || haystack.contains("claude")
                || haystack.contains("gpt-4")
                || haystack.contains("gemini");

            let pricing = if m.input_cost_per_million > 0.0 || m.output_cost_per_million > 0.0 {
                Some(Pricing {
                    prompt: m.input_cost_per_million,
                    completion: m.output_cost_per_million,
                    unit: "1M tokens".to_string(),
                })
            } else {
                None
            };

            cards.insert(
                m.id.clone(),
                WebModelCard {
                    tags,
                    context_window: m.context_window,
                    pricing,
                    reasoning_depth,
                    code_native,
                },
            );
        }
    }

    pub fn get_all_models(&self) -> Vec<String> {
        self.cards.read().keys().cloned().collect()
    }

    pub fn get_card(&self, model_id: &str) -> Option<WebModelCard> {
        self.cards.read().get(model_id).cloned()
    }
}

impl Default for ModelCardStore {
    fn default() -> Self {
        Self::new()
    }
}

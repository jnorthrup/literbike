// Model Mux + Keymux Unified Integration
// literbike hosts both model mux and keymux with unified precedence

use crate::env_facade_parity::{
    normalize_env_pairs, ModelmuxMvpLifecycle, NormalizedEnvProfile,
    QuotaInventorySlot, StaticMockQuotaInventoryAdapter, QuotaInventoryAdapter,
    evaluate_modelmux_mvp_quota_inventory, run_modelmux_quota_drainer_dry_run,
};
use crate::model_serving_taxonomy::ProviderFamily;
use std::collections::BTreeMap;

/// Unified Model Mux + Keymux State
/// literbike hosts both systems with unified decision making
#[derive(Debug, Clone)]
pub struct UnifiedMuxState {
    /// Normalized environment profile from env projection
    pub env_profile: NormalizedEnvProfile,
    
    /// Modelmux lifecycle state
    pub lifecycle: Option<ModelmuxMvpLifecycle>,
    
    /// Quota inventory slots
    pub quota_slots: Vec<QuotaInventorySlot>,
    
    /// Selected provider from keymux
    pub selected_provider: Option<String>,
    
    /// Precedence mode for decision making
    pub precedence: PrecedenceMode,
}

impl UnifiedMuxState {
    /// Create new unified mux state from env pairs
    pub fn from_env_pairs(env_pairs: Vec<(String, String)>) -> Self {
        let env_profile = normalize_env_pairs(env_pairs);
        
        Self {
            env_profile,
            lifecycle: None,
            quota_slots: Vec::new(),
            selected_provider: None,
            precedence: PrecedenceMode::default(),
        }
    }
    
    /// Set precedence mode
    pub fn with_precedence(mut self, mode: PrecedenceMode) -> Self {
        self.precedence = mode;
        self
    }
    
    /// Make routing decision based on precedence mode
    pub fn make_decision(&self) -> Option<RoutingDecision> {
        match &self.precedence {
            PrecedenceMode::EnvFirst => self.decision_from_env(),
            PrecedenceMode::KeymuxFirst => self.decision_from_keymux(),
            PrecedenceMode::Balanced { env_weight, keymux_weight } => {
                self.decision_balanced(*env_weight, *keymux_weight)
            }
            PrecedenceMode::Custom(rules) => self.decision_custom(rules),
        }
    }
    
    /// Decision from env projection
    fn decision_from_env(&self) -> Option<RoutingDecision> {
        for entry in &self.env_profile.entries {
            if entry.key.ends_with("_API_KEY") {
                let provider = Self::extract_provider_from_key(&entry.key);
                if !provider.is_empty() {
                    return Some(RoutingDecision::from_env(&provider, 0.9));
                }
            }
        }
        None
    }
    
    /// Decision from keymux
    fn decision_from_keymux(&self) -> Option<RoutingDecision> {
        if let Some(ref provider) = self.selected_provider {
            return Some(RoutingDecision::from_keymux(provider, 0.95));
        }
        None
    }
    
    /// Balanced decision with weighted voting
    fn decision_balanced(&self, env_weight: f32, keymux_weight: f32) -> Option<RoutingDecision> {
        let env_decision = self.decision_from_env();
        let keymux_decision = self.decision_from_keymux();
        
        match (env_decision, keymux_decision) {
            (Some(env), Some(keymux)) => {
                if env.provider_id == keymux.provider_id {
                    return Some(RoutingDecision::balanced(&env.provider_id, 0.95));
                }
                let env_score = env.confidence * env_weight;
                let keymux_score = keymux.confidence * keymux_weight;
                if env_score > keymux_score {
                    Some(RoutingDecision::balanced(&env.provider_id, env_score))
                } else {
                    Some(RoutingDecision::balanced(&keymux.provider_id, keymux_score))
                }
            }
            (Some(decision), None) => Some(decision),
            (None, Some(decision)) => Some(decision),
            (None, None) => None,
        }
    }
    
    /// Custom rule-based decision
    fn decision_custom(&self, rules: &[PrecedenceRule]) -> Option<RoutingDecision> {
        for entry in &self.env_profile.entries {
            if entry.key.ends_with("_API_KEY") {
                let provider = Self::extract_provider_from_key(&entry.key);
                if let Some(rule) = rules.iter().find(|r| r.provider_id == provider) {
                    match &rule.precedence {
                        ProviderPrecedence::EnvOnly => {
                            return Some(RoutingDecision::from_env(&provider, 0.9));
                        }
                        ProviderPrecedence::KeymuxOnly => {
                            return Some(RoutingDecision::from_keymux(&provider, 0.9));
                        }
                        _ => return self.make_decision(),
                    }
                }
            }
        }
        self.make_decision()
    }
    
    fn extract_provider_from_key(key: &str) -> String {
        key.trim_end_matches("_API_KEY")
            .trim_end_matches("_AUTH_TOKEN")
            .to_lowercase()
    }
}

/// Precedence mode for unified mux decision making
#[derive(Debug, Clone, PartialEq)]
pub enum PrecedenceMode {
    EnvFirst,
    KeymuxFirst,
    Balanced { env_weight: f32, keymux_weight: f32 },
    Custom(Vec<PrecedenceRule>),
}

impl Default for PrecedenceMode {
    fn default() -> Self {
        PrecedenceMode::EnvFirst
    }
}

/// Precedence rule for custom configuration
#[derive(Debug, Clone, PartialEq)]
pub struct PrecedenceRule {
    pub provider_id: String,
    pub precedence: ProviderPrecedence,
}

impl PrecedenceRule {
    pub fn new(provider_id: &str, precedence: ProviderPrecedence) -> Self {
        Self {
            provider_id: provider_id.to_string(),
            precedence,
        }
    }
}

/// Provider-specific precedence
#[derive(Debug, Clone, PartialEq)]
pub enum ProviderPrecedence {
    EnvOnly,
    KeymuxOnly,
    EnvFirst,
    KeymuxFirst,
}

/// Decision source
#[derive(Debug, Clone, PartialEq)]
pub enum DecisionSource {
    EnvProjection,
    Keymux,
    Balanced { confidence: f32 },
}

/// Routing decision result
#[derive(Debug, Clone)]
pub struct RoutingDecision {
    pub provider_id: String,
    pub source: DecisionSource,
    pub confidence: f32,
    pub reason: String,
}

impl RoutingDecision {
    pub fn from_env(provider_id: &str, confidence: f32) -> Self {
        Self {
            provider_id: provider_id.to_string(),
            source: DecisionSource::EnvProjection,
            confidence,
            reason: "Selected via env projection (literbike host)".to_string(),
        }
    }
    
    pub fn from_keymux(provider_id: &str, confidence: f32) -> Self {
        Self {
            provider_id: provider_id.to_string(),
            source: DecisionSource::Keymux,
            confidence,
            reason: "Selected via keymux (literbike host)".to_string(),
        }
    }
    
    pub fn balanced(provider_id: &str, confidence: f32) -> Self {
        Self {
            provider_id: provider_id.to_string(),
            source: DecisionSource::Balanced { confidence },
            confidence,
            reason: "Selected via balanced decision (literbike unified)".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_unified_mux_state_from_env() {
        let env_pairs = vec![
            ("KILO_API_KEY".to_string(), "sk-kilo".to_string()),
        ];
        let state = UnifiedMuxState::from_env_pairs(env_pairs);
        assert!(!state.env_profile.entries.is_empty());
    }
    
    #[test]
    fn test_unified_mux_decision_env_first() {
        let env_pairs = vec![("KILO_API_KEY".to_string(), "sk-kilo".to_string())];
        let state = UnifiedMuxState::from_env_pairs(env_pairs)
            .with_precedence(PrecedenceMode::EnvFirst);
        let decision = state.make_decision();
        assert!(decision.is_some());
    }
    
    #[test]
    fn test_extract_provider_from_key() {
        assert_eq!(UnifiedMuxState::extract_provider_from_key("KILO_API_KEY"), "kilo");
        assert_eq!(UnifiedMuxState::extract_provider_from_key("MOONSHOT_API_KEY"), "moonshot");
    }
}

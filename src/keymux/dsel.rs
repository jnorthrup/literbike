//! CC-Store DSEL (Domain Specific Expression Language)
//!
//! Implements quota management and provider selection for model serving.
//! Used by Freqtrade ring agent for alpha-first model selection.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

/// Represents a provider with quota and priority information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderPotential {
    pub name: String,
    pub available_tokens: usize,
    pub priority: u8,
    pub cost_per_million: f64,
    pub is_free: bool,
    /// Quota management for free providers
    pub free_quota: Option<FreeQuotaConfig>,
    /// Unit timeframe for quota management (e.g., per hour, per day)
    pub quota_timeframe: Option<QuotaTimeframe>,
    /// Rate limiting configuration
    pub rate_limit: Option<RateLimitConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreeQuotaConfig {
    pub daily_tokens: usize,
    pub monthly_tokens: usize,
    pub reset_hour: u8,                 // 0-23
    pub reset_day_of_month: Option<u8>, // 1-28 for monthly resets
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaTimeframe {
    pub timeframe_type: TimeframeType,
    pub quota_limit: usize,
    pub current_usage: usize,
    pub reset_timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TimeframeType {
    Hourly,
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    pub requests_per_minute: u64,
    pub requests_per_hour: u64,
    pub requests_per_day: u64,
    pub burst_limit: u64,
}

/// Provider-specific quota tracking for API token ledger
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderTokenLedger {
    pub provider_name: String,
    pub total_tokens_used: u64,
    pub tokens_used_today: u64,
    pub tokens_used_this_hour: u64,
    pub last_api_check: Option<i64>,
    pub api_status: ProviderApiStatus,
    pub vague_quota_remaining: Option<VagueQuota>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProviderApiStatus {
    Healthy,
    Degraded,
    RateLimited,
    AuthenticationFailed,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VagueQuota {
    pub estimated_remaining: u64,
    pub confidence: f64, // 0.0 to 1.0
    pub last_updated: i64,
    pub source: QuotaSource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QuotaSource {
    ApiDirect,
    EstimatedFromUsage,
    ManualConfiguration,
    Unknown,
}

/// Specific provider configurations for the focused providers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KiloCodeConfig {
    pub api_key: Option<String>,
    pub base_url: String,
    pub estimated_daily_limit: u64,
    pub api_check_interval: u64, // seconds
    pub last_api_check: i64,
    pub current_ledger: ProviderTokenLedger,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenCodeConfig {
    pub api_key: Option<String>,
    pub base_url: String,
    pub estimated_daily_limit: u64,
    pub api_check_interval: u64,
    pub last_api_check: i64,
    pub current_ledger: ProviderTokenLedger,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenRouterConfig {
    pub api_key: Option<String>,
    pub base_url: String,
    pub estimated_daily_limit: u64,
    pub api_check_interval: u64,
    pub last_api_check: i64,
    pub current_ledger: ProviderTokenLedger,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NvidiaConfig {
    pub api_key: Option<String>,
    pub base_url: String,
    pub estimated_daily_limit: u64,
    pub api_check_interval: u64,
    pub last_api_check: i64,
    pub current_ledger: ProviderTokenLedger,
}

/// Moonshot (Kimi) provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoonshotConfig {
    pub api_key: Option<String>,
    pub base_url: String,
    pub estimated_daily_limit: u64,
    pub api_check_interval: u64,
    pub last_api_check: i64,
    pub current_ledger: ProviderTokenLedger,
}

/// Groq provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroqConfig {
    pub api_key: Option<String>,
    pub base_url: String,
    pub estimated_daily_limit: u64,
    pub api_check_interval: u64,
    pub last_api_check: i64,
    pub current_ledger: ProviderTokenLedger,
}

/// xAI (Grok) provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XAIConfig {
    pub api_key: Option<String>,
    pub base_url: String,
    pub estimated_daily_limit: u64,
    pub api_check_interval: u64,
    pub last_api_check: i64,
    pub current_ledger: ProviderTokenLedger,
}

/// Cerebras provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CerebrasConfig {
    pub api_key: Option<String>,
    pub base_url: String,
    pub estimated_daily_limit: u64,
    pub api_check_interval: u64,
    pub last_api_check: i64,
    pub current_ledger: ProviderTokenLedger,
}

impl ProviderPotential {
    pub fn new(
        name: &str,
        available_tokens: usize,
        priority: u8,
        cost_per_million: f64,
        is_free: bool,
    ) -> Self {
        Self {
            name: name.to_string(),
            available_tokens,
            priority,
            cost_per_million,
            is_free,
            free_quota: None,
            quota_timeframe: None,
            rate_limit: None,
        }
    }

    pub fn with_free_quota(
        mut self,
        daily_tokens: usize,
        monthly_tokens: usize,
        reset_hour: u8,
    ) -> Self {
        self.free_quota = Some(FreeQuotaConfig {
            daily_tokens,
            monthly_tokens,
            reset_hour,
            reset_day_of_month: None,
        });
        self
    }

    pub fn with_timeframe_quota(
        mut self,
        timeframe: TimeframeType,
        limit: usize,
        reset_timestamp: i64,
    ) -> Self {
        self.quota_timeframe = Some(QuotaTimeframe {
            timeframe_type: timeframe,
            quota_limit: limit,
            current_usage: 0,
            reset_timestamp,
        });
        self
    }

    pub fn with_rate_limit(
        mut self,
        per_minute: u64,
        per_hour: u64,
        per_day: u64,
        burst: u64,
    ) -> Self {
        self.rate_limit = Some(RateLimitConfig {
            requests_per_minute: per_minute,
            requests_per_hour: per_hour,
            requests_per_day: per_day,
            burst_limit: burst,
        });
        self
    }

    /// Calculate cost for given token count
    pub fn calculate_cost(&self, tokens: usize) -> f64 {
        if self.is_free {
            0.0
        } else {
            (tokens as f64 * self.cost_per_million) / 1_000_000.0
        }
    }

    /// Check if provider can handle the request considering free quotas
    pub fn can_handle(&self, tokens: usize) -> bool {
        // First check base availability
        if tokens > self.available_tokens {
            return false;
        }

        // Check free quota limits if provider is free
        if let Some(free_quota) = &self.free_quota {
            // This would need to track usage across timeframes
            // For now, assume we have tracking logic elsewhere
            return tokens <= free_quota.daily_tokens;
        }

        // Check timeframe-based quotas
        if let Some(timeframe) = &self.quota_timeframe {
            return (timeframe.current_usage + tokens) <= timeframe.quota_limit;
        }

        true
    }

    /// Get priority score (lower is better)
    pub fn get_priority_score(&self) -> u8 {
        // Free providers get bonus priority
        if self.is_free {
            self.priority.saturating_sub(1)
        } else {
            self.priority
        }
    }

    /// Check if rate limited
    pub fn is_rate_limited(&self, current_requests: u64, timeframe: &str) -> bool {
        match (&self.rate_limit, timeframe) {
            (Some(limit), "minute") => current_requests > limit.requests_per_minute,
            (Some(limit), "hour") => current_requests > limit.requests_per_hour,
            (Some(limit), "day") => current_requests > limit.requests_per_day,
            _ => false,
        }
    }
}

/// Container for managing quota across multiple providers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaContainer {
    pub name: String,
    pub providers: HashMap<String, ProviderPotential>,
    pub total_quota: usize,
    pub used_quota: usize,
}

impl QuotaContainer {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            providers: HashMap::new(),
            total_quota: 0,
            used_quota: 0,
        }
    }

    /// Add a provider to the container
    pub fn add_provider(
        &mut self,
        name: &str,
        tokens: usize,
        priority: u8,
        cost_per_million: f64,
        is_free: bool,
    ) {
        let provider = ProviderPotential::new(name, tokens, priority, cost_per_million, is_free);
        self.providers.insert(name.to_string(), provider);
        self.total_quota += tokens;
    }

    /// Add a provider from a ProviderPotential struct
    pub fn add_provider_from_struct(&mut self, provider: ProviderPotential) {
        let tokens = provider.available_tokens;
        self.providers.insert(provider.name.clone(), provider);
        self.total_quota += tokens;
    }

    /// Add free provider with quota configuration
    pub fn add_free_provider(
        &mut self,
        name: &str,
        tokens: usize,
        priority: u8,
        daily_tokens: usize,
        monthly_tokens: usize,
        reset_hour: u8,
    ) {
        let provider = ProviderPotential::new(name, tokens, priority, 0.0, true).with_free_quota(
            daily_tokens,
            monthly_tokens,
            reset_hour,
        );
        self.add_provider_from_struct(provider);
    }

    /// Add provider with timeframe-based quota
    pub fn add_timeframe_provider(
        &mut self,
        name: &str,
        tokens: usize,
        priority: u8,
        cost_per_million: f64,
        timeframe: TimeframeType,
        quota_limit: usize,
        reset_timestamp: i64,
    ) {
        let provider = ProviderPotential::new(name, tokens, priority, cost_per_million, false)
            .with_timeframe_quota(timeframe, quota_limit, reset_timestamp);
        self.add_provider_from_struct(provider);
    }

    /// Check if we can allocate the requested tokens
    pub fn can_allocate(&self, tokens: usize) -> bool {
        let available = self.total_quota - self.used_quota;
        tokens <= available
    }

    /// Allocate tokens from the best provider
    pub fn allocate(&mut self, tokens: usize) -> Option<&ProviderPotential> {
        if !self.can_allocate(tokens) {
            return None;
        }

        // Find best provider based on priority and availability
        let best_provider = self
            .providers
            .values()
            .filter(|p| p.can_handle(tokens))
            .min_by_key(|p| p.get_priority_score());

        if let Some(provider) = best_provider {
            self.used_quota += tokens;
            Some(provider)
        } else {
            None
        }
    }

    /// Select provider for request (read-only)
    pub fn select_provider(&self, tokens: usize) -> Option<&ProviderPotential> {
        self.providers
            .values()
            .filter(|p| p.can_handle(tokens))
            .min_by_key(|p| p.get_priority_score())
    }

    /// Get provider by name
    pub fn get_provider(&self, name: &str) -> Option<&ProviderPotential> {
        self.providers.get(name)
    }

    /// Get all providers sorted by priority
    pub fn get_providers_by_priority(&self) -> Vec<&ProviderPotential> {
        let mut providers: Vec<&ProviderPotential> = self.providers.values().collect();
        providers.sort_by_key(|p| p.get_priority_score());
        providers
    }
}

/// DSEL Builder for constructing quota containers with hierarchical prefix support
pub struct DSELBuilder {
    container: QuotaContainer,
    prefix_transformations: HashMap<String, Vec<String>>,
}

impl DSELBuilder {
    pub fn new() -> Self {
        Self {
            container: QuotaContainer::new("default"),
            prefix_transformations: HashMap::new(),
        }
    }

    pub fn with_quota(mut self, name: &str, total_quota: usize) -> Self {
        self.container.name = name.to_string();
        self.container.total_quota = total_quota;
        self
    }

    pub fn with_provider(
        mut self,
        name: &str,
        tokens: usize,
        priority: u8,
        cost_per_million: f64,
        is_free: bool,
    ) -> Self {
        self.container
            .add_provider(name, tokens, priority, cost_per_million, is_free);
        self
    }

    /// Add free provider with quota configuration
    pub fn with_free_provider(
        mut self,
        name: &str,
        tokens: usize,
        priority: u8,
        daily_tokens: usize,
        monthly_tokens: usize,
        reset_hour: u8,
    ) -> Self {
        self.container.add_free_provider(
            name,
            tokens,
            priority,
            daily_tokens,
            monthly_tokens,
            reset_hour,
        );
        self
    }

    /// Add provider with timeframe-based quota
    pub fn with_timeframe_provider(
        mut self,
        name: &str,
        tokens: usize,
        priority: u8,
        cost_per_million: f64,
        timeframe: TimeframeType,
        quota_limit: usize,
        reset_timestamp: i64,
    ) -> Self {
        self.container.add_timeframe_provider(
            name,
            tokens,
            priority,
            cost_per_million,
            timeframe,
            quota_limit,
            reset_timestamp,
        );
        self
    }

    /// Add provider with rate limiting
    pub fn with_rate_limited_provider(
        mut self,
        name: &str,
        tokens: usize,
        priority: u8,
        cost_per_million: f64,
        per_minute: u64,
        per_hour: u64,
        per_day: u64,
        burst: u64,
    ) -> Self {
        let provider = ProviderPotential::new(name, tokens, priority, cost_per_million, false)
            .with_rate_limit(per_minute, per_hour, per_day, burst);
        self.container.add_provider_from_struct(provider);
        self
    }

    /// Add hierarchical prefix transformation patterns
    /// Example: "/litellm/litellm/litellm/gpt-4" -> "/litellm/gpt-4"
    pub fn with_prefix_transformation(mut self, from_prefix: &str, to_prefix: &str) -> Self {
        self.prefix_transformations
            .entry(from_prefix.to_string())
            .or_insert_with(Vec::new)
            .push(to_prefix.to_string());
        self
    }

    pub fn build(self) -> Result<QuotaContainer, String> {
        if self.container.providers.is_empty() {
            return Err("No providers defined".to_string());
        }
        if self.container.total_quota == 0 {
            return Err("Total quota must be greater than zero".to_string());
        }
        Ok(self.container)
    }

    /// Build a complete RuleEngine with prefix handling integrated with quota management
    /// This creates a fully-configured DSEL engine that can:
    /// 1. Transform hierarchical model IDs to canonical form
    /// 2. Select providers based on quota availability and priority
    /// 3. Track token usage across providers
    pub fn build_with_rule_engine(mut self) -> Result<RuleEngine, String> {
        if self.container.providers.is_empty() {
            return Err("No providers defined".to_string());
        }

        // Create hierarchical model selector with quota container
        let mut hierarchical_selector = HierarchicalModelSelector::new(self.container.clone());

        // Add prefix transformation rules from DSEL configuration
        for (from_prefix, to_prefixes) in &self.prefix_transformations {
            for to_prefix in to_prefixes {
                hierarchical_selector.add_transformation_rule(from_prefix, to_prefix, 100);
            }
        }

        // Also add common transformation rules for known bad agent concatenations
        // These handle patterns like /litellm/litellm/litellm/ -> /litellm/
        hierarchical_selector.add_transformation_rule("/litellm/litellm/litellm/", "/litellm/", 100);
        hierarchical_selector.add_transformation_rule("/ccswitch/ccswitch/ccswitch/", "/ccswitch/", 90);
        hierarchical_selector.add_transformation_rule("/openai/openai/openai/", "/openai/", 80);
        hierarchical_selector.add_transformation_rule("/anthropic/anthropic/anthropic/", "/anthropic/", 85);

        // Create and configure rule engine
        let mut rule_engine = RuleEngine::new();
        rule_engine.set_hierarchical_selector(hierarchical_selector);
        rule_engine.enable_token_ledger();

        // Initialize quota tracking from container providers
        for (name, provider) in &self.container.providers {
            let tracking = ProviderQuotaTracking {
                provider_name: name.clone(),
                tokens_used_today: 0,
                tokens_used_this_hour: 0,
                estimated_remaining_quota: provider.available_tokens as u64,
                quota_confidence: 0.9,
                last_quota_update: Self::current_timestamp(),
            };
            rule_engine.quota_tracking.insert(name.clone(), tracking);
        }

        Ok(rule_engine)
    }

    /// Build a QuotaContainer with prefix transformation support
    /// Returns both the container and the hierarchical selector for direct use
    pub fn build_with_hierarchical_selector(mut self) -> Result<(QuotaContainer, HierarchicalModelSelector), String> {
        if self.container.providers.is_empty() {
            return Err("No providers defined".to_string());
        }

        // Create hierarchical model selector
        let mut hierarchical_selector = HierarchicalModelSelector::new(self.container.clone());

        // Add prefix transformation rules
        for (from_prefix, to_prefixes) in &self.prefix_transformations {
            for to_prefix in to_prefixes {
                hierarchical_selector.add_transformation_rule(from_prefix, to_prefix, 100);
            }
        }

        // Add common transformation rules
        hierarchical_selector.add_transformation_rule("/litellm/litellm/litellm/", "/litellm/", 100);
        hierarchical_selector.add_transformation_rule("/ccswitch/ccswitch/ccswitch/", "/ccswitch/", 90);
        hierarchical_selector.add_transformation_rule("/openai/openai/openai/", "/openai/", 80);
        hierarchical_selector.add_transformation_rule("/anthropic/anthropic/anthropic/", "/anthropic/", 85);

        Ok((self.container, hierarchical_selector))
    }

    fn current_timestamp() -> i64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
    }
}

/// Hierarchical model selector that handles prefix transformations
pub struct HierarchicalModelSelector {
    base_selector: QuotaContainer,
    prefix_cache: HashMap<String, String>,
    transformation_rules: Vec<PrefixTransformation>,
}

#[derive(Debug, Clone)]
pub struct PrefixTransformation {
    pub pattern: String,
    pub replacement: String,
    pub priority: u8,
}

impl HierarchicalModelSelector {
    pub fn new(base_selector: QuotaContainer) -> Self {
        Self {
            base_selector,
            prefix_cache: HashMap::new(),
            transformation_rules: Vec::new(),
        }
    }

    /// Add transformation rules for hierarchical model IDs
    pub fn add_transformation_rule(&mut self, pattern: &str, replacement: &str, priority: u8) {
        self.transformation_rules.push(PrefixTransformation {
            pattern: pattern.to_string(),
            replacement: replacement.to_string(),
            priority,
        });

        // Sort by priority (higher priority first)
        self.transformation_rules
            .sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    /// Transform hierarchical model ID to best approximation
    pub fn transform_model_id(&mut self, model_id: &str) -> String {
        // Check cache first
        if let Some(cached) = self.prefix_cache.get(model_id) {
            return cached.clone();
        }

        let mut best_match = model_id.to_string();
        let mut best_score = 0;
        let mut best_rule = None;

        // Try each transformation rule
        for rule in &self.transformation_rules {
            if let Some(matched) = self.apply_transformation_rule(model_id, rule) {
                // Calculate score based on rule priority and match quality
                let score = rule.priority as u32 * 100
                    + self.calculate_match_quality(matched.len(), model_id.len());

                if score > best_score {
                    best_score = score;
                    best_match = matched;
                    best_rule = Some(rule);
                }
            }
        }

        // Store in cache
        if let Some(rule) = best_rule {
            println!(
                "Transformed {} -> {} using rule: {}",
                model_id, best_match, rule.pattern
            );
        }

        self.prefix_cache
            .insert(model_id.to_string(), best_match.clone());
        best_match
    }

    /// Apply a single transformation rule
    fn apply_transformation_rule(
        &self,
        model_id: &str,
        rule: &PrefixTransformation,
    ) -> Option<String> {
        // Handle various transformation patterns
        let patterns = vec![
            // Exact prefix match: /litellm/litellm/litellm/gpt-4 -> /litellm/gpt-4
            format!(
                "^{}{}",
                rule.pattern,
                if rule.pattern.ends_with('/') { "" } else { "/" }
            ),
            // Multiple repetitions: /litellm/litellm/litellm/ -> /litellm/
            format!(
                "^{}(?:{}(?:{}/)+)",
                rule.pattern, rule.pattern, rule.pattern
            ),
            // Partial match with hierarchical meaning
            format!("^{}(.*)", rule.pattern),
        ];

        for pattern in patterns {
            if let Some(replaced) = self.replace_pattern(model_id, &pattern, &rule.replacement) {
                return Some(replaced);
            }
        }

        None
    }

    /// Replace pattern in model ID
    fn replace_pattern(&self, model_id: &str, pattern: &str, replacement: &str) -> Option<String> {
        use regex::Regex;

        if let Ok(regex) = Regex::new(pattern) {
            if regex.is_match(model_id) {
                let result = regex.replace(model_id, replacement);
                return Some(result.to_string());
            }
        }
        None
    }

    /// Calculate match quality score
    fn calculate_match_quality(&self, transformed_len: usize, original_len: usize) -> u32 {
        // Prefer transformations that:
        // 1. Remove redundant prefixes (shorter is better)
        // 2. Maintain hierarchical meaning
        // 3. Don't over-transform

        if transformed_len < original_len {
            // Good: removed redundancy
            (original_len - transformed_len) as u32 * 10
        } else if transformed_len == original_len {
            // Neutral: no change
            0
        } else {
            // Bad: made longer
            0
        }
    }

    /// Handle complex hierarchical transformations
    pub fn handle_complex_transformations(&self, model_id: &str) -> Vec<String> {
        let mut transformations = Vec::new();

        // Example transformations:
        // 1. /litellm/litellm/litellm/gpt-4 -> /litellm/gpt-4
        // 2. /ccswitch/ccswitch/openai/gpt-4 -> /openai/gpt-4
        // 3. /provider/provider/provider/model -> /provider/model

        let patterns = vec![
            (r"^/litellm/litellm/litellm/(.+)$", "/litellm/$1"),
            (r"^/ccswitch/ccswitch/ccswitch/(.+)$", "/ccswitch/$1"),
            (r"^/openai/openai/openai/(.+)$", "/openai/$1"),
            (r"^/anthropic/anthropic/anthropic/(.+)$", "/anthropic/$1"),
            (r"^/(.+)/\1/\1/(.+)$", "/$1/$2"),
            (r"^/(.+)/\1/(.+)$", "/$1/$2"),
        ];

        for (pattern, replacement) in patterns {
            if let Ok(regex) = regex::Regex::new(pattern) {
                if regex.is_match(model_id) {
                    let transformed = regex.replace(model_id, replacement).to_string();
                    if !transformations.contains(&transformed) {
                        transformations.push(transformed);
                    }
                }
            }
        }

        // If no patterns matched, try simpler transformations
        if transformations.is_empty() {
            let parts: Vec<&str> = model_id.split('/').filter(|s| !s.is_empty()).collect();
            if parts.len() >= 2 {
                // Extract the last provider and model name
                let provider = parts[parts.len() - 2];
                let model = parts[parts.len() - 1];
                transformations.push(format!("/{}/{}", provider, model));
            }
        }

        transformations
    }

    /// Select best provider approximation for hierarchical model ID
    pub fn select_best_approximation(
        &self,
        hierarchical_model_id: &str,
    ) -> Option<&ProviderPotential> {
        // For now, just use the hierarchical_model_id directly to find provider
        // In a real implementation, you might want to cache transformations
        let parts: Vec<&str> = hierarchical_model_id
            .split('/')
            .filter(|s| !s.is_empty())
            .collect();

        if let Some(provider_name) = parts.first() {
            return self.base_selector.get_provider(provider_name);
        }

        None
    }
}

impl Default for DSELBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// DSL for provider selection rules
pub struct ProviderSelectionRule {
    pub name: String,
    pub conditions: Vec<SelectionCondition>,
    pub priority: u8,
}

pub enum SelectionCondition {
    MaxTokens(usize),
    CostThreshold(f64),
    FreeOnly,
    ProviderName(String),
}

impl ProviderSelectionRule {
    pub fn new(name: &str, priority: u8) -> Self {
        Self {
            name: name.to_string(),
            conditions: Vec::new(),
            priority,
        }
    }

    pub fn with_max_tokens(mut self, max_tokens: usize) -> Self {
        self.conditions
            .push(SelectionCondition::MaxTokens(max_tokens));
        self
    }

    pub fn with_cost_threshold(mut self, threshold: f64) -> Self {
        self.conditions
            .push(SelectionCondition::CostThreshold(threshold));
        self
    }

    pub fn with_free_only(mut self) -> Self {
        self.conditions.push(SelectionCondition::FreeOnly);
        self
    }

    pub fn with_provider(mut self, provider: &str) -> Self {
        self.conditions
            .push(SelectionCondition::ProviderName(provider.to_string()));
        self
    }

    /// Check if provider matches this rule
    pub fn matches(&self, provider: &ProviderPotential, tokens: usize) -> bool {
        for condition in &self.conditions {
            match condition {
                SelectionCondition::MaxTokens(max) => {
                    if tokens > *max {
                        return false;
                    }
                }
                SelectionCondition::CostThreshold(threshold) => {
                    if provider.calculate_cost(tokens) > *threshold {
                        return false;
                    }
                }
                SelectionCondition::FreeOnly => {
                    if !provider.is_free {
                        return false;
                    }
                }
                SelectionCondition::ProviderName(name) => {
                    if &provider.name != name {
                        return false;
                    }
                }
            }
        }
        true
    }
}

/// Rule-based provider selection engine with hierarchical support
pub struct RuleEngine {
    rules: Vec<ProviderSelectionRule>,
    hierarchical_selector: Option<HierarchicalModelSelector>,
    token_ledger_enabled: bool,
    quota_tracking: HashMap<String, ProviderQuotaTracking>,
    /// Metrics for observability
    metrics: DSELMetrics,
}

/// Metrics and logging for DSEL operations
#[derive(Debug, Default)]
pub struct DSELMetrics {
    /// Total provider selections made
    pub total_selections: AtomicU64,
    /// Selections by provider name
    pub selections_by_provider: Mutex<HashMap<String, u64>>,
    /// Quota violations (requests that exceeded quota)
    pub quota_violations: AtomicU64,
    /// Hierarchical transformations applied
    pub hierarchical_transforms: AtomicU64,
    /// Token usage tracked
    pub total_tokens_tracked: AtomicU64,
    /// Rate limit hits
    pub rate_limit_hits: AtomicU64,
    /// Fallback selections (second choice or later)
    pub fallback_selections: AtomicU64,
}

impl DSELMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a provider selection
    pub fn record_selection(&self, provider: &str, is_fallback: bool) {
        self.total_selections.fetch_add(1, Ordering::Relaxed);
        let mut selections = self.selections_by_provider.lock().unwrap();
        *selections.entry(provider.to_string()).or_insert(0) += 1;
        if is_fallback {
            self.fallback_selections.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Record quota violation
    pub fn record_quota_violation(&self) {
        self.quota_violations.fetch_add(1, Ordering::Relaxed);
    }

    /// Record hierarchical transformation
    pub fn record_hierarchical_transform(&self) {
        self.hierarchical_transforms.fetch_add(1, Ordering::Relaxed);
    }

    /// Record token tracking
    pub fn record_token_usage(&self, tokens: u64) {
        self.total_tokens_tracked.fetch_add(tokens, Ordering::Relaxed);
    }

    /// Record rate limit hit
    pub fn record_rate_limit(&self) {
        self.rate_limit_hits.fetch_add(1, Ordering::Relaxed);
    }

    /// Get selection statistics
    pub fn get_selection_stats(&self) -> (u64, u64, f64) {
        let total = self.total_selections.load(Ordering::Relaxed);
        let fallbacks = self.fallback_selections.load(Ordering::Relaxed);
        let fallback_rate = if total > 0 {
            (fallbacks as f64 / total as f64) * 100.0
        } else {
            0.0
        };
        (total, fallbacks, fallback_rate)
    }

    /// Get top providers by usage
    pub fn get_top_providers(&self, limit: usize) -> Vec<(String, u64)> {
        let providers: Vec<(String, u64)> = self.selections_by_provider.lock().unwrap().iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        let mut sorted = providers;
        sorted.sort_by(|a, b| b.1.cmp(&a.1));
        sorted.into_iter().take(limit).collect()
    }

    /// Export metrics as JSON-serializable structure
    pub fn export(&self) -> serde_json::Value {
        let (total, fallbacks, fallback_rate) = self.get_selection_stats();
        serde_json::json!({
            "total_selections": self.total_selections.load(Ordering::Relaxed),
            "selections_by_provider": *self.selections_by_provider.lock().unwrap(),
            "quota_violations": self.quota_violations.load(Ordering::Relaxed),
            "hierarchical_transforms": self.hierarchical_transforms.load(Ordering::Relaxed),
            "total_tokens_tracked": self.total_tokens_tracked.load(Ordering::Relaxed),
            "rate_limit_hits": self.rate_limit_hits.load(Ordering::Relaxed),
            "fallback_selections": fallbacks,
            "fallback_rate_percent": fallback_rate,
            "top_providers": self.get_top_providers(5)
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderQuotaTracking {
    pub provider_name: String,
    pub tokens_used_today: u64,
    pub tokens_used_this_hour: u64,
    pub estimated_remaining_quota: u64,
    pub quota_confidence: f64,
    pub last_quota_update: i64,
}

impl RuleEngine {
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            hierarchical_selector: None,
            token_ledger_enabled: false,
            quota_tracking: HashMap::new(),
            metrics: DSELMetrics::new(),
        }
    }

    /// Get current metrics
    pub fn get_metrics(&self) -> &DSELMetrics {
        &self.metrics
    }

    /// Get mutable metrics for advanced operations
    pub fn get_metrics_mut(&mut self) -> &mut DSELMetrics {
        &mut self.metrics
    }

    /// Enable token ledger tracking for providers
    pub fn enable_token_ledger(&mut self) {
        self.token_ledger_enabled = true;
        log::info!("DSEL: Token ledger enabled");

        // Initialize quota tracking for specific providers
        let providers = vec!["kilo_code", "opencode", "openrouter", "nvidia", "moonshot", "groq", "xai", "cerebras"];
        let provider_count = providers.len();
        for provider in &providers {
            self.quota_tracking.insert(
                provider.to_string(),
                ProviderQuotaTracking {
                    provider_name: provider.to_string(),
                    tokens_used_today: 0,
                    tokens_used_this_hour: 0,
                    estimated_remaining_quota: match *provider {
                        "kilo_code" => 1_000_000,
                        "opencode" => 500_000,
                        "openrouter" => 2_000_000,
                        "nvidia" => 3_000_000,
                        "moonshot" => 1_500_000, // Moonshot/Kimi typical quota
                        "groq" => 2_000_000,    // Groq typical quota
                        "xai" => 1_500_000,     // xAI/Grok typical quota
                        "cerebras" => 2_000_000, // Cerebras typical quota
                        _ => 0,
                    },
                    quota_confidence: 0.8,
                    last_quota_update: Self::current_timestamp(),
                },
            );
        }
        log::info!("DSEL: Initialized quota tracking for {} providers", provider_count);
    }

    pub fn add_rule(&mut self, rule: ProviderSelectionRule) {
        self.rules.push(rule);
    }

    /// Set hierarchical model selector for prefix transformations
    pub fn set_hierarchical_selector(&mut self, selector: HierarchicalModelSelector) {
        self.hierarchical_selector = Some(selector);
    }

    /// Track token usage for provider quota management
    pub fn track_token_usage(&mut self, provider: &str, tokens: u64) -> Result<(), String> {
        if !self.token_ledger_enabled {
            return Ok(()); // Tracking not enabled
        }

        let tracking = self
            .quota_tracking
            .entry(provider.to_string())
            .or_insert_with(|| ProviderQuotaTracking {
                provider_name: provider.to_string(),
                tokens_used_today: 0,
                tokens_used_this_hour: 0,
                estimated_remaining_quota: match provider {
                    "kilo_code" => 1_000_000,
                    "opencode" => 500_000,
                    "openrouter" => 2_000_000,
                    "nvidia" => 3_000_000,
                    "moonshot" => 1_500_000,
                    "groq" => 2_000_000,
                    "xai" => 1_500_000,
                    "cerebras" => 2_000_000,
                    _ => 100_000, // Default for unknown providers
                },
                quota_confidence: 0.7,
                last_quota_update: Self::current_timestamp(),
            });

        tracking.tokens_used_today += tokens;
        tracking.tokens_used_this_hour += tokens;

        // Update estimated remaining quota
        if tracking.estimated_remaining_quota >= tokens {
            tracking.estimated_remaining_quota -= tokens;
            tracking.quota_confidence *= 0.99; // Slight decrease in confidence
        } else {
            tracking.estimated_remaining_quota = 0;
            tracking.quota_confidence = 0.0;
            log::warn!("DSEL: Provider {} quota exhausted", provider);
        }

        tracking.last_quota_update = Self::current_timestamp();
        
        // Record metrics
        self.metrics.record_token_usage(tokens);
        log::debug!("DSEL: Tracked {} tokens for provider {}", tokens, provider);

        Ok(())
    }

    /// Check if provider has sufficient quota
    pub fn has_sufficient_quota(&self, provider: &str, tokens_needed: u64) -> bool {
        if !self.token_ledger_enabled {
            return true; // No quota checking if not enabled
        }

        if let Some(tracking) = self.quota_tracking.get(provider) {
            return tracking.estimated_remaining_quota >= tokens_needed;
        }

        false
    }

    /// Get quota status for provider
    pub fn get_quota_status(&self, provider: &str) -> Option<(u64, u64, f64)> {
        self.quota_tracking.get(provider).map(|t| {
            (
                t.tokens_used_today,
                t.estimated_remaining_quota,
                t.quota_confidence,
            )
        })
    }

    /// Reset hourly usage (call this hourly)
    pub fn reset_hourly_usage(&mut self) {
        for tracking in self.quota_tracking.values_mut() {
            tracking.tokens_used_this_hour = 0;
        }
    }

    /// Reset daily usage (call this daily)
    pub fn reset_daily_usage(&mut self) {
        for tracking in self.quota_tracking.values_mut() {
            tracking.tokens_used_today = 0;
            // Reset to initial quota levels
            tracking.estimated_remaining_quota = match tracking.provider_name.as_str() {
                "kilo_code" => 1_000_000,
                "opencode" => 500_000,
                "openrouter" => 2_000_000,
                "nvidia" => 3_000_000,
                "moonshot" => 1_500_000,
                "groq" => 2_000_000,
                "xai" => 1_500_000,
                "cerebras" => 2_000_000,
                _ => tracking.estimated_remaining_quota,
            };
            tracking.quota_confidence = 0.8;
        }
    }

    /// Get all quota tracking data
    pub fn get_all_quota_tracking(&self) -> &HashMap<String, ProviderQuotaTracking> {
        &self.quota_tracking
    }

    fn current_timestamp() -> i64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
    }

    /// Select provider based on rules with hierarchical model ID support and quota tracking
    pub fn select_provider<'a>(
        &'a self,
        providers: &'a [ProviderPotential],
        tokens: usize,
        model_id: Option<&str>,
    ) -> Option<&'a ProviderPotential> {
        // If model_id is provided and hierarchical selector exists, try to find best approximation
        if let (Some(model_id), Some(selector)) = (model_id, &self.hierarchical_selector) {
            // Record hierarchical transformation attempt
            self.metrics.record_hierarchical_transform();
            
            // Handle complex hierarchical transformations
            let transformations = selector.handle_complex_transformations(model_id);
            log::debug!("DSEL: Transforming hierarchical model ID: {} -> {} transformations", 
                       model_id, transformations.len());

            // Try each transformation to find a matching provider
            for transformed in transformations {
                // Parse provider from transformed ID
                let parts: Vec<&str> = transformed.split('/').filter(|s| !s.is_empty()).collect();
                if let Some(provider_name) = parts.first() {
                    // Find matching provider
                    for provider in providers {
                        if provider.name == *provider_name && provider.can_handle(tokens) {
                            // Check quota if enabled
                            if self.token_ledger_enabled
                                && !self.has_sufficient_quota(&provider.name, tokens as u64)
                            {
                                log::debug!("DSEL: Provider {} has insufficient quota", provider.name);
                                continue; // Skip providers with insufficient quota
                            }

                            // Apply rule filtering if any
                            if self.rules.is_empty()
                                || self.rules.iter().any(|rule| rule.matches(provider, tokens))
                            {
                                log::info!("DSEL: Selected provider {} for transformed model {}", 
                                          provider.name, transformed);
                                return Some(provider);
                            }
                        }
                    }
                }
            }
        }

        // Fallback to standard rule-based selection with quota consideration
        let result = self.select_provider_by_rules_with_quota(providers, tokens);
        if let Some(provider) = result {
            log::debug!("DSEL: Selected provider {} via standard selection", provider.name);
        } else {
            log::warn!("DSEL: No provider found for {} tokens", tokens);
            self.metrics.record_quota_violation();
        }
        result
    }

    /// Standard rule-based provider selection
    fn select_provider_by_rules<'a>(
        &'a self,
        providers: &'a [ProviderPotential],
        tokens: usize,
    ) -> Option<&'a ProviderPotential> {
        // Find all matching providers
        let matches: Vec<&ProviderPotential> =
            providers.iter().filter(|p| p.can_handle(tokens)).collect();

        // Apply rules in priority order
        for rule in self.rules.iter() {
            for provider in &matches {
                if rule.matches(provider, tokens) {
                    return Some(*provider);
                }
            }
        }

        // Fallback to highest priority provider
        matches
            .iter()
            .min_by_key(|p| p.get_priority_score())
            .copied()
    }

    /// Standard rule-based provider selection with quota consideration
    fn select_provider_by_rules_with_quota<'a>(
        &'a self,
        providers: &'a [ProviderPotential],
        tokens: usize,
    ) -> Option<&'a ProviderPotential> {
        // Find all matching providers
        let mut matches: Vec<&ProviderPotential> =
            providers.iter().filter(|p| p.can_handle(tokens)).collect();

        // Filter out providers with insufficient quota if ledger is enabled
        if self.token_ledger_enabled {
            matches = matches
                .into_iter()
                .filter(|p| self.has_sufficient_quota(&p.name, tokens as u64))
                .collect();
        }

        // Apply rules in priority order
        for rule in self.rules.iter() {
            for provider in &matches {
                if rule.matches(provider, tokens) {
                    return Some(*provider);
                }
            }
        }

        // Fallback to highest priority provider
        matches
            .iter()
            .min_by_key(|p| p.get_priority_score())
            .copied()
    }

    /// Enhanced selection with hierarchical model ID and token count
    pub fn select_provider_enhanced<'a>(
        &'a self,
        providers: &'a [ProviderPotential],
        hierarchical_model_id: &str,
        tokens: usize,
    ) -> Option<&'a ProviderPotential> {
        // First, try direct provider match
        let parts: Vec<&str> = hierarchical_model_id
            .split('/')
            .filter(|s| !s.is_empty())
            .collect();
        if let Some(provider_name) = parts.first() {
            for provider in providers {
                if provider.name == *provider_name && provider.can_handle(tokens) {
                    if self.rules.is_empty()
                        || self.rules.iter().any(|rule| rule.matches(provider, tokens))
                    {
                        return Some(provider);
                    }
                }
            }
        }

        // If hierarchical selector exists, try transformations
        if let Some(selector) = &self.hierarchical_selector {
            let best_approximation = selector.select_best_approximation(hierarchical_model_id);
            if let Some(provider) = best_approximation {
                if provider.can_handle(tokens) {
                    return Some(provider);
                }
            }
        }

        // Fallback to standard selection
        self.select_provider_by_rules(providers, tokens)
    }
}

/// Hierarchical model ID processor for DSEL
pub struct HierarchicalModelProcessor {
    transformations: Vec<(String, String)>,
    provider_mappings: HashMap<String, Vec<String>>,
}

impl HierarchicalModelProcessor {
    pub fn new() -> Self {
        Self {
            transformations: Vec::new(),
            provider_mappings: HashMap::new(),
        }
    }

    /// Add transformation rule for hierarchical model IDs
    pub fn add_transformation(&mut self, pattern: &str, replacement: &str) {
        self.transformations
            .push((pattern.to_string(), replacement.to_string()));
    }

    /// Add provider mapping for hierarchical naming
    pub fn add_provider_mapping(&mut self, provider: &str, aliases: Vec<&str>) {
        self.provider_mappings.insert(
            provider.to_string(),
            aliases.iter().map(|s| s.to_string()).collect(),
        );
    }

    /// Process hierarchical model ID and return best approximation
    pub fn process_model_id(&self, model_id: &str) -> (String, String) {
        // Apply transformations
        let mut processed = model_id.to_string();

        for (pattern, replacement) in &self.transformations {
            if let Ok(regex) = regex::Regex::new(pattern) {
                processed = regex.replace(&processed, replacement).to_string();
            }
        }

        // Extract provider and model
        let parts: Vec<&str> = processed.split('/').filter(|s| !s.is_empty()).collect();

        if parts.len() >= 2 {
            let provider = parts[0].to_string();
            let model = parts[1..].join("/");

            // Check if provider has aliases
            for (canonical, aliases) in &self.provider_mappings {
                if aliases.contains(&provider) || provider == *canonical {
                    return (canonical.clone(), model);
                }
            }

            (provider, model)
        } else if parts.len() == 1 {
            // Assume it's a model name with default provider
            ("unknown".to_string(), parts[0].to_string())
        } else {
            ("unknown".to_string(), processed)
        }
    }

    /// Find best provider approximation for hierarchical model ID
    pub fn find_best_provider_approximation(
        &self,
        hierarchical_model_id: &str,
        available_providers: &[&str],
    ) -> Option<String> {
        let (provider, _) = self.process_model_id(hierarchical_model_id);

        // Check if processed provider is available
        if available_providers.contains(&provider.as_str()) {
            return Some(provider);
        }

        // Check aliases
        for (canonical, aliases) in &self.provider_mappings {
            if available_providers.contains(&canonical.as_str()) {
                // Check if hierarchical ID matches any alias
                for alias in aliases {
                    if hierarchical_model_id.contains(alias) {
                        return Some(canonical.clone());
                    }
                }
            }
        }

        None
    }
}

/// Route a model ID to (provider_name, base_url, key_env_var).
/// Model IDs use "provider/model-name" convention.
/// Returns None if no provider can be resolved.
pub fn route(model: &str) -> Option<(String, String, String)> {
    let provider = model.split('/').next().unwrap_or(model);

    match provider {
        "anthropic" => Some((
            "anthropic".into(),
            "https://api.anthropic.com/v1".into(),
            "ANTHROPIC_API_KEY".into(),
        )),
        "openai" => Some((
            "openai".into(),
            "https://api.openai.com/v1".into(),
            "OPENAI_API_KEY".into(),
        )),
        "google" | "gemini" => Some((
            "google".into(),
            "https://generativelanguage.googleapis.com/v1beta/openai".into(),
            "GOOGLE_API_KEY".into(),
        )),
        "groq" => Some((
            "groq".into(),
            "https://api.groq.com/openai/v1".into(),
            "GROQ_API_KEY".into(),
        )),
        "openrouter" => Some((
            "openrouter".into(),
            "https://openrouter.ai/api/v1".into(),
            "OPENROUTER_API_KEY".into(),
        )),
        "mistral" => Some((
            "mistral".into(),
            "https://api.mistral.ai/v1".into(),
            "MISTRAL_API_KEY".into(),
        )),
        "xai" | "grok" => Some((
            "xai".into(),
            "https://api.x.ai/v1".into(),
            "XAI_API_KEY".into(),
        )),
        "cerebras" => Some((
            "cerebras".into(),
            "https://api.cerebras.ai/v1".into(),
            "CEREBRAS_API_KEY".into(),
        )),
        "ollama" => Some((
            "ollama".into(),
            std::env::var("OLLAMA_HOST")
                .unwrap_or_else(|_| "http://localhost:11434/v1".into()),
            String::new(),
        )),
        "lmstudio" => Some((
            "lmstudio".into(),
            "http://localhost:1234/v1".into(),
            String::new(),
        )),
        _ => None,
    }
}

/// Track token usage for quota accounting.
pub fn track_tokens(provider: &str, tokens: u64) -> Result<(), String> {
    log::debug!("token usage: provider={} tokens={}", provider, tokens);
    Ok(())
}

/// A discovered/configured provider with its routing info.
#[derive(Debug, Clone)]
pub struct ProviderDef {
    pub name: String,
    pub base_url: String,
    pub key_env: String,
}

/// Return all providers that have an API key set in the environment.
pub fn discover_providers() -> Vec<ProviderDef> {
    let candidates = [
        ("anthropic",   "https://api.anthropic.com/v1",                           "ANTHROPIC_API_KEY"),
        ("openai",      "https://api.openai.com/v1",                              "OPENAI_API_KEY"),
        ("google",      "https://generativelanguage.googleapis.com/v1beta/openai","GOOGLE_API_KEY"),
        ("gemini",      "https://generativelanguage.googleapis.com/v1beta/openai","GEMINI_API_KEY"),
        ("deepseek",    "https://api.deepseek.com/v1",                            "DEEPSEEK_API_KEY"),
        ("groq",        "https://api.groq.com/openai/v1",                         "GROQ_API_KEY"),
        ("openrouter",  "https://openrouter.ai/api/v1",                           "OPENROUTER_API_KEY"),
        ("mistral",     "https://api.mistral.ai/v1",                              "MISTRAL_API_KEY"),
        ("xai",         "https://api.x.ai/v1",                                    "XAI_API_KEY"),
        ("cerebras",    "https://api.cerebras.ai/v1",                             "CEREBRAS_API_KEY"),
        ("nvidia",      "https://integrate.api.nvidia.com/v1",                    "NVIDIA_API_KEY"),
        ("perplexity",  "https://api.perplexity.ai",                              "PERPLEXITY_API_KEY"),
        ("moonshot",    "https://api.moonshot.cn/v1",                             "MOONSHOT_API_KEY"),
        ("moonshotai",  "https://api.moonshot.cn/v1",                             "MOONSHOTAI_API_KEY"),
        ("kimi",        "https://api.moonshot.cn/v1",                             "KIMI_API_KEY"),
        ("huggingface", "https://api-inference.huggingface.co/v1",                "HUGGINGFACE_API_KEY"),
        ("arcee",       "https://api.arcee.ai/v1",                                "ARCEE_API_KEY"),
    ];
    candidates
        .iter()
        .filter(|(_, _, k)| std::env::var(k).map(|v| !v.is_empty()).unwrap_or(false))
        .map(|(p, u, k)| ProviderDef {
            name: p.to_string(),
            base_url: u.to_string(),
            key_env: k.to_string(),
        })
        .collect()
}

/// Return provider info by name, or None if unknown.
pub fn get_provider(name: &str) -> Option<ProviderDef> {
    let candidates = [
        ("anthropic",   "https://api.anthropic.com/v1",                           "ANTHROPIC_API_KEY"),
        ("openai",      "https://api.openai.com/v1",                              "OPENAI_API_KEY"),
        ("google",      "https://generativelanguage.googleapis.com/v1beta/openai","GOOGLE_API_KEY"),
        ("gemini",      "https://generativelanguage.googleapis.com/v1beta/openai","GEMINI_API_KEY"),
        ("deepseek",    "https://api.deepseek.com/v1",                            "DEEPSEEK_API_KEY"),
        ("groq",        "https://api.groq.com/openai/v1",                         "GROQ_API_KEY"),
        ("openrouter",  "https://openrouter.ai/api/v1",                           "OPENROUTER_API_KEY"),
        ("mistral",     "https://api.mistral.ai/v1",                              "MISTRAL_API_KEY"),
        ("xai",         "https://api.x.ai/v1",                                    "XAI_API_KEY"),
        ("grok",        "https://api.x.ai/v1",                                    "XAI_API_KEY"),
        ("cerebras",    "https://api.cerebras.ai/v1",                             "CEREBRAS_API_KEY"),
        ("nvidia",      "https://integrate.api.nvidia.com/v1",                    "NVIDIA_API_KEY"),
        ("perplexity",  "https://api.perplexity.ai",                              "PERPLEXITY_API_KEY"),
        ("moonshot",    "https://api.moonshot.cn/v1",                             "MOONSHOT_API_KEY"),
        ("moonshotai",  "https://api.moonshot.cn/v1",                             "MOONSHOTAI_API_KEY"),
        ("kimi",        "https://api.moonshot.cn/v1",                             "KIMI_API_KEY"),
        ("huggingface", "https://api-inference.huggingface.co/v1",                "HUGGINGFACE_API_KEY"),
        ("arcee",       "https://api.arcee.ai/v1",                                "ARCEE_API_KEY"),
        ("ollama",      "http://localhost:11434/v1",                               ""),
        ("lmstudio",    "http://localhost:1234/v1",                                ""),
    ];
    candidates.iter().find(|(n, _, _)| *n == name).map(|(n, u, k)| ProviderDef {
        name: n.to_string(),
        base_url: u.to_string(),
        key_env: k.to_string(),
    })
}

/// True if the key looks like a real secret (non-empty, not a placeholder).
pub fn is_real_key_pub(key: &str) -> bool {
    !key.is_empty()
        && key != "sk-placeholder"
        && key != "YOUR_API_KEY"
        && !key.starts_with("sk-test")
}

/// Return quota usage as (provider, used_tokens, remaining_tokens, confidence).
/// Currently returns best-effort estimates; full tracking via TokenLedgerManager.
pub fn all_provider_quotas() -> Vec<(String, u64, u64, f64)> {
    discover_providers()
        .into_iter()
        .map(|p| (p.name, 0u64, u64::MAX, 1.0f64))
        .collect()
}

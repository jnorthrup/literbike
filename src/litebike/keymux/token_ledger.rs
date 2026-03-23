//! Token Ledger Manager for Provider API Tracking
//!
//! Tracks token usage for specific providers (kilo code, opencode, openrouter, nvidia)
//! with vague quota estimation and API checks.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use chrono::{DateTime, Utc};

use super::dsel::{ProviderTokenLedger, ProviderApiStatus, VagueQuota, QuotaSource, KiloCodeConfig, OpenCodeConfig, OpenRouterConfig, NvidiaConfig, MoonshotConfig, GroqConfig, XAIConfig, CerebrasConfig};

/// Manager for tracking token usage across providers
pub struct TokenLedgerManager {
    ledgers: HashMap<String, ProviderTokenLedger>,
    provider_configs: ProviderConfigs,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfigs {
    pub kilo_code: Option<KiloCodeConfig>,
    pub opencode: Option<OpenCodeConfig>,
    pub openrouter: Option<OpenRouterConfig>,
    pub nvidia: Option<NvidiaConfig>,
    pub moonshot: Option<MoonshotConfig>,
    pub groq: Option<GroqConfig>,
    pub xai: Option<XAIConfig>,
    pub cerebras: Option<CerebrasConfig>,
}

impl TokenLedgerManager {
    pub fn new() -> Self {
        Self {
            ledgers: HashMap::new(),
            provider_configs: ProviderConfigs {
                kilo_code: None,
                opencode: None,
                openrouter: None,
                nvidia: None,
                moonshot: None,
                groq: None,
                xai: None,
                cerebras: None,
            },
        }
    }

    /// Initialize provider configurations
    pub fn initialize_providers(&mut self, api_keys: HashMap<String, String>) {
        // Kilo Code Configuration
        if let Some(api_key) = api_keys.get("kilo_code") {
            let config = KiloCodeConfig {
                api_key: Some(api_key.clone()),
                base_url: "https://api.kilocode.ai".to_string(),
                estimated_daily_limit: 1_000_000, // 1M tokens per day
                api_check_interval: 3600, // Check every hour
                last_api_check: 0,
                current_ledger: ProviderTokenLedger {
                    provider_name: "kilo_code".to_string(),
                    total_tokens_used: 0,
                    tokens_used_today: 0,
                    tokens_used_this_hour: 0,
                    last_api_check: None,
                    api_status: ProviderApiStatus::Unknown,
                    vague_quota_remaining: Some(VagueQuota {
                        estimated_remaining: 1_000_000,
                        confidence: 0.7,
                        last_updated: Self::current_timestamp(),
                        source: QuotaSource::ManualConfiguration,
                    }),
                },
            };
            self.provider_configs.kilo_code = Some(config);
        }

        // OpenCode Configuration
        if let Some(api_key) = api_keys.get("opencode") {
            let config = OpenCodeConfig {
                api_key: Some(api_key.clone()),
                base_url: "https://api.opencode.ai".to_string(),
                estimated_daily_limit: 500_000,
                api_check_interval: 3600,
                last_api_check: 0,
                current_ledger: ProviderTokenLedger {
                    provider_name: "opencode".to_string(),
                    total_tokens_used: 0,
                    tokens_used_today: 0,
                    tokens_used_this_hour: 0,
                    last_api_check: None,
                    api_status: ProviderApiStatus::Unknown,
                    vague_quota_remaining: Some(VagueQuota {
                        estimated_remaining: 500_000,
                        confidence: 0.6,
                        last_updated: Self::current_timestamp(),
                        source: QuotaSource::ManualConfiguration,
                    }),
                },
            };
            self.provider_configs.opencode = Some(config);
        }

        // OpenRouter Configuration
        if let Some(api_key) = api_keys.get("openrouter") {
            let config = OpenRouterConfig {
                api_key: Some(api_key.clone()),
                base_url: "https://openrouter.ai/api/v1".to_string(),
                estimated_daily_limit: 2_000_000, // OpenRouter typically has higher limits
                api_check_interval: 1800, // Check every 30 minutes
                last_api_check: 0,
                current_ledger: ProviderTokenLedger {
                    provider_name: "openrouter".to_string(),
                    total_tokens_used: 0,
                    tokens_used_today: 0,
                    tokens_used_this_hour: 0,
                    last_api_check: None,
                    api_status: ProviderApiStatus::Unknown,
                    vague_quota_remaining: Some(VagueQuota {
                        estimated_remaining: 2_000_000,
                        confidence: 0.8,
                        last_updated: Self::current_timestamp(),
                        source: QuotaSource::ManualConfiguration,
                    }),
                },
            };
            self.provider_configs.openrouter = Some(config);
        }

        // NVIDIA Configuration
        if let Some(api_key) = api_keys.get("nvidia") {
            let config = NvidiaConfig {
                api_key: Some(api_key.clone()),
                base_url: "https://api.nvidia.com/v1".to_string(),
                estimated_daily_limit: 3_000_000, // NVIDIA typically has generous quotas
                api_check_interval: 7200, // Check every 2 hours
                last_api_check: 0,
                current_ledger: ProviderTokenLedger {
                    provider_name: "nvidia".to_string(),
                    total_tokens_used: 0,
                    tokens_used_today: 0,
                    tokens_used_this_hour: 0,
                    last_api_check: None,
                    api_status: ProviderApiStatus::Unknown,
                    vague_quota_remaining: Some(VagueQuota {
                        estimated_remaining: 3_000_000,
                        confidence: 0.9,
                        last_updated: Self::current_timestamp(),
                        source: QuotaSource::ManualConfiguration,
                    }),
                },
            };
            self.provider_configs.nvidia = Some(config);
        }

        // Moonshot (Kimi) Configuration
        if let Some(api_key) = api_keys.get("moonshot") {
            let config = MoonshotConfig {
                api_key: Some(api_key.clone()),
                base_url: "https://api.moonshot.cn/v1".to_string(),
                estimated_daily_limit: 1_500_000, // Moonshot/Kimi typical quota
                api_check_interval: 3600, // Check every hour
                last_api_check: 0,
                current_ledger: ProviderTokenLedger {
                    provider_name: "moonshot".to_string(),
                    total_tokens_used: 0,
                    tokens_used_today: 0,
                    tokens_used_this_hour: 0,
                    last_api_check: None,
                    api_status: ProviderApiStatus::Unknown,
                    vague_quota_remaining: Some(VagueQuota {
                        estimated_remaining: 1_500_000,
                        confidence: 0.7,
                        last_updated: Self::current_timestamp(),
                        source: QuotaSource::ManualConfiguration,
                    }),
                },
            };
            self.provider_configs.moonshot = Some(config);
        }

        // Groq Configuration
        if let Some(api_key) = api_keys.get("groq") {
            let config = GroqConfig {
                api_key: Some(api_key.clone()),
                base_url: "https://api.groq.com/openai/v1".to_string(),
                estimated_daily_limit: 2_000_000, // Groq typical quota
                api_check_interval: 3600, // Check every hour
                last_api_check: 0,
                current_ledger: ProviderTokenLedger {
                    provider_name: "groq".to_string(),
                    total_tokens_used: 0,
                    tokens_used_today: 0,
                    tokens_used_this_hour: 0,
                    last_api_check: None,
                    api_status: ProviderApiStatus::Unknown,
                    vague_quota_remaining: Some(VagueQuota {
                        estimated_remaining: 2_000_000,
                        confidence: 0.8,
                        last_updated: Self::current_timestamp(),
                        source: QuotaSource::ManualConfiguration,
                    }),
                },
            };
            self.provider_configs.groq = Some(config);
        }

        // xAI (Grok) Configuration
        if let Some(api_key) = api_keys.get("xai") {
            let config = XAIConfig {
                api_key: Some(api_key.clone()),
                base_url: "https://api.x.ai/v1".to_string(),
                estimated_daily_limit: 1_500_000, // xAI/Grok typical quota
                api_check_interval: 3600, // Check every hour
                last_api_check: 0,
                current_ledger: ProviderTokenLedger {
                    provider_name: "xai".to_string(),
                    total_tokens_used: 0,
                    tokens_used_today: 0,
                    tokens_used_this_hour: 0,
                    last_api_check: None,
                    api_status: ProviderApiStatus::Unknown,
                    vague_quota_remaining: Some(VagueQuota {
                        estimated_remaining: 1_500_000,
                        confidence: 0.7,
                        last_updated: Self::current_timestamp(),
                        source: QuotaSource::ManualConfiguration,
                    }),
                },
            };
            self.provider_configs.xai = Some(config);
        }

        // Cerebras Configuration
        if let Some(api_key) = api_keys.get("cerebras") {
            let config = CerebrasConfig {
                api_key: Some(api_key.clone()),
                base_url: "https://api.cerebras.ai/v1".to_string(),
                estimated_daily_limit: 2_000_000, // Cerebras typical quota
                api_check_interval: 3600, // Check every hour
                last_api_check: 0,
                current_ledger: ProviderTokenLedger {
                    provider_name: "cerebras".to_string(),
                    total_tokens_used: 0,
                    tokens_used_today: 0,
                    tokens_used_this_hour: 0,
                    last_api_check: None,
                    api_status: ProviderApiStatus::Unknown,
                    vague_quota_remaining: Some(VagueQuota {
                        estimated_remaining: 2_000_000,
                        confidence: 0.8,
                        last_updated: Self::current_timestamp(),
                        source: QuotaSource::ManualConfiguration,
                    }),
                },
            };
            self.provider_configs.cerebras = Some(config);
        }
    }

    /// Track token usage for a provider
    pub fn track_usage(&mut self, provider: &str, tokens: u64) -> Result<(), String> {
        let ledger = self.ledgers.entry(provider.to_string()).or_insert_with(|| {
            ProviderTokenLedger {
                provider_name: provider.to_string(),
                total_tokens_used: 0,
                tokens_used_today: 0,
                tokens_used_this_hour: 0,
                last_api_check: None,
                api_status: ProviderApiStatus::Unknown,
                vague_quota_remaining: None,
            }
        });

        ledger.total_tokens_used += tokens;
        ledger.tokens_used_today += tokens;
        ledger.tokens_used_this_hour += tokens;

        // Update vague quota estimation
        if let Some(ref mut vague_quota) = ledger.vague_quota_remaining {
            if tokens <= vague_quota.estimated_remaining {
                vague_quota.estimated_remaining -= tokens;
                vague_quota.confidence = vague_quota.confidence * 0.99; // Slight decrease in confidence
                vague_quota.last_updated = Self::current_timestamp();
            } else {
                // Quota exhausted
                vague_quota.estimated_remaining = 0;
                vague_quota.confidence = 0.0;
                vague_quota.last_updated = Self::current_timestamp();
            }
        }

        Ok(())
    }

    /// Perform API check for provider quota
    pub async fn perform_api_check(&mut self, provider: &str) -> Result<ProviderApiStatus, String> {
        let current_time = Self::current_timestamp();
        
        match provider {
            "kilo_code" => {
                if let Some(config) = &self.provider_configs.kilo_code {
                    if let Some(api_key) = &config.api_key {
                        // Simulate API check (in real implementation, make HTTP request)
                        let status = self.check_kilo_code_api(api_key).await?;
                        
                        // Update ledger
                        if let Some(ledger) = self.ledgers.get_mut(provider) {
                            ledger.last_api_check = Some(current_time);
                            ledger.api_status = status.clone();
                            
                            // Update vague quota from API response
                            let estimated_quota = self.estimate_quota_from_api(provider, &status);
                            ledger.vague_quota_remaining = Some(VagueQuota {
                                estimated_remaining: estimated_quota,
                                confidence: 0.8,
                                last_updated: current_time,
                                source: QuotaSource::ApiDirect,
                            });
                        }
                        
                        return Ok(status);
                    }
                }
            }
            "opencode" => {
                if let Some(config) = &self.provider_configs.opencode {
                    if let Some(api_key) = &config.api_key {
                        let status = self.check_opencode_api(api_key).await?;
                        // Similar update logic
                    }
                }
            }
            "openrouter" => {
                if let Some(config) = &self.provider_configs.openrouter {
                    if let Some(api_key) = &config.api_key {
                        let status = self.check_openrouter_api(api_key).await?;
                        // Similar update logic
                    }
                }
            }
            "nvidia" => {
                if let Some(config) = &self.provider_configs.nvidia {
                    if let Some(api_key) = &config.api_key {
                        let status = self.check_nvidia_api(api_key).await?;
                        // Similar update logic
                    }
                }
            }
            "moonshot" => {
                if let Some(config) = &self.provider_configs.moonshot {
                    if let Some(api_key) = &config.api_key {
                        let status = self.check_moonshot_api(api_key).await?;
                        
                        // Update ledger
                        if let Some(ledger) = self.ledgers.get_mut(provider) {
                            ledger.last_api_check = Some(current_time);
                            ledger.api_status = status.clone();
                            
                            // Update vague quota from API response
                            let estimated_quota = self.estimate_quota_from_api(provider, &status);
                            ledger.vague_quota_remaining = Some(VagueQuota {
                                estimated_remaining: estimated_quota,
                                confidence: 0.8,
                                last_updated: current_time,
                                source: QuotaSource::ApiDirect,
                            });
                        }
                        
                        return Ok(status);
                    }
                }
            }
            "groq" => {
                if let Some(config) = &self.provider_configs.groq {
                    if let Some(api_key) = &config.api_key {
                        let status = self.check_groq_api(api_key).await?;
                        
                        // Update ledger
                        if let Some(ledger) = self.ledgers.get_mut(provider) {
                            ledger.last_api_check = Some(current_time);
                            ledger.api_status = status.clone();
                            
                            // Update vague quota from API response
                            let estimated_quota = self.estimate_quota_from_api(provider, &status);
                            ledger.vague_quota_remaining = Some(VagueQuota {
                                estimated_remaining: estimated_quota,
                                confidence: 0.8,
                                last_updated: current_time,
                                source: QuotaSource::ApiDirect,
                            });
                        }
                        
                        return Ok(status);
                    }
                }
            }
            "xai" => {
                if let Some(config) = &self.provider_configs.xai {
                    if let Some(api_key) = &config.api_key {
                        let status = self.check_xai_api(api_key).await?;
                        
                        // Update ledger
                        if let Some(ledger) = self.ledgers.get_mut(provider) {
                            ledger.last_api_check = Some(current_time);
                            ledger.api_status = status.clone();
                            
                            // Update vague quota from API response
                            let estimated_quota = self.estimate_quota_from_api(provider, &status);
                            ledger.vague_quota_remaining = Some(VagueQuota {
                                estimated_remaining: estimated_quota,
                                confidence: 0.8,
                                last_updated: current_time,
                                source: QuotaSource::ApiDirect,
                            });
                        }
                        
                        return Ok(status);
                    }
                }
            }
            "cerebras" => {
                if let Some(config) = &self.provider_configs.cerebras {
                    if let Some(api_key) = &config.api_key {
                        let status = self.check_cerebras_api(api_key).await?;
                        
                        // Update ledger
                        if let Some(ledger) = self.ledgers.get_mut(provider) {
                            ledger.last_api_check = Some(current_time);
                            ledger.api_status = status.clone();
                            
                            // Update vague quota from API response
                            let estimated_quota = self.estimate_quota_from_api(provider, &status);
                            ledger.vague_quota_remaining = Some(VagueQuota {
                                estimated_remaining: estimated_quota,
                                confidence: 0.8,
                                last_updated: current_time,
                                source: QuotaSource::ApiDirect,
                            });
                        }
                        
                        return Ok(status);
                    }
                }
            }
            _ => {
                return Err(format!("Unknown provider: {}", provider));
            }
        }

        Ok(ProviderApiStatus::Unknown)
    }

    /// Check Kilo Code API
    async fn check_kilo_code_api(&self, api_key: &str) -> Result<ProviderApiStatus, String> {
        // In real implementation, make HTTP request to Kilo Code API
        // For now, simulate based on usage patterns
        if let Some(ledger) = self.ledgers.get("kilo_code") {
            if ledger.tokens_used_today > 1_000_000 {
                return Ok(ProviderApiStatus::RateLimited);
            }
        }
        Ok(ProviderApiStatus::Healthy)
    }

    /// Check OpenCode API
    async fn check_opencode_api(&self, api_key: &str) -> Result<ProviderApiStatus, String> {
        // Simulate OpenCode API check
        if let Some(ledger) = self.ledgers.get("opencode") {
            if ledger.tokens_used_today > 500_000 {
                return Ok(ProviderApiStatus::RateLimited);
            }
        }
        Ok(ProviderApiStatus::Healthy)
    }

    /// Check OpenRouter API
    async fn check_openrouter_api(&self, api_key: &str) -> Result<ProviderApiStatus, String> {
        // Simulate OpenRouter API check
        if let Some(ledger) = self.ledgers.get("openrouter") {
            if ledger.tokens_used_today > 2_000_000 {
                return Ok(ProviderApiStatus::RateLimited);
            }
        }
        Ok(ProviderApiStatus::Healthy)
    }

    /// Check NVIDIA API
    async fn check_nvidia_api(&self, api_key: &str) -> Result<ProviderApiStatus, String> {
        // Simulate NVIDIA API check
        if let Some(ledger) = self.ledgers.get("nvidia") {
            if ledger.tokens_used_today > 3_000_000 {
                return Ok(ProviderApiStatus::RateLimited);
            }
        }
        Ok(ProviderApiStatus::Healthy)
    }

    /// Check Moonshot API
    async fn check_moonshot_api(&self, api_key: &str) -> Result<ProviderApiStatus, String> {
        // Simulate Moonshot API check
        if let Some(ledger) = self.ledgers.get("moonshot") {
            if ledger.tokens_used_today > 1_500_000 {
                return Ok(ProviderApiStatus::RateLimited);
            }
        }
        Ok(ProviderApiStatus::Healthy)
    }

    /// Check Groq API
    async fn check_groq_api(&self, api_key: &str) -> Result<ProviderApiStatus, String> {
        // Simulate Groq API check
        if let Some(ledger) = self.ledgers.get("groq") {
            if ledger.tokens_used_today > 2_000_000 {
                return Ok(ProviderApiStatus::RateLimited);
            }
        }
        Ok(ProviderApiStatus::Healthy)
    }

    /// Check xAI (Grok) API
    async fn check_xai_api(&self, api_key: &str) -> Result<ProviderApiStatus, String> {
        // Simulate xAI API check
        if let Some(ledger) = self.ledgers.get("xai") {
            if ledger.tokens_used_today > 1_500_000 {
                return Ok(ProviderApiStatus::RateLimited);
            }
        }
        Ok(ProviderApiStatus::Healthy)
    }

    /// Check Cerebras API
    async fn check_cerebras_api(&self, api_key: &str) -> Result<ProviderApiStatus, String> {
        // Simulate Cerebras API check
        if let Some(ledger) = self.ledgers.get("cerebras") {
            if ledger.tokens_used_today > 2_000_000 {
                return Ok(ProviderApiStatus::RateLimited);
            }
        }
        Ok(ProviderApiStatus::Healthy)
    }

    /// Estimate quota from API response
    fn estimate_quota_from_api(&self, provider: &str, status: &ProviderApiStatus) -> u64 {
        match status {
            ProviderApiStatus::Healthy => {
                // Estimate remaining quota based on provider limits
                match provider {
                    "kilo_code" => 1_000_000,
                    "opencode" => 500_000,
                    "openrouter" => 2_000_000,
                    "nvidia" => 3_000_000,
                    "moonshot" => 1_500_000,
                    "groq" => 2_000_000,
                    "xai" => 1_500_000,
                    "cerebras" => 2_000_000,
                    _ => 0,
                }
            }
            ProviderApiStatus::Degraded => {
                // Reduced quota estimation
                match provider {
                    "kilo_code" => 500_000,
                    "opencode" => 250_000,
                    "openrouter" => 1_000_000,
                    "nvidia" => 1_500_000,
                    "moonshot" => 750_000,
                    "groq" => 1_000_000,
                    "xai" => 750_000,
                    "cerebras" => 1_000_000,
                    _ => 0,
                }
            }
            ProviderApiStatus::RateLimited => 0,
            ProviderApiStatus::AuthenticationFailed => 0,
            ProviderApiStatus::Unknown => 0,
        }
    }

    /// Get current token ledger for provider
    pub fn get_ledger(&self, provider: &str) -> Option<&ProviderTokenLedger> {
        self.ledgers.get(provider)
    }

    /// Get all ledgers
    pub fn get_all_ledgers(&self) -> &HashMap<String, ProviderTokenLedger> {
        &self.ledgers
    }

    /// Check if provider has sufficient quota
    pub fn has_sufficient_quota(&self, provider: &str, tokens_needed: u64) -> bool {
        if let Some(ledger) = self.ledgers.get(provider) {
            if let Some(vague_quota) = &ledger.vague_quota_remaining {
                return vague_quota.estimated_remaining >= tokens_needed;
            }
        }
        false
    }

    /// Reset hourly usage (call this hourly)
    pub fn reset_hourly_usage(&mut self) {
        for ledger in self.ledgers.values_mut() {
            ledger.tokens_used_this_hour = 0;
        }
    }

    /// Reset daily usage (call this daily)
    pub fn reset_daily_usage(&mut self) {
        for ledger in self.ledgers.values_mut() {
            ledger.tokens_used_today = 0;
        }
    }

    /// Get provider status summary
    pub fn get_status_summary(&self) -> HashMap<String, (ProviderApiStatus, u64, u64)> {
        let mut summary = HashMap::new();
        
        for (provider, ledger) in &self.ledgers {
            let status = ledger.api_status.clone();
            let used = ledger.tokens_used_today;
            let remaining = ledger.vague_quota_remaining.as_ref()
                .map(|q| q.estimated_remaining)
                .unwrap_or(0);
            
            summary.insert(provider.clone(), (status, used, remaining));
        }
        
        summary
    }

    fn current_timestamp() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
    }
}
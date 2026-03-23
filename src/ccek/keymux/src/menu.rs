//! ModelMux DSEL Menu - High-level provider selection and routing interface
//!
//! This module provides the working model/key mux menu for CCEK.
//! Use this to discover providers, route models, and manage quota.

use crate::dsel::{all_provider_quotas, discover_providers, route, track_tokens, ProviderDef};
use std::collections::HashMap;
use std::env;

/// Current state of the model mux menu
#[derive(Debug, Clone)]
pub struct MuxMenu {
    /// Currently active providers (have API keys set)
    pub active_providers: Vec<ProviderDef>,
    /// Selected provider for routing
    pub selected_provider: Option<String>,
    /// Provider quotas
    pub quotas: HashMap<String, ProviderQuota>,
    /// Last error if discovery failed
    pub last_error: Option<String>,
}

/// Quota information for a provider
#[derive(Debug, Clone)]
pub struct ProviderQuota {
    pub provider: String,
    pub used_tokens: u64,
    pub remaining_tokens: u64,
    pub confidence: f64,
}

impl MuxMenu {
    /// Create a new mux menu and discover available providers
    pub fn new() -> Self {
        let active_providers = discover_providers();
        let quotas = all_provider_quotas()
            .into_iter()
            .map(|(name, used, remaining, confidence)| {
                (
                    name.clone(),
                    ProviderQuota {
                        provider: name,
                        used_tokens: used,
                        remaining_tokens: remaining,
                        confidence,
                    },
                )
            })
            .collect();

        Self {
            active_providers,
            selected_provider: None,
            quotas,
            last_error: None,
        }
    }

    /// Refresh provider discovery from environment
    pub fn refresh(&mut self) {
        self.active_providers = discover_providers();
        self.quotas = all_provider_quotas()
            .into_iter()
            .map(|(name, used, remaining, confidence)| {
                (
                    name.clone(),
                    ProviderQuota {
                        provider: name,
                        used_tokens: used,
                        remaining_tokens: remaining,
                        confidence,
                    },
                )
            })
            .collect();
    }

    /// Get list of active provider names
    pub fn provider_names(&self) -> Vec<String> {
        self.active_providers
            .iter()
            .map(|p| p.name.clone())
            .collect()
    }

    /// Select a provider by name
    pub fn select_provider(&mut self, name: &str) -> bool {
        if self.active_providers.iter().any(|p| p.name == name) {
            self.selected_provider = Some(name.to_string());
            true
        } else {
            false
        }
    }

    /// Get routing info for a model reference
    /// Format: "provider/model-name" or just "model-name" (uses selected provider)
    pub fn route(&self, model_ref: &str) -> Option<(String, String, String)> {
        // If model_ref contains a slash, use the provider prefix
        if model_ref.contains('/') {
            return route(model_ref);
        }

        // Otherwise, prepend selected provider if set
        if let Some(ref provider) = self.selected_provider {
            let full_ref = format!("{}/{}", provider, model_ref);
            route(&full_ref)
        } else if let Some(first) = self.active_providers.first() {
            let full_ref = format!("{}/{}", first.name, model_ref);
            route(&full_ref)
        } else {
            None
        }
    }

    /// Get base URL for a provider
    pub fn get_base_url(&self, provider: &str) -> Option<String> {
        self.active_providers
            .iter()
            .find(|p| p.name == provider)
            .map(|p| p.base_url.clone())
    }

    /// Get API key env var for a provider
    pub fn get_key_env(&self, provider: &str) -> Option<String> {
        self.active_providers
            .iter()
            .find(|p| p.name == provider)
            .map(|p| p.key_env.clone())
    }

    /// Get the actual API key for a provider
    pub fn get_api_key(&self, provider: &str) -> Option<String> {
        self.get_key_env(provider)
            .and_then(|env_var| env::var(&env_var).ok())
            .filter(|key| !key.is_empty())
    }

    /// Check if a provider is available
    pub fn has_provider(&self, name: &str) -> bool {
        self.active_providers.iter().any(|p| p.name == name)
    }

    /// Get quota for a provider
    pub fn get_quota(&self, provider: &str) -> Option<&ProviderQuota> {
        self.quotas.get(provider)
    }

    /// Track token usage for a provider
    pub fn track_usage(&mut self, provider: &str, tokens: u64) -> Result<(), String> {
        track_tokens(provider, tokens)?;

        // Update local quota tracking
        if let Some(quota) = self.quotas.get_mut(provider) {
            quota.used_tokens += tokens;
            if quota.remaining_tokens >= tokens {
                quota.remaining_tokens -= tokens;
            } else {
                quota.remaining_tokens = 0;
            }
        }

        Ok(())
    }

    /// Display menu of available providers
    pub fn display_menu(&self) {
        println!("\n=== ModelMux DSEL Menu ===");
        println!("Active Providers (from env):");

        if self.active_providers.is_empty() {
            println!("  (none - set API key env vars to activate providers)");
        } else {
            for provider in &self.active_providers {
                let key_status = if self.get_api_key(&provider.name).is_some() {
                    "[key set]"
                } else {
                    "[no key]"
                };
                let selected = if self.selected_provider.as_ref() == Some(&provider.name) {
                    " <- SELECTED"
                } else {
                    ""
                };
                println!(
                    "  - {} {} {}{}",
                    provider.name, provider.base_url, key_status, selected
                );
            }
        }

        println!("\nAvailable Providers (all):");
        let all = [
            "anthropic",
            "openai",
            "google",
            "gemini",
            "deepseek",
            "groq",
            "openrouter",
            "mistral",
            "xai",
            "grok",
            "cerebras",
            "nvidia",
            "perplexity",
            "moonshot",
            "kimi",
            "kilo",
            "zai",
            "huggingface",
            "arcee",
        ];
        for name in all {
            let status = if self.has_provider(name) { "*" } else { " " };
            println!("  [{}] {}", status, name);
        }
        println!("\n* = has API key in environment");
    }

    /// Export provider configuration as environment variables
    pub fn export_env(&self) -> Vec<(String, String)> {
        let mut exports = Vec::new();

        for provider in &self.active_providers {
            if let Ok(key) = env::var(&provider.key_env) {
                exports.push((provider.key_env.clone(), key));
            }
        }

        exports
    }
}

impl Default for MuxMenu {
    fn default() -> Self {
        Self::new()
    }
}

/// Quick route function for direct model routing without menu state
pub fn quick_route(model_ref: &str) -> Option<(String, String, String)> {
    route(model_ref)
}

/// Check which providers are available from environment
pub fn check_providers() -> Vec<String> {
    discover_providers().into_iter().map(|p| p.name).collect()
}

/// Get full provider configuration for all discovered providers
pub fn get_provider_configs() -> HashMap<String, ProviderDef> {
    discover_providers()
        .into_iter()
        .map(|p| (p.name.clone(), p))
        .collect()
}

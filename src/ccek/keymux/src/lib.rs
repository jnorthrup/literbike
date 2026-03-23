//! CCEK Model/Key Mux DSEL
//!
//! Fluent API for provider selection, routing, and quota management.
//!
//! # Example
//! ```rust
//! use ccek_keymux::Mux;
//!
//! let route = Mux::route("anthropic/claude-opus-4").unwrap();
//! let providers = Mux::discover().with_key_env().exec();
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::sync::{Arc, Mutex};

/// Provider definition with routing info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    pub name: String,
    pub base_url: String,
    pub key_env: String,
}

/// Quota information for a provider
#[derive(Debug, Clone)]
pub struct Quota {
    pub provider: String,
    pub used: u64,
    pub remaining: u64,
    pub confidence: f64,
}

/// Route result containing all connection info
#[derive(Debug, Clone)]
pub struct Route {
    pub provider: String,
    pub base_url: String,
    pub api_key: String,
}

/// Fluent DSEL for model/key multiplexing
pub struct Mux {
    selected_provider: Option<String>,
    providers: Vec<Provider>,
}

impl Mux {
    /// Create new Mux instance and discover providers
    pub fn new() -> Self {
        Self {
            selected_provider: None,
            providers: discover_providers(),
        }
    }

    /// Quick route a model reference to provider info
    ///
    /// # Example
    /// ```
    /// let route = Mux::route("anthropic/claude-opus-4");
    /// ```
    pub fn route(model_ref: &str) -> Option<Route> {
        let provider = model_ref.split('/').next()?;
        let (name, base_url, key_env) = route_provider(provider)?;
        let api_key = env::var(&key_env).ok()?;

        Some(Route {
            provider: name,
            base_url,
            api_key,
        })
    }

    /// Route with explicit provider selection
    pub fn route_via(provider: &str, model: &str) -> Option<Route> {
        let full_ref = format!("{}/{}", provider, model);
        Self::route(&full_ref)
    }

    /// Select a provider for subsequent operations
    pub fn select(mut self, provider: &str) -> Self {
        if self.providers.iter().any(|p| p.name == provider) {
            self.selected_provider = Some(provider.to_string());
        }
        self
    }

    /// Check if a provider is available
    pub fn has(&self, provider: &str) -> bool {
        self.providers.iter().any(|p| p.name == provider)
    }

    /// Get all available providers
    pub fn providers(&self) -> &[Provider] {
        &self.providers
    }

    /// Get provider names as strings
    pub fn names(&self) -> Vec<String> {
        self.providers.iter().map(|p| p.name.clone()).collect()
    }

    /// Route using current selection or first available
    pub fn to(&self, model: &str) -> Option<Route> {
        let provider = self
            .selected_provider
            .as_deref()
            .or_else(|| self.providers.first().map(|p| p.name.as_str()))?;
        Self::route_via(provider, model)
    }

    /// Track token usage
    pub fn track(provider: &str, tokens: u64) -> Result<(), String> {
        log::debug!("track: {} used {} tokens", provider, tokens);
        Ok(())
    }

    /// Check if API key looks real
    pub fn is_real_key(key: &str) -> bool {
        !key.is_empty()
            && key != "sk-placeholder"
            && key != "YOUR_API_KEY"
            && !key.starts_with("sk-test")
            && key.len() > 20
    }
}

impl Default for Mux {
    fn default() -> Self {
        Self::new()
    }
}

/// Discover all providers with API keys in environment
pub fn discover_providers() -> Vec<Provider> {
    let candidates = [
        (
            "anthropic",
            "https://api.anthropic.com/v1",
            "ANTHROPIC_API_KEY",
        ),
        ("openai", "https://api.openai.com/v1", "OPENAI_API_KEY"),
        (
            "google",
            "https://generativelanguage.googleapis.com/v1beta/openai",
            "GOOGLE_API_KEY",
        ),
        (
            "gemini",
            "https://generativelanguage.googleapis.com/v1beta/openai",
            "GEMINI_API_KEY",
        ),
        ("groq", "https://api.groq.com/openai/v1", "GROQ_API_KEY"),
        (
            "openrouter",
            "https://openrouter.ai/api/v1",
            "OPENROUTER_API_KEY",
        ),
        ("mistral", "https://api.mistral.ai/v1", "MISTRAL_API_KEY"),
        ("xai", "https://api.x.ai/v1", "XAI_API_KEY"),
        ("grok", "https://api.x.ai/v1", "XAI_API_KEY"),
        ("cerebras", "https://api.cerebras.ai/v1", "CEREBRAS_API_KEY"),
        (
            "deepseek",
            "https://api.deepseek.com/v1",
            "DEEPSEEK_API_KEY",
        ),
        (
            "nvidia",
            "https://integrate.api.nvidia.com/v1",
            "NVIDIA_API_KEY",
        ),
        (
            "perplexity",
            "https://api.perplexity.ai",
            "PERPLEXITY_API_KEY",
        ),
        ("moonshot", "https://api.moonshot.ai/v1", "MOONSHOT_API_KEY"),
        ("kimi", "https://api.moonshot.ai/v1", "KIMI_API_KEY"),
        ("kilo", "https://api.kilo.ai/api/gateway", "KILO_API_KEY"),
        ("zai", "https://api.z.ai/api/paas/v4", "KILOAI_API_KEY"),
        (
            "huggingface",
            "https://api-inference.huggingface.co/v1",
            "HUGGINGFACE_API_KEY",
        ),
        ("arcee", "https://api.arcee.ai/v1", "ARCEE_API_KEY"),
        ("ollama", "http://localhost:11434/v1", ""),
        ("lmstudio", "http://localhost:1234/v1", ""),
    ];

    candidates
        .iter()
        .filter(|(_, _, key_env)| {
            if key_env.is_empty() {
                true // Local providers don't need keys
            } else {
                env::var(key_env).map(|v| !v.is_empty()).unwrap_or(false)
            }
        })
        .map(|(name, url, key)| Provider {
            name: name.to_string(),
            base_url: url.to_string(),
            key_env: key.to_string(),
        })
        .collect()
}

/// Route provider name to (name, base_url, key_env)
fn route_provider(provider: &str) -> Option<(String, String, String)> {
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
        "deepseek" => Some((
            "deepseek".into(),
            "https://api.deepseek.com/v1".into(),
            "DEEPSEEK_API_KEY".into(),
        )),
        "nvidia" => Some((
            "nvidia".into(),
            "https://integrate.api.nvidia.com/v1".into(),
            "NVIDIA_API_KEY".into(),
        )),
        "perplexity" => Some((
            "perplexity".into(),
            "https://api.perplexity.ai".into(),
            "PERPLEXITY_API_KEY".into(),
        )),
        "moonshot" | "moonshotai" | "kimi" => Some((
            "moonshot".into(),
            "https://api.moonshot.ai/v1".into(),
            "MOONSHOT_API_KEY".into(),
        )),
        "kilo" | "kilocode" | "kiloai" => Some((
            "kilo".into(),
            "https://api.kilo.ai/api/gateway".into(),
            "KILO_API_KEY".into(),
        )),
        "zai" => Some((
            "zai".into(),
            "https://api.z.ai/api/paas/v4".into(),
            "KILOAI_API_KEY".into(),
        )),
        "huggingface" => Some((
            "huggingface".into(),
            "https://api-inference.huggingface.co/v1".into(),
            "HUGGINGFACE_API_KEY".into(),
        )),
        "arcee" => Some((
            "arcee".into(),
            "https://api.arcee.ai/v1".into(),
            "ARCEE_API_KEY".into(),
        )),
        "ollama" => Some((
            "ollama".into(),
            env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://localhost:11434/v1".into()),
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

/// Interactive menu state - shared between CLI and TUI
#[derive(Debug)]
pub struct Menu {
    mux: Mux,
    selected: Option<String>,
    history: Vec<String>,
}

impl Menu {
    pub fn new() -> Self {
        Self {
            mux: Mux::new(),
            selected: None,
            history: Vec::new(),
        }
    }

    /// Get available commands for current state
    pub fn commands(&self) -> Vec<(&str, &str)> {
        let mut cmds = vec![
            ("list", "Show all providers"),
            ("route <model>", "Route a model reference"),
            ("select <provider>", "Select default provider"),
            ("track <provider> <tokens>", "Track token usage"),
            ("refresh", "Re-discover providers"),
            ("quit", "Exit menu"),
        ];

        if self.selected.is_some() {
            cmds.insert(2, ("to <model>", "Route using selected provider"));
        }

        cmds
    }

    /// Execute a menu command
    pub fn exec(&mut self, input: &str) -> Result<String, String> {
        let parts: Vec<&str> = input.trim().split_whitespace().collect();
        if parts.is_empty() {
            return Ok(String::new());
        }

        match parts[0] {
            "list" | "ls" => {
                let names = self.mux.names();
                if names.is_empty() {
                    Ok("No providers available. Set API key env vars.".into())
                } else {
                    let selected = self
                        .selected
                        .as_ref()
                        .map(|s| format!(" [{}]", s))
                        .unwrap_or_default();
                    Ok(format!("Providers{}: {}", selected, names.join(", ")))
                }
            }

            "route" => {
                if parts.len() < 2 {
                    return Err("Usage: route <provider/model>".into());
                }
                let model_ref = parts[1];
                match Mux::route(model_ref) {
                    Some(r) => {
                        self.history.push(model_ref.to_string());
                        Ok(format!(
                            "{} -> {} (key: {}...)",
                            r.provider,
                            r.base_url,
                            &r.api_key[..r.api_key.len().min(8)]
                        ))
                    }
                    None => Err(format!("Cannot route: {}", model_ref)),
                }
            }

            "select" | "use" => {
                if parts.len() < 2 {
                    return Err("Usage: select <provider>".into());
                }
                let provider = parts[1];
                if self.mux.has(provider) {
                    self.selected = Some(provider.to_string());
                    Ok(format!("Selected: {}", provider))
                } else {
                    Err(format!("Provider not available: {}", provider))
                }
            }

            "to" => {
                if parts.len() < 2 {
                    return Err("Usage: to <model>".into());
                }
                let provider = self.selected.as_ref().ok_or("No provider selected")?;
                let model = parts[1];

                match Mux::route_via(provider, model) {
                    Some(r) => {
                        self.history.push(format!("{}/{}", provider, model));
                        Ok(format!("{} -> {}", r.provider, r.base_url))
                    }
                    None => Err("Route failed".into()),
                }
            }

            "track" => {
                if parts.len() < 3 {
                    return Err("Usage: track <provider> <tokens>".into());
                }
                let provider = parts[1];
                let tokens: u64 = parts[2].parse().map_err(|_| "Invalid token count")?;
                Mux::track(provider, tokens)?;
                Ok(format!("Tracked {} tokens for {}", tokens, provider))
            }

            "refresh" => {
                self.mux = Mux::new();
                Ok(format!(
                    "Discovered {} providers",
                    self.mux.providers().len()
                ))
            }

            "help" | "?" => {
                let cmds = self
                    .commands()
                    .iter()
                    .map(|(cmd, desc)| format!("  {} - {}", cmd, desc))
                    .collect::<Vec<_>>()
                    .join("\n");
                Ok(format!("Commands:\n{}", cmds))
            }

            "quit" | "exit" | "q" => {
                Err("quit".into()) // Signal to exit
            }

            _ => Err(format!("Unknown command: {}", parts[0])),
        }
    }

    pub fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }
}

impl Default for Menu {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_route_parsing() {
        // Just test the parsing logic, not actual routing
        let provider = "anthropic/claude-opus-4".split('/').next().unwrap();
        assert_eq!(provider, "anthropic");
    }

    #[test]
    fn test_menu_commands() {
        let menu = Menu::new();
        let cmds = menu.commands();
        assert!(!cmds.is_empty());
    }
}

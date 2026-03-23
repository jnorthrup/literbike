//! Unified API Client for N-Way Provider Access

use crate::types::*;
use crate::converter::ApiConverter;

#[derive(Debug)]
pub struct UnifiedClient {
    provider: Provider,
    api_key: String,
    client: reqwest::Client,
}

impl UnifiedClient {
    pub fn new(provider: Provider, api_key: String) -> Self {
        Self { provider, api_key, client: reqwest::Client::new() }
    }
    
    pub fn from_env(provider: Provider) -> Option<Self> {
        let key_var = match provider {
            Provider::OpenAI => "OPENAI_API_KEY",
            Provider::Anthropic => "ANTHROPIC_AUTH_TOKEN",
            Provider::Gemini => "GEMINI_API_KEY",
            Provider::DeepSeek => "DEEPSEEK_API_KEY",
            Provider::Moonshot => "MOONSHOT_API_KEY",
            Provider::Groq => "GROQ_API_KEY",
            Provider::XAI => "XAI_API_KEY",
            Provider::Cohere => "COHERE_API_KEY",
            Provider::Mistral => "MISTRAL_API_KEY",
            Provider::Perplexity => "PERPLEXITY_API_KEY",
            Provider::OpenRouter => "OPENROUTER_API_KEY",
            Provider::NVIDIA => "NVIDIA_API_KEY",
            Provider::Cerebras => "CEREBRAS_API_KEY",
            Provider::BraveSearch => "BRAVE_SEARCH_API_KEY",
            Provider::TavilySearch => "TAVILY_SEARCH_API_KEY",
            Provider::SerperSearch => "SERPER_API_KEY",
        };
        std::env::var(key_var).ok().map(|key| Self::new(provider, key))
    }
    
    pub fn provider(&self) -> Provider { self.provider }
    pub fn base_url(&self) -> &'static str { self.provider.base_url() }
    
    pub async fn chat(&self, request: UnifiedChatRequest) -> Result<UnifiedChatResponse, String> {
        let url = if self.provider.is_openai_compatible() {
            format!("{}/chat/completions", self.provider.base_url())
        } else {
            match self.provider {
                Provider::Anthropic => format!("{}/v1/messages", self.provider.base_url()),
                Provider::Gemini => format!("{}/models/{}:generateContent?key={}", 
                    self.provider.base_url(), request.model, self.api_key),
                _ => return Err("Unknown provider".to_string()),
            }
        };
        
        let body = if self.provider.is_openai_compatible() {
            ApiConverter::to_openai(&request)
        } else {
            match self.provider {
                Provider::Anthropic => ApiConverter::to_anthropic(&request),
                Provider::Gemini => ApiConverter::to_gemini(&request),
                _ => return Err("Unknown provider".to_string()),
            }
        };
        
        let mut req = self.client.post(&url).json(&body);
        req = match self.provider {
            Provider::Anthropic => req.header("x-api-key", &self.api_key).header("anthropic-version", "2023-06-01"),
            Provider::Gemini => req,
            _ => req.header("Authorization", format!("Bearer {}", self.api_key)),
        };
        
        let resp = req.send().await.map_err(|e| format!("Request failed: {}", e))?;
        if !resp.status().is_success() {
            return Err(format!("HTTP {}: {}", resp.status(), resp.text().await.unwrap_or_default()));
        }
        
        let json: serde_json::Value = resp.json().await.map_err(|e| format!("Parse failed: {}", e))?;
        
        let result = if self.provider.is_openai_compatible() {
            ApiConverter::from_openai(json)
        } else {
            match self.provider {
                Provider::Anthropic => ApiConverter::from_anthropic(json),
                Provider::Gemini => Self::from_gemini_static(json),
                _ => return Err("Unknown provider".to_string()),
            }
        };
        
        result.ok_or_else(|| "Failed to parse response".to_string())
    }
    
    pub async fn search(&self, query: String) -> Result<UnifiedSearchResponse, String> {
        let url = self.provider.base_url();
        let body = match self.provider {
            Provider::BraveSearch => serde_json::json!({ "q": query, "count": 10 }),
            Provider::TavilySearch => serde_json::json!({ "query": query, "api_key": self.api_key }),
            Provider::SerperSearch => serde_json::json!({ "q": query }),
            _ => return Err("Not a search provider".to_string()),
        };
        
        let mut req = self.client.post(url).json(&body);
        req = match self.provider {
            Provider::BraveSearch => req.header("X-Subscription-Token", &self.api_key),
            Provider::TavilySearch => req,
            Provider::SerperSearch => req.header("X-API-KEY", &self.api_key),
            _ => return Err("Not a search provider".to_string()),
        };
        
        let resp = req.send().await.map_err(|e| format!("Search failed: {}", e))?;
        if !resp.status().is_success() {
            return Err(format!("HTTP {}: {}", resp.status(), resp.text().await.unwrap_or_default()));
        }
        
        let json: serde_json::Value = resp.json().await.map_err(|e| format!("Parse failed: {}", e))?;
        
        let results = match self.provider {
            Provider::BraveSearch => json["web"]["results"].as_array(),
            Provider::TavilySearch => json["results"].as_array(),
            Provider::SerperSearch => json["organic"].as_array(),
            _ => None,
        };
        
        let results = results.map(|arr| arr.iter().filter_map(|r| {
            Some(SearchResult {
                title: r["title"].as_str()?.to_string(),
                url: r["url"].as_str()?.to_string(),
                snippet: r["snippet"].as_str()?.to_string(),
                score: r["score"].as_f64().map(|s| s as f32),
            })
        }).collect()).unwrap_or_default();
        
        Ok(UnifiedSearchResponse { query, results })
    }
    
    fn from_gemini_static(response: serde_json::Value) -> Option<UnifiedChatResponse> {
        let text = response["candidates"].as_array()?.first()?
            .get("content")?.get("parts")?.as_array()?.first()?
            .get("text")?.as_str()?;
        
        Some(UnifiedChatResponse {
            id: "gemini-1".to_string(), model: "gemini".to_string(),
            choices: vec![Choice {
                index: 0,
                message: UnifiedMessage { role: MessageRole::Assistant, content: MessageContent::Text(text.to_string()), name: None, tool_calls: None, tool_call_id: None },
                finish_reason: None,
            }],
            usage: None,
        })
    }
}

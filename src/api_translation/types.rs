//! Unified API Types for N-Way Conversion

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole { System, User, Assistant, Tool }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageUrl { pub url: String, pub detail: Option<String> }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ContentPart { Text { text: String }, ImageUrl { image_url: ImageUrl } }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedMessage {
    pub role: MessageRole,
    pub content: MessageContent,
    pub name: Option<String>,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent { Text(String), Parts(Vec<ContentPart>) }
impl From<String> for MessageContent { fn from(s: String) -> Self { MessageContent::Text(s) } }
impl From<&str> for MessageContent { fn from(s: &str) -> Self { MessageContent::Text(s.to_string()) } }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall { pub id: String, pub name: String, pub arguments: String }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedChatRequest {
    pub model: String, pub messages: Vec<UnifiedMessage>,
    pub temperature: Option<f32>, pub max_tokens: Option<u32>,
    pub stream: Option<bool>, pub tools: Option<Vec<ToolDefinition>>, pub system: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition { pub name: String, pub description: Option<String>, pub parameters: serde_json::Value }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedChatResponse {
    pub id: String, pub model: String, pub choices: Vec<Choice>, pub usage: Option<Usage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Choice { pub index: u32, pub message: UnifiedMessage, pub finish_reason: Option<String> }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage { pub prompt_tokens: u32, pub completion_tokens: u32, pub total_tokens: u32 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedSearchRequest { pub query: String, pub num_results: Option<u32>, pub search_depth: Option<String> }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedSearchResponse { pub query: String, pub results: Vec<SearchResult> }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult { pub title: String, pub url: String, pub snippet: String, pub score: Option<f32> }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    OpenAI, Anthropic, Gemini, DeepSeek, Moonshot, Groq, XAI, Cohere, Mistral,
    Perplexity, OpenRouter, NVIDIA, Cerebras, BraveSearch, TavilySearch, SerperSearch,
}

impl Provider {
    pub fn from_env_key(key: &str) -> Option<Self> {
        match key.to_uppercase().as_str() {
            s if s.contains("OPENAI") => Some(Provider::OpenAI),
            s if s.contains("ANTHROPIC") => Some(Provider::Anthropic),
            s if s.contains("GEMINI") || s.contains("GOOGLE") => Some(Provider::Gemini),
            s if s.contains("DEEPSEEK") => Some(Provider::DeepSeek),
            s if s.contains("MOONSHOT") || s.contains("KIMI") => Some(Provider::Moonshot),
            s if s.contains("GROQ") => Some(Provider::Groq),
            s if s.contains("XAI") || s.contains("GROK") => Some(Provider::XAI),
            s if s.contains("COHERE") => Some(Provider::Cohere),
            s if s.contains("MISTRAL") => Some(Provider::Mistral),
            s if s.contains("PERPLEXITY") => Some(Provider::Perplexity),
            s if s.contains("OPENROUTER") => Some(Provider::OpenRouter),
            s if s.contains("NVIDIA") => Some(Provider::NVIDIA),
            s if s.contains("CEREBRAS") => Some(Provider::Cerebras),
            s if s.contains("BRAVE") => Some(Provider::BraveSearch),
            s if s.contains("TAVILY") => Some(Provider::TavilySearch),
            s if s.contains("SERPER") => Some(Provider::SerperSearch),
            _ => None,
        }
    }
    
    pub fn base_url(&self) -> &'static str {
        match self {
            Provider::OpenAI => "https://api.openai.com/v1",
            Provider::Anthropic => "https://api.anthropic.com",
            Provider::Gemini => "https://generativelanguage.googleapis.com/v1beta",
            Provider::DeepSeek => "https://api.deepseek.com",
            Provider::Moonshot => "https://api.moonshot.cn/v1",
            Provider::Groq => "https://api.groq.com/openai/v1",
            Provider::XAI => "https://api.x.ai/v1",
            Provider::Cohere => "https://api.cohere.ai",
            Provider::Mistral => "https://api.mistral.ai/v1",
            Provider::Perplexity => "https://api.perplexity.ai",
            Provider::OpenRouter => "https://openrouter.ai/api/v1",
            Provider::NVIDIA => "https://integrate.api.nvidia.com/v1",
            Provider::Cerebras => "https://api.cerebras.ai/v1",
            Provider::BraveSearch => "https://api.search.brave.com/res/v1/web/search",
            Provider::TavilySearch => "https://api.tavily.com/search",
            Provider::SerperSearch => "https://google.serper.dev/search",
        }
    }
    
    pub fn is_openai_compatible(&self) -> bool {
        matches!(self, Provider::OpenAI | Provider::DeepSeek | Provider::Moonshot | 
            Provider::Groq | Provider::XAI | Provider::Mistral | Provider::Perplexity | 
            Provider::OpenRouter | Provider::NVIDIA | Provider::Cerebras)
    }
}

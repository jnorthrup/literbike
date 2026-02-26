// Provider facade models — normalized request/response types across AI providers
//
// Adapts OpenAI, Anthropic, Gemini to a unified chat interface.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatChoice {
    pub index: u32,
    pub message: Message,
    pub finish_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub id: String,
    pub model: String,
    pub choices: Vec<ChatChoice>,
    pub usage: Option<TokenUsage>,
}

pub trait ProviderAdapter: Send + Sync {
    fn to_request_body(&self, req: &ChatRequest) -> serde_json::Value;
    fn from_response_body(&self, body: serde_json::Value) -> Result<ChatResponse, String>;
}

/// OpenAI-compatible adapter (also used for Azure OpenAI)
pub struct OpenAiAdapter;

impl ProviderAdapter for OpenAiAdapter {
    fn to_request_body(&self, req: &ChatRequest) -> serde_json::Value {
        serde_json::to_value(req).unwrap_or(serde_json::json!({}))
    }
    fn from_response_body(&self, body: serde_json::Value) -> Result<ChatResponse, String> {
        serde_json::from_value(body).map_err(|e| e.to_string())
    }
}

/// Anthropic adapter — maps to /v1/messages format
pub struct AnthropicAdapter;

impl ProviderAdapter for AnthropicAdapter {
    fn to_request_body(&self, req: &ChatRequest) -> serde_json::Value {
        let (system, messages): (Option<&Message>, Vec<&Message>) = {
            let mut sys = None;
            let mut msgs = Vec::new();
            for m in &req.messages {
                if m.role == "system" { sys = Some(m); } else { msgs.push(m); }
            }
            (sys, msgs)
        };
        let mut body = serde_json::json!({
            "model": req.model,
            "messages": messages,
            "max_tokens": req.max_tokens.unwrap_or(1024),
        });
        if let Some(s) = system {
            body["system"] = serde_json::Value::String(s.content.clone());
        }
        body
    }
    fn from_response_body(&self, body: serde_json::Value) -> Result<ChatResponse, String> {
        // Map Anthropic's {content:[{text}]} to unified ChatResponse
        let id = body["id"].as_str().unwrap_or("").to_string();
        let model = body["model"].as_str().unwrap_or("").to_string();
        let text = body["content"].as_array()
            .and_then(|arr| arr.first())
            .and_then(|c| c["text"].as_str())
            .unwrap_or("");
        Ok(ChatResponse {
            id,
            model,
            choices: vec![ChatChoice {
                index: 0,
                message: Message { role: "assistant".into(), content: text.into() },
                finish_reason: body["stop_reason"].as_str().unwrap_or("stop").into(),
            }],
            usage: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn openai_roundtrip() {
        let req = ChatRequest {
            model: "gpt-4o".into(),
            messages: vec![Message { role: "user".into(), content: "hi".into() }],
            temperature: None,
            max_tokens: Some(256),
        };
        let adapter = OpenAiAdapter;
        let body = adapter.to_request_body(&req);
        assert_eq!(body["model"], "gpt-4o");
    }
}

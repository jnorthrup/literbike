//! API Format Converter

use crate::api_translation::types::*;

pub struct ApiConverter;

impl ApiConverter {
    pub fn to_openai(request: &UnifiedChatRequest) -> serde_json::Value {
        let messages: Vec<serde_json::Value> = request.messages.iter().map(|m| {
            let mut msg = serde_json::json!({ "role": Self::role_to_openai(&m.role), "content": Self::content_to_string(&m.content) });
            if let Some(ref tc) = m.tool_calls { msg["tool_calls"] = serde_json::to_value(tc).ok().unwrap(); }
            if let Some(ref id) = m.tool_call_id { msg["tool_call_id"] = serde_json::json!(id); }
            msg
        }).collect();
        let mut payload = serde_json::json!({ "model": request.model, "messages": messages });
        if let Some(t) = request.temperature { payload["temperature"] = serde_json::json!(t); }
        if let Some(m) = request.max_tokens { payload["max_tokens"] = serde_json::json!(m); }
        if let Some(s) = request.stream { payload["stream"] = serde_json::json!(s); }
        payload
    }
    
    pub fn from_openai(response: serde_json::Value) -> Option<UnifiedChatResponse> {
        let id = response["id"].as_str()?.to_string();
        let model = response["model"].as_str()?.to_string();
        let choices_arr = response["choices"].as_array()?;
        let mut choices = Vec::new();
        for c in choices_arr {
            let idx = c["index"].as_u64()? as u32;
            let role_str = c["message"]["role"].as_str()?;
            let role = Self::role_from_openai(role_str);
            let content_str = c["message"]["content"].as_str()?;
            let content = MessageContent::Text(content_str.to_string());
            let finish = c["finish_reason"].as_str().map(String::from);
            choices.push(Choice { index: idx, message: UnifiedMessage { role, content, name: None, tool_calls: None, tool_call_id: None }, finish_reason: finish });
        }
        let usage = response["usage"].as_object().and_then(|u| {
            Some(Usage {
                prompt_tokens: u["prompt_tokens"].as_u64()? as u32,
                completion_tokens: u["completion_tokens"].as_u64()? as u32,
                total_tokens: u["total_tokens"].as_u64()? as u32,
            })
        });
        Some(UnifiedChatResponse { id, model, choices, usage })
    }
    
    pub fn to_anthropic(request: &UnifiedChatRequest) -> serde_json::Value {
        let messages: Vec<serde_json::Value> = request.messages.iter()
            .filter(|m| m.role != MessageRole::System)
            .map(|m| serde_json::json!({ "role": Self::role_to_anthropic(&m.role), "content": Self::content_to_string(&m.content) }))
            .collect();
        let mut payload = serde_json::json!({ "model": request.model, "messages": messages, "max_tokens": request.max_tokens.unwrap_or(1024) });
        if let Some(ref sys) = request.system { payload["system"] = serde_json::json!(sys); }
        payload
    }
    
    pub fn from_anthropic(response: serde_json::Value) -> Option<UnifiedChatResponse> {
        let id = response["id"].as_str()?.to_string();
        let model = response["model"].as_str()?.to_string();
        let content_arr = response["content"].as_array()?;
        let content_text = content_arr.first()?.get("text")?.as_str()?.to_string();
        let finish = response["stop_reason"].as_str().map(String::from);
        let usage = response["usage"].as_object().and_then(|u| {
            Some(Usage {
                prompt_tokens: u["input_tokens"].as_u64()? as u32,
                completion_tokens: u["output_tokens"].as_u64()? as u32,
                total_tokens: (u["input_tokens"].as_u64()? + u["output_tokens"].as_u64()?) as u32,
            })
        });
        Some(UnifiedChatResponse {
            id, model,
            choices: vec![Choice { index: 0, message: UnifiedMessage { role: MessageRole::Assistant, content: MessageContent::Text(content_text), name: None, tool_calls: None, tool_call_id: None }, finish_reason: finish }],
            usage,
        })
    }
    
    pub fn to_gemini(request: &UnifiedChatRequest) -> serde_json::Value {
        let contents: Vec<serde_json::Value> = request.messages.iter()
            .filter(|m| m.role != MessageRole::System)
            .map(|m| serde_json::json!({ "role": Self::role_to_gemini(&m.role), "parts": [{"text": Self::content_to_string(&m.content)}] }))
            .collect();
        let mut payload = serde_json::json!({ "contents": contents });
        if let Some(ref sys) = request.system { payload["systemInstruction"] = serde_json::json!({ "parts": [{"text": sys}] }); }
        payload
    }
    
    fn role_to_openai(role: &MessageRole) -> &'static str { match role { MessageRole::System => "system", MessageRole::User => "user", MessageRole::Assistant => "assistant", MessageRole::Tool => "tool" } }
    fn role_from_openai(role: &str) -> MessageRole { match role { "system" => MessageRole::System, "user" => MessageRole::User, "assistant" => MessageRole::Assistant, "tool" => MessageRole::Tool, _ => MessageRole::User } }
    fn role_to_anthropic(role: &MessageRole) -> &'static str { match role { MessageRole::User => "user", MessageRole::Assistant => "assistant", _ => "user" } }
    fn role_to_gemini(role: &MessageRole) -> &'static str { match role { MessageRole::User => "user", MessageRole::Assistant => "model", _ => "user" } }
    fn content_to_string(content: &MessageContent) -> String {
        match content { MessageContent::Text(s) => s.clone(), MessageContent::Parts(parts) => {
            parts.iter().filter_map(|p| match p { ContentPart::Text { text } => Some(text.clone()), _ => None }).collect::<Vec<_>>().join(" ")
        }}
    }
}

//! N-Way API Translation Layer
//!
//! Unified API translation between all major AI providers:
//! - Gemini (Google)
//! - Codex/OpenAI (OpenAI)
//! - Anthropic (Claude)
//! - DeepSeek R1
//! - WebSearch (Brave, Tavily, Serper)
//! - Moonshot/Kimi, Groq, xAI/Grok, Cohere, Mistral, Perplexity, OpenRouter, NVIDIA, Cerebras

pub mod types;
pub mod openai;
pub mod anthropic;
pub mod gemini;
pub mod deepseek;
pub mod websearch;
pub mod client;
pub mod converter;

pub use types::*;
pub use converter::ApiConverter;
pub use client::UnifiedClient;

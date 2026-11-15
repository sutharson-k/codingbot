use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[async_trait]
pub trait LlmClient: Send + Sync {
    /// Generate a completion given system and user instructions
    async fn complete(&self, system_prompt: &str, user_prompt: &str) -> Result<String>;
}

/// Generic OpenAI-compatible client (works with OpenAI, Ollama, Groq, vLLM, DeepSeek)
pub struct OpenAiClient {
    api_key: String,
    base_url: String,
    model: String,
    client: reqwest::Client,
}

impl OpenAiClient {
    pub fn new(api_key: String, base_url: Option<String>, model: Option<String>) -> Self {
        let base_url = base_url
            .unwrap_or_else(|| "https://api.openai.com/v1".to_string())
            .trim_end_matches('/')
            .to_string();

        let model = model.unwrap_or_else(|| "gpt-4o".to_string());

        Self {
            api_key,
            base_url,
            model,
            client: reqwest::Client::new(),
        }
    }

    pub fn from_env() -> Option<Self> {
        let api_key = std::env::var("OPENAI_API_KEY").ok()?;
        let base_url = std::env::var("OPENAI_BASE_URL").ok();
        let model = std::env::var("CODINGBOT_MODEL").ok();
        Some(Self::new(api_key, base_url, model))
    }
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage<'a>>,
    temperature: f32,
}

#[derive(Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatResponseMessage,
}

#[derive(Deserialize)]
struct ChatResponseMessage {
    content: Option<String>,
}

#[async_trait]
impl LlmClient for OpenAiClient {
    async fn complete(&self, system_prompt: &str, user_prompt: &str) -> Result<String> {
        let url = format!("{}/chat/completions", self.base_url);
        let request = ChatRequest {
            model: &self.model,
            messages: vec![
                ChatMessage {
                    role: "system",
                    content: system_prompt,
                },
                ChatMessage {
                    role: "user",
                    content: user_prompt,
                },
            ],
            temperature: 0.2,
        };

        let res = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to send HTTP request to LLM endpoint")?;

        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            anyhow::bail!("LLM API returned error {}: {}", status, body);
        }

        let resp_body: ChatResponse = res
            .json()
            .await
            .context("Failed to deserialize LLM chat response")?;

        let text = resp_body
            .choices
            .first()
            .and_then(|c| c.message.content.clone())
            .unwrap_or_default();

        Ok(text)
    }
}

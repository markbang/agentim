use crate::config::AgentType;
use crate::error::{AgentError, Result};
use crate::session::Message;
use async_trait::async_trait;

fn openai_messages(messages: &[Message]) -> Vec<serde_json::Value> {
    messages
        .iter()
        .map(|message| {
            serde_json::json!({
                "role": message.role.to_string(),
                "content": message.content,
            })
        })
        .collect()
}

#[async_trait]
pub trait Agent: Send + Sync {
    fn agent_type(&self) -> AgentType;
    fn id(&self) -> &str;

    async fn send_message(&self, messages: Vec<Message>) -> Result<String>;
    async fn health_check(&self) -> Result<()>;
}

/// Claude Agent - 本地模拟实现
pub struct ClaudeAgent {
    id: String,
    model: String,
}

impl ClaudeAgent {
    pub fn new(id: String, model: Option<String>) -> Self {
        Self {
            id,
            model: model.unwrap_or_else(|| "claude-3-5-sonnet-20241022".to_string()),
        }
    }
}

#[async_trait]
impl Agent for ClaudeAgent {
    fn agent_type(&self) -> AgentType {
        AgentType::Claude
    }

    fn id(&self) -> &str {
        &self.id
    }

    async fn send_message(&self, messages: Vec<Message>) -> Result<String> {
        // 本地模拟响应，实际调用由外部CLI处理
        let last_msg = messages.last().map(|m| m.content.as_str()).unwrap_or("");
        let response = format!("[Claude {}] Processed: {}", self.model, last_msg);
        Ok(response)
    }

    async fn health_check(&self) -> Result<()> {
        // 本地检查，总是成功
        Ok(())
    }
}

/// Codex Agent - 本地模拟实现
pub struct CodexAgent {
    id: String,
    model: String,
}

impl CodexAgent {
    pub fn new(id: String, model: Option<String>) -> Self {
        Self {
            id,
            model: model.unwrap_or_else(|| "code-davinci-002".to_string()),
        }
    }
}

#[async_trait]
impl Agent for CodexAgent {
    fn agent_type(&self) -> AgentType {
        AgentType::Codex
    }

    fn id(&self) -> &str {
        &self.id
    }

    async fn send_message(&self, messages: Vec<Message>) -> Result<String> {
        let last_msg = messages.last().map(|m| m.content.as_str()).unwrap_or("");
        let response = format!("[Codex {}] Processed: {}", self.model, last_msg);
        Ok(response)
    }

    async fn health_check(&self) -> Result<()> {
        Ok(())
    }
}

/// Pi Agent - 本地模拟实现
pub struct PiAgent {
    id: String,
}

impl PiAgent {
    pub fn new(id: String) -> Self {
        Self { id }
    }
}

#[async_trait]
impl Agent for PiAgent {
    fn agent_type(&self) -> AgentType {
        AgentType::Pi
    }

    fn id(&self) -> &str {
        &self.id
    }

    async fn send_message(&self, messages: Vec<Message>) -> Result<String> {
        let last_msg = messages.last().map(|m| m.content.as_str()).unwrap_or("");
        let response = format!("[Pi] Processed: {}", last_msg);
        Ok(response)
    }

    async fn health_check(&self) -> Result<()> {
        Ok(())
    }
}

/// OpenAI-compatible Agent - 调用兼容 /chat/completions 的 HTTP backend
pub struct OpenAiCompatibleAgent {
    id: String,
    api_key: String,
    base_url: String,
    model: String,
    client: reqwest::Client,
}

impl OpenAiCompatibleAgent {
    pub fn new(id: String, api_key: String, base_url: String, model: String) -> Self {
        Self {
            id,
            api_key,
            base_url: base_url.trim_end_matches('/').to_string(),
            model,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl Agent for OpenAiCompatibleAgent {
    fn agent_type(&self) -> AgentType {
        AgentType::OpenAI
    }

    fn id(&self) -> &str {
        &self.id
    }

    async fn send_message(&self, messages: Vec<Message>) -> Result<String> {
        let response = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&serde_json::json!({
                "model": self.model,
                "messages": openai_messages(&messages),
            }))
            .send()
            .await?
            .error_for_status()?
            .json::<serde_json::Value>()
            .await?;

        response["choices"]
            .get(0)
            .and_then(|choice| choice.get("message"))
            .and_then(|message| message.get("content"))
            .and_then(|content| content.as_str())
            .map(|content| content.to_string())
            .ok_or_else(|| {
                AgentError::ApiError(
                    "OpenAI-compatible response missing choices[0].message.content".to_string(),
                )
            })
    }

    async fn health_check(&self) -> Result<()> {
        self.client
            .get(format!("{}/models", self.base_url))
            .bearer_auth(&self.api_key)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }
}

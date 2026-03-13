use async_trait::async_trait;
use crate::error::Result;
use crate::session::Message;
use crate::config::AgentType;

#[async_trait]
pub trait Agent: Send + Sync {
    fn agent_type(&self) -> AgentType;
    fn id(&self) -> &str;

    async fn send_message(&self, messages: Vec<Message>) -> Result<String>;
    async fn health_check(&self) -> Result<()>;
}

pub struct ClaudeAgent {
    id: String,
    api_key: String,
    model: String,
    base_url: String,
}

impl ClaudeAgent {
    pub fn new(id: String, api_key: String, model: Option<String>, base_url: Option<String>) -> Self {
        Self {
            id,
            api_key,
            model: model.unwrap_or_else(|| "claude-3-5-sonnet-20241022".to_string()),
            base_url: base_url.unwrap_or_else(|| "https://api.anthropic.com".to_string()),
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
        let client = reqwest::Client::new();

        let formatted_messages: Vec<serde_json::Value> = messages
            .iter()
            .map(|m| {
                serde_json::json!({
                    "role": m.role.to_string(),
                    "content": m.content
                })
            })
            .collect();

        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": 1024,
            "messages": formatted_messages
        });

        let response = client
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await?;

        let result: serde_json::Value = response.json().await?;

        result["content"][0]["text"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| crate::error::AgentError::ApiError("Invalid response format".to_string()))
    }

    async fn health_check(&self) -> Result<()> {
        let client = reqwest::Client::new();
        client
            .get(format!("{}/v1/models", self.base_url))
            .header("x-api-key", &self.api_key)
            .send()
            .await?;
        Ok(())
    }
}

pub struct CodexAgent {
    id: String,
    api_key: String,
    model: String,
}

impl CodexAgent {
    pub fn new(id: String, api_key: String, model: Option<String>) -> Self {
        Self {
            id,
            api_key,
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
        let client = reqwest::Client::new();

        let prompt = messages
            .iter()
            .map(|m| format!("{}: {}", m.role, m.content))
            .collect::<Vec<_>>()
            .join("\n");

        let body = serde_json::json!({
            "model": self.model,
            "prompt": prompt,
            "max_tokens": 1024,
            "temperature": 0.7
        });

        let response = client
            .post("https://api.openai.com/v1/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await?;

        let result: serde_json::Value = response.json().await?;

        result["choices"][0]["text"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| crate::error::AgentError::ApiError("Invalid response format".to_string()))
    }

    async fn health_check(&self) -> Result<()> {
        let client = reqwest::Client::new();
        client
            .get("https://api.openai.com/v1/models")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await?;
        Ok(())
    }
}

pub struct PiAgent {
    id: String,
    api_key: String,
}

impl PiAgent {
    pub fn new(id: String, api_key: String) -> Self {
        Self { id, api_key }
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
        let client = reqwest::Client::new();

        let formatted_messages: Vec<serde_json::Value> = messages
            .iter()
            .map(|m| {
                serde_json::json!({
                    "role": m.role.to_string(),
                    "content": m.content
                })
            })
            .collect();

        let body = serde_json::json!({
            "messages": formatted_messages
        });

        let response = client
            .post("https://api.pi.ai/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await?;

        let result: serde_json::Value = response.json().await?;

        result["choices"][0]["message"]["content"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| crate::error::AgentError::ApiError("Invalid response format".to_string()))
    }

    async fn health_check(&self) -> Result<()> {
        let client = reqwest::Client::new();
        client
            .get("https://api.pi.ai/v1/models")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await?;
        Ok(())
    }
}

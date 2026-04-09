use crate::config::AgentType;
use crate::error::{AgentError, Result};
use crate::session::{Message, Session};
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
    async fn send_message_with_session(
        &self,
        session: &mut Session,
        messages: Vec<Message>,
    ) -> Result<String> {
        let _ = session;
        self.send_message(messages).await
    }
    async fn health_check(&self) -> Result<()>;
}

/// OpenAI-compatible Agent - calls any /chat/completions HTTP backend
pub struct OpenAiCompatibleAgent {
    id: String,
    api_key: String,
    base_url: String,
    model: String,
    max_retries: usize,
    client: reqwest::Client,
}

impl OpenAiCompatibleAgent {
    pub fn new(id: String, api_key: String, base_url: String, model: String) -> Self {
        Self {
            id,
            api_key,
            base_url: base_url.trim_end_matches('/').to_string(),
            model,
            max_retries: 0,
            client: reqwest::Client::new(),
        }
    }

    pub fn with_max_retries(mut self, max_retries: usize) -> Self {
        self.max_retries = max_retries;
        self
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
        let payload = serde_json::json!({
            "model": self.model,
            "messages": openai_messages(&messages),
        });

        for attempt in 0..=self.max_retries {
            let response = self
                .client
                .post(format!("{}/chat/completions", self.base_url))
                .bearer_auth(&self.api_key)
                .json(&payload)
                .send()
                .await;

            match response {
                Ok(response) if response.status().is_success() => {
                    let response = response.json::<serde_json::Value>().await?;
                    return response["choices"]
                        .get(0)
                        .and_then(|choice| choice.get("message"))
                        .and_then(|message| message.get("content"))
                        .and_then(|content| content.as_str())
                        .map(|content| content.to_string())
                        .ok_or_else(|| {
                            AgentError::ApiError(
                                "OpenAI-compatible response missing choices[0].message.content"
                                    .to_string(),
                            )
                        });
                }
                Ok(response) => {
                    let status = response.status();
                    let body = response.text().await.unwrap_or_default();
                    if status.is_server_error() && attempt < self.max_retries {
                        continue;
                    }
                    return Err(AgentError::ApiError(format!(
                        "OpenAI-compatible request failed with {}: {}",
                        status, body
                    )));
                }
                Err(err) => {
                    if attempt < self.max_retries {
                        continue;
                    }
                    return Err(err.into());
                }
            }
        }

        Err(AgentError::Unknown(
            "OpenAI-compatible retry loop exited unexpectedly".to_string(),
        ))
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

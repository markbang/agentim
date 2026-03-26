use crate::channel::{Channel, ChannelMessage};
use crate::config::ChannelType;
use crate::error::Result;
use crate::manager::AgentIM;
use async_trait::async_trait;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub const TELEGRAM_CHANNEL_ID: &str = "telegram-bot";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramUpdate {
    pub update_id: i64,
    pub message: Option<TelegramMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramMessage {
    pub message_id: i64,
    pub chat: TelegramChat,
    pub text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramChat {
    pub id: i64,
}

pub struct TelegramBotChannel {
    id: String,
    token: String,
    api_url: String,
    pending_messages: Arc<DashMap<String, Vec<String>>>,
}

impl TelegramBotChannel {
    pub fn new(id: String, token: String) -> Self {
        let api_url = format!("https://api.telegram.org/bot{}", token);
        Self {
            id,
            token,
            api_url,
            pending_messages: Arc::new(DashMap::new()),
        }
    }

    pub async fn set_webhook(&self, webhook_url: &str) -> Result<()> {
        let client = reqwest::Client::new();
        let url = format!("{}/setWebhook", self.api_url);
        let params = serde_json::json!({
            "url": webhook_url
        });

        client
            .post(&url)
            .json(&params)
            .send()
            .await
            .map_err(|e| crate::error::AgentError::ChannelError(e.to_string()))?;

        Ok(())
    }

    pub fn get_pending_messages(&self, user_id: &str) -> Vec<String> {
        self.pending_messages
            .remove(user_id)
            .map(|(_, msgs)| msgs)
            .unwrap_or_default()
    }

    pub fn add_pending_message(&self, user_id: String, message: String) {
        self.pending_messages
            .entry(user_id)
            .or_default()
            .push(message);
    }
}

#[async_trait]
impl Channel for TelegramBotChannel {
    fn channel_type(&self) -> ChannelType {
        ChannelType::Telegram
    }

    fn id(&self) -> &str {
        &self.id
    }

    async fn send_message(&self, user_id: &str, content: &str) -> Result<()> {
        let client = reqwest::Client::new();
        let url = format!("{}/sendMessage", self.api_url);
        let chat_id: i64 = user_id.parse()?;

        let params = serde_json::json!({
            "chat_id": chat_id,
            "text": content
        });

        client
            .post(&url)
            .json(&params)
            .send()
            .await
            .map_err(|e| crate::error::AgentError::ChannelError(e.to_string()))?;

        Ok(())
    }

    async fn receive_message(&self) -> Result<Option<ChannelMessage>> {
        Ok(None)
    }

    async fn health_check(&self) -> Result<()> {
        let client = reqwest::Client::new();
        let url = format!("{}/getMe", self.api_url);

        client.get(&url).send().await.map_err(|e| {
            crate::error::AgentError::ChannelError(format!("Telegram health check failed: {}", e))
        })?;

        Ok(())
    }
}

pub async fn telegram_webhook_handler(
    agentim: Arc<AgentIM>,
    agent_id: &str,
    update: TelegramUpdate,
) -> Result<()> {
    if let Some(message) = update.message {
        if let Some(text) = message.text {
            let user_id = message.chat.id.to_string();
            agentim
                .handle_incoming_message(
                    agent_id,
                    TELEGRAM_CHANNEL_ID,
                    &user_id,
                    Some(&user_id),
                    text,
                )
                .await?;
        }
    }

    Ok(())
}

use crate::channel::{Channel, ChannelMessage};
use crate::config::ChannelType;
use crate::error::Result;
use crate::manager::AgentIM;
use async_trait::async_trait;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub const QQ_CHANNEL_ID: &str = "qq-bot";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QQMessage {
    pub id: String,
    pub author: QQUser,
    pub content: String,
    pub channel_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QQUser {
    pub id: String,
    pub username: String,
}

pub struct QQBotChannel {
    id: String,
    bot_id: String,
    bot_token: String,
    api_url: String,
    pending_messages: Arc<DashMap<String, Vec<String>>>,
}

impl QQBotChannel {
    pub fn new(id: String, bot_id: String, bot_token: String) -> Self {
        let api_url = "https://api.sgroup.qq.com".to_string();
        Self {
            id,
            bot_id,
            bot_token,
            api_url,
            pending_messages: Arc::new(DashMap::new()),
        }
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
impl Channel for QQBotChannel {
    fn channel_type(&self) -> ChannelType {
        ChannelType::QQ
    }

    fn id(&self) -> &str {
        &self.id
    }

    async fn send_message(&self, user_id: &str, content: &str) -> Result<()> {
        let client = reqwest::Client::new();
        let url = format!("{}/channels/{}/messages", self.api_url, user_id);

        let params = serde_json::json!({
            "content": content,
            "msg_type": 0
        });

        client
            .post(&url)
            .header(
                "Authorization",
                format!("Bot {}.{}", self.bot_id, self.bot_token),
            )
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
        let url = format!("{}/users/@me", self.api_url);

        client
            .get(&url)
            .header(
                "Authorization",
                format!("Bot {}.{}", self.bot_id, self.bot_token),
            )
            .send()
            .await
            .map_err(|e| {
                crate::error::AgentError::ChannelError(format!("QQ health check failed: {}", e))
            })?;

        Ok(())
    }
}

pub async fn qq_webhook_handler(
    agentim: Arc<AgentIM>,
    agent_id: &str,
    max_session_messages: Option<usize>,
    context_message_limit: usize,
    message: QQMessage,
) -> Result<()> {
    let user_id = message.author.id;
    let reply_target = message.channel_id;
    let content = message.content;

    agentim
        .handle_incoming_message_with_limits(
            agent_id,
            QQ_CHANNEL_ID,
            &user_id,
            Some(&reply_target),
            content,
            max_session_messages,
            context_message_limit,
        )
        .await?;

    Ok(())
}

use crate::channel::{Channel, ChannelMessage};
use crate::config::ChannelType;
use crate::error::Result;
use crate::manager::AgentIM;
use async_trait::async_trait;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub const DISCORD_CHANNEL_ID: &str = "discord-bot";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordMessage {
    pub id: String,
    pub author: DiscordUser,
    pub content: String,
    pub channel_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordUser {
    pub id: String,
    pub username: String,
}

pub struct DiscordBotChannel {
    id: String,
    token: String,
    api_url: String,
    pending_messages: Arc<DashMap<String, Vec<String>>>,
}

impl DiscordBotChannel {
    pub fn new(id: String, token: String) -> Self {
        let api_url = "https://discord.com/api/v10".to_string();
        Self {
            id,
            token,
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
impl Channel for DiscordBotChannel {
    fn channel_type(&self) -> ChannelType {
        ChannelType::Discord
    }

    fn id(&self) -> &str {
        &self.id
    }

    async fn send_message(&self, user_id: &str, content: &str) -> Result<()> {
        let client = reqwest::Client::new();
        let url = format!("{}/channels/{}/messages", self.api_url, user_id);

        let params = serde_json::json!({
            "content": content
        });

        client
            .post(&url)
            .header("Authorization", format!("Bot {}", self.token))
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
            .header("Authorization", format!("Bot {}", self.token))
            .send()
            .await
            .map_err(|e| {
                crate::error::AgentError::ChannelError(format!(
                    "Discord health check failed: {}",
                    e
                ))
            })?;

        Ok(())
    }
}

pub async fn discord_webhook_handler(
    agentim: Arc<AgentIM>,
    agent_id: &str,
    max_session_messages: Option<usize>,
    message: DiscordMessage,
) -> Result<()> {
    let user_id = message.author.id;
    let reply_target = message.channel_id;
    let content = message.content;

    agentim
        .handle_incoming_message_with_limit(
            agent_id,
            DISCORD_CHANNEL_ID,
            &user_id,
            Some(&reply_target),
            content,
            max_session_messages,
        )
        .await?;

    Ok(())
}

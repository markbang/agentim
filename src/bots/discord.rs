use crate::channel::{Channel, ChannelMessage};
use crate::config::ChannelType;
use crate::error::Result;
use crate::manager::AgentIM;
use async_trait::async_trait;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

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
            .or_insert_with(Vec::new)
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
            .map_err(|e| crate::error::AgentError::ChannelError(format!("Discord health check failed: {}", e)))?;

        Ok(())
    }
}

pub async fn discord_webhook_handler(
    channel: Arc<DiscordBotChannel>,
    agentim: Arc<AgentIM>,
    message: DiscordMessage,
) -> Result<()> {
    let user_id = message.author.id.clone();
    let content = message.content.clone();

    // Store the message
    channel.add_pending_message(user_id.clone(), content.clone());

    // Find sessions for this user
    let sessions = agentim.list_sessions();
    for session in sessions {
        if session.user_id == user_id && session.channel_id == channel.id() {
            // Send to agent
            if let Ok(response) = agentim.send_to_agent(&session.id, content.clone()).await {
                // Send response back to Discord
                let _ = channel.send_message(&message.channel_id, &response).await;
            }
        }
    }

    Ok(())
}

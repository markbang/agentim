use crate::channel::{Channel, ChannelMessage};
use crate::config::ChannelType;
use crate::error::Result;
use crate::manager::AgentIM;
use async_trait::async_trait;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuMessage {
    pub token: String,
    pub ts: String,
    pub uuid: String,
    pub event: FeishuEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuEvent {
    pub message: FeishuMessageContent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuMessageContent {
    pub chat_id: String,
    pub sender_id: FeishuSender,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuSender {
    pub user_id: String,
}

pub struct FeishuBotChannel {
    id: String,
    app_id: String,
    app_secret: String,
    api_url: String,
    pending_messages: Arc<DashMap<String, Vec<String>>>,
}

impl FeishuBotChannel {
    pub fn new(id: String, app_id: String, app_secret: String) -> Self {
        let api_url = "https://open.feishu.cn/open-apis".to_string();
        Self {
            id,
            app_id,
            app_secret,
            api_url,
            pending_messages: Arc::new(DashMap::new()),
        }
    }

    async fn get_access_token(&self) -> Result<String> {
        let client = reqwest::Client::new();
        let url = format!("{}/auth/v3/tenant_access_token/internal", self.api_url);

        let params = serde_json::json!({
            "app_id": self.app_id,
            "app_secret": self.app_secret
        });

        let response = client
            .post(&url)
            .json(&params)
            .send()
            .await
            .map_err(|e| crate::error::AgentError::ChannelError(e.to_string()))?;

        let data: serde_json::Value = response
            .json::<serde_json::Value>()
            .await
            .map_err(|e| crate::error::AgentError::ChannelError(e.to_string()))?;

        data["tenant_access_token"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| crate::error::AgentError::ChannelError("Failed to get access token".to_string()))
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
impl Channel for FeishuBotChannel {
    fn channel_type(&self) -> ChannelType {
        ChannelType::Feishu
    }

    fn id(&self) -> &str {
        &self.id
    }

    async fn send_message(&self, user_id: &str, content: &str) -> Result<()> {
        let token = self.get_access_token().await?;
        let client = reqwest::Client::new();
        let url = format!("{}/im/v1/messages", self.api_url);

        let params = serde_json::json!({
            "receive_id": user_id,
            "receive_id_type": "user_id",
            "msg_type": "text",
            "content": {
                "text": content
            }
        });

        client
            .post(&url)
            .header("Authorization", format!("Bearer {}", token))
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
        self.get_access_token().await?;
        Ok(())
    }
}

pub async fn feishu_webhook_handler(
    channel: Arc<FeishuBotChannel>,
    agentim: Arc<AgentIM>,
    message: FeishuMessage,
) -> Result<()> {
    let user_id = message.event.message.sender_id.user_id.clone();
    let content = message.event.message.content.clone();
    let _chat_id = message.event.message.chat_id.clone();

    // Store the message
    channel.add_pending_message(user_id.clone(), content.clone());

    // Find sessions for this user
    let sessions = agentim.list_sessions();
    for session in sessions {
        if session.user_id == user_id && session.channel_id == channel.id() {
            // Send to agent
            if let Ok(response) = agentim.send_to_agent(&session.id, content.clone()).await {
                // Send response back to Feishu
                let _ = channel.send_message(&user_id, &response).await;
            }
        }
    }

    Ok(())
}

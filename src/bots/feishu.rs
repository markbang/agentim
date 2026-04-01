use crate::channel::{Channel, ChannelMessage};
use crate::config::ChannelType;
use crate::error::Result;
use crate::manager::{AgentIM, MessageHandlingOptions};
use async_trait::async_trait;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub const FEISHU_CHANNEL_ID: &str = "feishu-bot";

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
            .ok_or_else(|| {
                crate::error::AgentError::ChannelError("Failed to get access token".to_string())
            })
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
    agentim: Arc<AgentIM>,
    agent_id: &str,
    max_session_messages: Option<usize>,
    context_message_limit: usize,
    agent_timeout_ms: Option<u64>,
    message: FeishuMessage,
) -> Result<()> {
    let user_id = message.event.message.sender_id.user_id;
    let content = message.event.message.content;

    agentim
        .handle_incoming_message_with_options(
            agent_id,
            FEISHU_CHANNEL_ID,
            &user_id,
            Some(&user_id),
            content,
            MessageHandlingOptions {
                max_messages: max_session_messages,
                context_message_limit,
                agent_timeout_ms,
            },
        )
        .await?;

    Ok(())
}

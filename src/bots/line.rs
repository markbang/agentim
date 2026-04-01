use crate::channel::{Channel, ChannelMessage};
use crate::config::ChannelType;
use crate::error::Result;
use crate::manager::{AgentIM, MessageHandlingOptions};
use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD, Engine};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::sync::Arc;

pub const LINE_CHANNEL_ID: &str = "line-bot";

pub type HmacSha256 = Hmac<Sha256>;

/// LINE webhook event wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineWebhook {
    pub destination: Option<String>,
    pub events: Vec<LineEvent>,
}

/// LINE webhook event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    #[serde(rename = "replyToken")]
    pub reply_token: Option<String>,
    pub timestamp: Option<i64>,
    pub source: Option<LineSource>,
    pub message: Option<LineMessage>,
}

/// LINE event source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineSource {
    #[serde(rename = "type")]
    pub source_type: String,
    #[serde(rename = "userId")]
    pub user_id: Option<String>,
    #[serde(rename = "groupId")]
    pub group_id: Option<String>,
    #[serde(rename = "roomId")]
    pub room_id: Option<String>,
}

/// LINE message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineMessage {
    pub id: Option<String>,
    #[serde(rename = "type")]
    pub message_type: String,
    pub text: Option<String>,
}

/// LINE reply message
#[derive(Debug, Clone, Serialize)]
pub struct LineReplyRequest {
    #[serde(rename = "replyToken")]
    pub reply_token: String,
    pub messages: Vec<LineOutMessage>,
}

/// LINE push message
#[derive(Debug, Clone, Serialize)]
pub struct LinePushRequest {
    pub to: String,
    pub messages: Vec<LineOutMessage>,
}

/// LINE output message
#[derive(Debug, Clone, Serialize)]
pub struct LineOutMessage {
    #[serde(rename = "type")]
    pub message_type: String,
    pub text: Option<String>,
}

pub struct LineBotChannel {
    id: String,
    channel_token: String,
    channel_secret: Option<String>,
    client: reqwest::Client,
}

impl LineBotChannel {
    pub fn new(id: String, channel_token: String, channel_secret: Option<String>) -> Self {
        Self {
            id,
            channel_token,
            channel_secret,
            client: reqwest::Client::new(),
        }
    }

    /// Verify LINE signature
    pub fn verify_signature(&self, body: &[u8], signature: &str) -> bool {
        let Some(secret) = self.channel_secret.as_ref() else {
            return true;
        };

        let mut mac = match HmacSha256::new_from_slice(secret.as_bytes()) {
            Ok(m) => m,
            Err(_) => return false,
        };

        mac.update(body);
        let computed = STANDARD.encode(mac.finalize().into_bytes());

        computed == signature
    }

    /// Reply to a message
    pub async fn reply_message(&self, reply_token: &str, text: &str) -> Result<()> {
        let url = "https://api.line.me/v2/bot/message/reply";

        let request = LineReplyRequest {
            reply_token: reply_token.to_string(),
            messages: vec![LineOutMessage {
                message_type: "text".to_string(),
                text: Some(text.to_string()),
            }],
        };

        let response = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", self.channel_token))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                crate::error::AgentError::ChannelError(format!("LINE API error: {}", e))
            })?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(crate::error::AgentError::ChannelError(format!(
                "LINE API returned {}: {}",
                status, body
            )));
        }

        Ok(())
    }

    /// Push message to a user/group/room
    pub async fn push_message(&self, to: &str, text: &str) -> Result<()> {
        let url = "https://api.line.me/v2/bot/message/push";

        let request = LinePushRequest {
            to: to.to_string(),
            messages: vec![LineOutMessage {
                message_type: "text".to_string(),
                text: Some(text.to_string()),
            }],
        };

        let response = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", self.channel_token))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                crate::error::AgentError::ChannelError(format!("LINE API error: {}", e))
            })?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(crate::error::AgentError::ChannelError(format!(
                "LINE API returned {}: {}",
                status, body
            )));
        }

        Ok(())
    }
}

#[async_trait]
impl Channel for LineBotChannel {
    fn channel_type(&self) -> ChannelType {
        ChannelType::Line
    }

    fn id(&self) -> &str {
        &self.id
    }

    async fn send_message(&self, user_id: &str, content: &str) -> Result<()> {
        self.push_message(user_id, content).await
    }

    async fn receive_message(&self) -> Result<Option<ChannelMessage>> {
        Ok(None)
    }

    async fn health_check(&self) -> Result<()> {
        let url = "https://api.line.me/v2/bot/info";

        let response = self
            .client
            .get(url)
            .header("Authorization", format!("Bearer {}", self.channel_token))
            .send()
            .await
            .map_err(|e| {
                crate::error::AgentError::ChannelError(format!("LINE health check failed: {}", e))
            })?;

        if !response.status().is_success() {
            return Err(crate::error::AgentError::ChannelError(
                "LINE health check failed".to_string(),
            ));
        }

        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

pub async fn line_webhook_handler(
    agentim: Arc<AgentIM>,
    agent_id: &str,
    max_session_messages: Option<usize>,
    context_message_limit: usize,
    agent_timeout_ms: Option<u64>,
    webhook: LineWebhook,
) -> Result<()> {
    for event in webhook.events {
        // Only process message events
        if event.event_type != "message" {
            continue;
        }

        let Some(message) = &event.message else {
            continue;
        };

        // Only process text messages
        if message.message_type != "text" {
            continue;
        }

        let Some(text) = &message.text else {
            continue;
        };

        // Skip empty messages
        if text.trim().is_empty() {
            continue;
        }

        let Some(source) = &event.source else {
            continue;
        };

        // Get user ID
        let user_id = source.user_id.clone().unwrap_or_default();

        // Determine reply target (group, room, or user)
        let reply_target: Option<&str> = source
            .group_id
            .as_ref()
            .or(source.room_id.as_ref())
            .or(source.user_id.as_ref())
            .map(|s| s.as_str());

        agentim
            .handle_incoming_message_with_options(
                agent_id,
                LINE_CHANNEL_ID,
                &user_id,
                reply_target,
                text.clone(),
                MessageHandlingOptions {
                    max_messages: max_session_messages,
                    context_message_limit,
                    agent_timeout_ms,
                },
            )
            .await?;
    }

    Ok(())
}

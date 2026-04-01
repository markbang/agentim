use crate::channel::{Channel, ChannelMessage};
use crate::config::ChannelType;
use crate::error::Result;
use crate::manager::AgentIM;
use async_trait::async_trait;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::sync::Arc;

pub const DINGTALK_CHANNEL_ID: &str = "dingtalk-bot";

pub type HmacSha256 = Hmac<Sha256>;

/// DingTalk outgoing webhook (from DingTalk to bot)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DingTalkWebhook {
    #[serde(rename = "conversationId")]
    pub conversation_id: String,
    #[serde(rename = "atUserIds")]
    pub at_user_ids: Option<Vec<String>>,
    #[serde(rename = "chatbotCorpId")]
    pub chatbot_corp_id: Option<String>,
    #[serde(rename = "chatbotUserId")]
    pub chatbot_user_id: Option<String>,
    pub msgtype: String,
    pub text: DingTalkText,
    pub msgid: Option<String>,
    #[serde(rename = "createAt")]
    pub create_at: Option<i64>,
    #[serde(rename = "conversationType")]
    pub conversation_type: Option<String>,
    #[serde(rename = "senderId")]
    pub sender_id: String,
    #[serde(rename = "senderNick")]
    pub sender_nick: Option<String>,
    #[serde(rename = "senderCorpId")]
    pub sender_corp_id: Option<String>,
    #[serde(rename = "senderStaffId")]
    pub sender_staff_id: Option<String>,
    #[serde(rename = "sessionWebhookExpiredTime")]
    pub session_webhook_expired_time: Option<i64>,
    #[serde(rename = "sessionWebhook")]
    pub session_webhook: Option<String>,
    #[serde(rename = "isAdmin")]
    pub is_admin: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DingTalkText {
    pub content: Option<String>,
}

/// DingTalk message for sending via robot
#[derive(Debug, Clone, Serialize)]
pub struct DingTalkOutMessage {
    pub msgtype: String,
    pub text: DingTalkOutText,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at: Option<DingTalkAt>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DingTalkOutText {
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DingTalkAt {
    #[serde(rename = "atUserIds")]
    pub at_user_ids: Option<Vec<String>>,
    #[serde(rename = "isAtAll")]
    pub is_at_all: Option<bool>,
}

pub struct DingTalkBotChannel {
    id: String,
    webhook_url: Option<String>,
    secret: Option<String>,
    access_token: Option<String>,
    client: reqwest::Client,
}

impl DingTalkBotChannel {
    pub fn new(id: String, webhook_url: Option<String>, secret: Option<String>) -> Self {
        Self {
            id,
            webhook_url,
            secret,
            access_token: None,
            client: reqwest::Client::new(),
        }
    }

    pub fn with_access_token(id: String, access_token: String, secret: Option<String>) -> Self {
        Self {
            id,
            webhook_url: None,
            secret,
            access_token: Some(access_token),
            client: reqwest::Client::new(),
        }
    }

    /// Generate signature for DingTalk webhook
    pub fn generate_signature(&self, timestamp: i64) -> Option<String> {
        let secret = self.secret.as_ref()?;
        let string_to_sign = format!("{}\n{}", timestamp, secret);

        let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
            .map_err(|e| tracing::error!("HMAC error: {}", e))
            .ok()?;

        mac.update(string_to_sign.as_bytes());
        let signature = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            mac.finalize().into_bytes(),
        );
        Some(signature)
    }

    fn build_webhook_url(&self) -> Option<String> {
        if let Some(ref url) = self.webhook_url {
            return Some(url.clone());
        }

        if let Some(ref token) = self.access_token {
            let timestamp = chrono::Utc::now().timestamp_millis();

            if let Some(signature) = self.generate_signature(timestamp) {
                Some(format!(
                    "https://oapi.dingtalk.com/robot/send?access_token={}&timestamp={}&sign={}",
                    token, timestamp, signature
                ))
            } else {
                Some(format!(
                    "https://oapi.dingtalk.com/robot/send?access_token={}",
                    token
                ))
            }
        } else {
            None
        }
    }

    /// Send message via DingTalk robot webhook
    pub async fn send_robot_message(
        &self,
        content: &str,
        at_user_ids: Option<Vec<String>>,
    ) -> Result<()> {
        let url = self.build_webhook_url().ok_or_else(|| {
            crate::error::AgentError::ChannelError(
                "DingTalk webhook URL or access token not configured".to_string(),
            )
        })?;

        let message = DingTalkOutMessage {
            msgtype: "text".to_string(),
            text: DingTalkOutText {
                content: content.to_string(),
            },
            at: at_user_ids.map(|ids| DingTalkAt {
                at_user_ids: Some(ids),
                is_at_all: None,
            }),
        };

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&message)
            .send()
            .await
            .map_err(|e| {
                crate::error::AgentError::ChannelError(format!("DingTalk API error: {}", e))
            })?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(crate::error::AgentError::ChannelError(format!(
                "DingTalk API returned {}: {}",
                status, body
            )));
        }

        let response_json: serde_json::Value = response.json().await.map_err(|e| {
            crate::error::AgentError::ChannelError(format!(
                "Failed to parse DingTalk response: {}",
                e
            ))
        })?;

        let errcode = response_json
            .get("errcode")
            .and_then(|v| v.as_i64())
            .unwrap_or(-1);

        if errcode != 0 {
            let errmsg = response_json
                .get("errmsg")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown error");
            return Err(crate::error::AgentError::ChannelError(format!(
                "DingTalk API error: {}",
                errmsg
            )));
        }

        Ok(())
    }
}

#[async_trait]
impl Channel for DingTalkBotChannel {
    fn channel_type(&self) -> ChannelType {
        ChannelType::DingTalk
    }

    fn id(&self) -> &str {
        &self.id
    }

    async fn send_message(&self, user_id: &str, content: &str) -> Result<()> {
        // user_id could be a sender staff ID to @mention
        let at_user_ids = if user_id.starts_with("staff:") {
            Some(vec![user_id.trim_start_matches("staff:").to_string()])
        } else {
            None
        };
        self.send_robot_message(content, at_user_ids).await
    }

    async fn receive_message(&self) -> Result<Option<ChannelMessage>> {
        Ok(None)
    }

    async fn health_check(&self) -> Result<()> {
        // For webhook-based channels, we just check if credentials are configured
        if self.webhook_url.is_some() || self.access_token.is_some() {
            Ok(())
        } else {
            Err(crate::error::AgentError::ChannelError(
                "DingTalk webhook URL or access token not configured".to_string(),
            ))
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

pub async fn dingtalk_webhook_handler(
    agentim: Arc<AgentIM>,
    agent_id: &str,
    max_session_messages: Option<usize>,
    context_message_limit: usize,
    agent_timeout_ms: Option<u64>,
    webhook: DingTalkWebhook,
) -> Result<()> {
    let text = webhook.text.content.unwrap_or_default();

    // Skip empty messages
    if text.trim().is_empty() {
        return Ok(());
    }

    // Use sender_staff_id if available, otherwise sender_id
    let user_id = webhook
        .sender_staff_id
        .clone()
        .unwrap_or_else(|| webhook.sender_id.clone());

    // Use conversation_id as reply target (for group chats)
    let reply_target = Some(webhook.conversation_id.as_str());

    agentim
        .handle_incoming_message_with_runtime_limits(
            agent_id,
            DINGTALK_CHANNEL_ID,
            &user_id,
            reply_target,
            text,
            max_session_messages,
            context_message_limit,
            agent_timeout_ms,
        )
        .await?;

    Ok(())
}

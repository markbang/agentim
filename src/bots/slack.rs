use crate::channel::{Channel, ChannelMessage};
use crate::config::ChannelType;
use crate::error::Result;
use crate::manager::{AgentIM, MessageHandlingOptions};
use async_trait::async_trait;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::sync::Arc;

pub const SLACK_CHANNEL_ID: &str = "slack-bot";

pub type HmacSha256 = Hmac<Sha256>;

/// Slack webhook event wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackEvent {
    pub token: String,
    #[serde(rename = "team_id")]
    pub team_id: String,
    #[serde(rename = "api_app_id")]
    pub api_app_id: String,
    pub event: Option<SlackEventDetail>,
    #[serde(rename = "type")]
    pub event_type: String,
    #[serde(default)]
    pub challenge: Option<String>,
    #[serde(rename = "event_id")]
    pub event_id: String,
    #[serde(rename = "event_time")]
    pub event_time: i64,
}

/// Slack event detail
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackEventDetail {
    #[serde(rename = "type")]
    pub detail_type: String,
    pub user: Option<String>,
    pub text: Option<String>,
    pub channel: Option<String>,
    #[serde(rename = "channel_type")]
    pub channel_type: Option<String>,
    pub ts: Option<String>,
    #[serde(rename = "thread_ts")]
    pub thread_ts: Option<String>,
    pub bot_id: Option<String>,
}

/// Slack message for sending
#[derive(Debug, Clone, Serialize)]
pub struct SlackMessage {
    pub channel: String,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_ts: Option<String>,
}

pub struct SlackBotChannel {
    id: String,
    bot_token: String,
    signing_secret: Option<String>,
    client: reqwest::Client,
}

impl SlackBotChannel {
    pub fn new(id: String, bot_token: String, signing_secret: Option<String>) -> Self {
        Self {
            id,
            bot_token,
            signing_secret,
            client: reqwest::Client::new(),
        }
    }

    pub fn verify_signature(&self, body: &[u8], timestamp: &str, signature: &str) -> Result<bool> {
        let Some(secret) = self.signing_secret.as_ref() else {
            return Ok(true);
        };

        verify_signature_with_secret(secret, body, timestamp, signature)
    }

    /// Reply to a message in the same thread
    pub async fn send_message_in_thread(
        &self,
        channel: &str,
        text: &str,
        thread_ts: Option<&str>,
    ) -> Result<()> {
        let url = "https://slack.com/api/chat.postMessage";

        let message = SlackMessage {
            channel: channel.to_string(),
            text: text.to_string(),
            thread_ts: thread_ts.map(|s| s.to_string()),
        };

        let response = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", self.bot_token))
            .header("Content-Type", "application/json")
            .json(&message)
            .send()
            .await
            .map_err(|e| {
                crate::error::AgentError::ChannelError(format!("Slack API error: {}", e))
            })?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(crate::error::AgentError::ChannelError(format!(
                "Slack API returned {}: {}",
                status, body
            )));
        }

        // Check Slack API response for ok: false
        let response_json: serde_json::Value = response.json().await.map_err(|e| {
            crate::error::AgentError::ChannelError(format!("Failed to parse Slack response: {}", e))
        })?;

        if !response_json
            .get("ok")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            let error = response_json
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown error");
            return Err(crate::error::AgentError::ChannelError(format!(
                "Slack API error: {}",
                error
            )));
        }

        Ok(())
    }
}

pub fn verify_signature_with_secret(
    secret: &str,
    body: &[u8],
    timestamp: &str,
    signature: &str,
) -> Result<bool> {
    let provided_signature = signature.strip_prefix("v0=").ok_or_else(|| {
        crate::error::AgentError::ChannelError("Slack signature must start with v0=".to_string())
    })?;
    let provided_signature = hex::decode(provided_signature).map_err(|e| {
        crate::error::AgentError::ChannelError(format!("Invalid Slack signature encoding: {}", e))
    })?;

    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|e| crate::error::AgentError::ChannelError(format!("HMAC error: {}", e)))?;
    let sig_basestring = format!("v0:{}:{}", timestamp, String::from_utf8_lossy(body));
    mac.update(sig_basestring.as_bytes());

    Ok(mac.verify_slice(&provided_signature).is_ok())
}

#[async_trait]
impl Channel for SlackBotChannel {
    fn channel_type(&self) -> ChannelType {
        ChannelType::Slack
    }

    fn id(&self) -> &str {
        &self.id
    }

    async fn send_message(&self, user_id: &str, content: &str) -> Result<()> {
        // user_id is the channel ID for Slack
        self.send_message_in_thread(user_id, content, None).await
    }

    async fn receive_message(&self) -> Result<Option<ChannelMessage>> {
        Ok(None)
    }

    async fn health_check(&self) -> Result<()> {
        let url = "https://slack.com/api/auth.test";

        let response = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", self.bot_token))
            .send()
            .await
            .map_err(|e| {
                crate::error::AgentError::ChannelError(format!("Slack health check failed: {}", e))
            })?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(crate::error::AgentError::ChannelError(format!(
                "Slack API returned {}: {}",
                status, body
            )));
        }

        let response_json: serde_json::Value = response.json().await.map_err(|e| {
            crate::error::AgentError::ChannelError(format!("Failed to parse Slack response: {}", e))
        })?;

        if !response_json
            .get("ok")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            let error = response_json
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown error");
            return Err(crate::error::AgentError::ChannelError(format!(
                "Slack auth failed: {}",
                error
            )));
        }

        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

pub async fn slack_webhook_handler(
    agentim: Arc<AgentIM>,
    agent_id: &str,
    max_session_messages: Option<usize>,
    context_message_limit: usize,
    agent_timeout_ms: Option<u64>,
    event: SlackEvent,
) -> Result<Option<String>> {
    // Handle URL verification challenge
    if let Some(challenge) = event.challenge {
        return Ok(Some(challenge));
    }

    let Some(event_detail) = event.event else {
        return Ok(None);
    };

    // Only process message events
    if event_detail.detail_type != "message" {
        return Ok(None);
    }

    // Skip bot messages (including our own)
    if event_detail.bot_id.is_some() {
        return Ok(None);
    }

    let Some(user_id) = event_detail.user else {
        return Ok(None);
    };

    let Some(text) = event_detail.text else {
        return Ok(None);
    };

    let Some(channel) = event_detail.channel else {
        return Ok(None);
    };

    // Use thread_ts as reply target if available, otherwise use channel
    let reply_target = event_detail.thread_ts.as_deref().unwrap_or(&channel);

    agentim
        .handle_incoming_message_with_options(
            agent_id,
            SLACK_CHANNEL_ID,
            &user_id,
            Some(reply_target),
            text,
            MessageHandlingOptions {
                max_messages: max_session_messages,
                context_message_limit,
                agent_timeout_ms,
            },
        )
        .await?;

    Ok(None)
}

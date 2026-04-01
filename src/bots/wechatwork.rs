use crate::channel::{Channel, ChannelMessage};
use crate::config::ChannelType;
use crate::error::Result;
use crate::manager::{AgentIM, MessageHandlingOptions};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub const WECHATWORK_CHANNEL_ID: &str = "wechatwork-bot";

/// WeChat Work webhook message (from enterprise WeChat to bot)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeChatWorkWebhook {
    #[serde(rename = "ToUserName")]
    pub to_user_name: String,
    #[serde(rename = "FromUserName")]
    pub from_user_name: String,
    #[serde(rename = "CreateTime")]
    pub create_time: i64,
    #[serde(rename = "MsgType")]
    pub msg_type: String,
    #[serde(rename = "Content")]
    pub content: Option<String>,
    #[serde(rename = "MsgId")]
    pub msg_id: Option<String>,
    #[serde(rename = "AgentID")]
    pub agent_id: Option<String>,
    #[serde(rename = "ChatId")]
    pub chat_id: Option<String>,
}

/// WeChat Work message for sending via application
#[derive(Debug, Clone, Serialize)]
pub struct WeChatWorkOutMessage {
    #[serde(rename = "touser")]
    pub to_user: Option<String>,
    #[serde(rename = "toparty")]
    pub to_party: Option<String>,
    #[serde(rename = "totag")]
    pub to_tag: Option<String>,
    #[serde(rename = "msgtype")]
    pub msg_type: String,
    pub text: WeChatWorkText,
    #[serde(rename = "agentid")]
    pub agent_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WeChatWorkText {
    pub content: String,
}

/// WeChat Work access token response
#[derive(Debug, Clone, Deserialize)]
pub struct WeChatWorkTokenResponse {
    #[serde(rename = "errcode")]
    pub err_code: i64,
    #[serde(rename = "errmsg")]
    pub err_msg: String,
    #[serde(rename = "access_token")]
    pub access_token: Option<String>,
    #[serde(rename = "expires_in")]
    pub expires_in: Option<i64>,
}

pub struct WeChatWorkBotChannel {
    id: String,
    corp_id: String,
    agent_id: String,
    secret: String,
    client: reqwest::Client,
}

impl WeChatWorkBotChannel {
    pub fn new(id: String, corp_id: String, agent_id: String, secret: String) -> Self {
        Self {
            id,
            corp_id,
            agent_id,
            secret,
            client: reqwest::Client::new(),
        }
    }

    /// Get access token from WeChat Work API
    pub async fn get_access_token(&self) -> Result<String> {
        let url = format!(
            "https://qyapi.weixin.qq.com/cgi-bin/gettoken?corpid={}&corpsecret={}",
            self.corp_id, self.secret
        );

        let response = self.client.get(&url).send().await.map_err(|e| {
            crate::error::AgentError::ChannelError(format!(
                "WeChat Work token request failed: {}",
                e
            ))
        })?;

        let token_response: WeChatWorkTokenResponse = response.json().await.map_err(|e| {
            crate::error::AgentError::ChannelError(format!(
                "Failed to parse WeChat Work token response: {}",
                e
            ))
        })?;

        if token_response.err_code != 0 {
            return Err(crate::error::AgentError::ChannelError(format!(
                "WeChat Work API error: {}",
                token_response.err_msg
            )));
        }

        token_response.access_token.ok_or_else(|| {
            crate::error::AgentError::ChannelError(
                "WeChat Work token response missing access_token".to_string(),
            )
        })
    }

    /// Send message via WeChat Work application API
    pub async fn send_app_message(&self, to_user: &str, content: &str) -> Result<()> {
        let access_token = self.get_access_token().await?;

        let url = format!(
            "https://qyapi.weixin.qq.com/cgi-bin/message/send?access_token={}",
            access_token
        );

        let message = WeChatWorkOutMessage {
            to_user: Some(to_user.to_string()),
            to_party: None,
            to_tag: None,
            msg_type: "text".to_string(),
            text: WeChatWorkText {
                content: content.to_string(),
            },
            agent_id: Some(self.agent_id.clone()),
        };

        let response = self
            .client
            .post(&url)
            .json(&message)
            .send()
            .await
            .map_err(|e| {
                crate::error::AgentError::ChannelError(format!("WeChat Work API error: {}", e))
            })?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(crate::error::AgentError::ChannelError(format!(
                "WeChat Work API returned {}: {}",
                status, body
            )));
        }

        let response_json: serde_json::Value = response.json().await.map_err(|e| {
            crate::error::AgentError::ChannelError(format!(
                "Failed to parse WeChat Work response: {}",
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
                "WeChat Work API error: {}",
                errmsg
            )));
        }

        Ok(())
    }
}

#[async_trait]
impl Channel for WeChatWorkBotChannel {
    fn channel_type(&self) -> ChannelType {
        ChannelType::WeChatWork
    }

    fn id(&self) -> &str {
        &self.id
    }

    async fn send_message(&self, user_id: &str, content: &str) -> Result<()> {
        self.send_app_message(user_id, content).await
    }

    async fn receive_message(&self) -> Result<Option<ChannelMessage>> {
        Ok(None)
    }

    async fn health_check(&self) -> Result<()> {
        // Try to get access token to verify credentials
        self.get_access_token().await?;
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

pub async fn wechatwork_webhook_handler(
    agentim: Arc<AgentIM>,
    agent_id: &str,
    max_session_messages: Option<usize>,
    context_message_limit: usize,
    agent_timeout_ms: Option<u64>,
    webhook: WeChatWorkWebhook,
) -> Result<()> {
    // Only process text messages
    if webhook.msg_type != "text" {
        return Ok(());
    }

    let text = webhook.content.unwrap_or_default();

    // Skip empty messages
    if text.trim().is_empty() {
        return Ok(());
    }

    // Use FromUserName as user_id
    let user_id = webhook.from_user_name;

    // Use ChatId if available (for group chats), otherwise use user_id
    let reply_target: Option<&str> = webhook.chat_id.as_deref();

    agentim
        .handle_incoming_message_with_options(
            agent_id,
            WECHATWORK_CHANNEL_ID,
            &user_id,
            reply_target,
            text,
            MessageHandlingOptions {
                max_messages: max_session_messages,
                context_message_limit,
                agent_timeout_ms,
            },
        )
        .await?;

    Ok(())
}

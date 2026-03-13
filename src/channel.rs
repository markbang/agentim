use async_trait::async_trait;
use crate::error::Result;
use crate::config::ChannelType;

#[derive(Debug, Clone)]
pub struct ChannelMessage {
    pub user_id: String,
    pub content: String,
    pub metadata: std::collections::HashMap<String, String>,
}

#[async_trait]
pub trait Channel: Send + Sync {
    fn channel_type(&self) -> ChannelType;
    fn id(&self) -> &str;

    async fn send_message(&self, user_id: &str, content: &str) -> Result<()>;
    async fn receive_message(&self) -> Result<Option<ChannelMessage>>;
    async fn health_check(&self) -> Result<()>;
}

pub struct TelegramChannel {
    id: String,
    bot_token: String,
}

impl TelegramChannel {
    pub fn new(id: String, bot_token: String) -> Self {
        Self { id, bot_token }
    }
}

#[async_trait]
impl Channel for TelegramChannel {
    fn channel_type(&self) -> ChannelType {
        ChannelType::Telegram
    }

    fn id(&self) -> &str {
        &self.id
    }

    async fn send_message(&self, user_id: &str, content: &str) -> Result<()> {
        let client = reqwest::Client::new();
        let url = format!(
            "https://api.telegram.org/bot{}/sendMessage",
            self.bot_token
        );

        let body = serde_json::json!({
            "chat_id": user_id,
            "text": content
        });

        client.post(&url).json(&body).send().await?;
        Ok(())
    }

    async fn receive_message(&self) -> Result<Option<ChannelMessage>> {
        // Placeholder for webhook/polling implementation
        Ok(None)
    }

    async fn health_check(&self) -> Result<()> {
        let client = reqwest::Client::new();
        let url = format!(
            "https://api.telegram.org/bot{}/getMe",
            self.bot_token
        );
        client.get(&url).send().await?;
        Ok(())
    }
}

pub struct DiscordChannel {
    id: String,
    bot_token: String,
}

impl DiscordChannel {
    pub fn new(id: String, bot_token: String) -> Self {
        Self { id, bot_token }
    }
}

#[async_trait]
impl Channel for DiscordChannel {
    fn channel_type(&self) -> ChannelType {
        ChannelType::Discord
    }

    fn id(&self) -> &str {
        &self.id
    }

    async fn send_message(&self, user_id: &str, content: &str) -> Result<()> {
        let client = reqwest::Client::new();
        let url = format!(
            "https://discord.com/api/v10/channels/{}/messages",
            user_id
        );

        let body = serde_json::json!({
            "content": content
        });

        client
            .post(&url)
            .header("Authorization", format!("Bot {}", self.bot_token))
            .json(&body)
            .send()
            .await?;
        Ok(())
    }

    async fn receive_message(&self) -> Result<Option<ChannelMessage>> {
        // Placeholder for webhook implementation
        Ok(None)
    }

    async fn health_check(&self) -> Result<()> {
        let client = reqwest::Client::new();
        client
            .get("https://discord.com/api/v10/users/@me")
            .header("Authorization", format!("Bot {}", self.bot_token))
            .send()
            .await?;
        Ok(())
    }
}

pub struct FeishuChannel {
    id: String,
    app_id: String,
    app_secret: String,
}

impl FeishuChannel {
    pub fn new(id: String, app_id: String, app_secret: String) -> Self {
        Self {
            id,
            app_id,
            app_secret,
        }
    }
}

#[async_trait]
impl Channel for FeishuChannel {
    fn channel_type(&self) -> ChannelType {
        ChannelType::Feishu
    }

    fn id(&self) -> &str {
        &self.id
    }

    async fn send_message(&self, user_id: &str, content: &str) -> Result<()> {
        let client = reqwest::Client::new();
        let url = "https://open.feishu.cn/open-apis/im/v1/messages";

        let body = serde_json::json!({
            "receive_id": user_id,
            "msg_type": "text",
            "content": serde_json::json!({
                "text": content
            })
        });

        client
            .post(url)
            .header("Authorization", format!("Bearer {}", self.app_id))
            .json(&body)
            .send()
            .await?;
        Ok(())
    }

    async fn receive_message(&self) -> Result<Option<ChannelMessage>> {
        Ok(None)
    }

    async fn health_check(&self) -> Result<()> {
        Ok(())
    }
}

pub struct QQChannel {
    id: String,
    bot_id: String,
    bot_token: String,
}

impl QQChannel {
    pub fn new(id: String, bot_id: String, bot_token: String) -> Self {
        Self {
            id,
            bot_id,
            bot_token,
        }
    }
}

#[async_trait]
impl Channel for QQChannel {
    fn channel_type(&self) -> ChannelType {
        ChannelType::QQ
    }

    fn id(&self) -> &str {
        &self.id
    }

    async fn send_message(&self, user_id: &str, content: &str) -> Result<()> {
        let client = reqwest::Client::new();
        let url = format!(
            "https://api.sgroup.qq.com/channels/{}/messages",
            user_id
        );

        let body = serde_json::json!({
            "content": content
        });

        client
            .post(&url)
            .header("Authorization", format!("Bot {}.{}", self.bot_id, self.bot_token))
            .json(&body)
            .send()
            .await?;
        Ok(())
    }

    async fn receive_message(&self) -> Result<Option<ChannelMessage>> {
        Ok(None)
    }

    async fn health_check(&self) -> Result<()> {
        Ok(())
    }
}

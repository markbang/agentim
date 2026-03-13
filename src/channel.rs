use crate::config::ChannelType;
use crate::error::Result;
use async_trait::async_trait;

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

/// Telegram Channel - 本地模拟实现
pub struct TelegramChannel {
    id: String,
}

impl TelegramChannel {
    pub fn new(id: String) -> Self {
        Self { id }
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
        println!("[Telegram {}] -> User {}: {}", self.id, user_id, content);
        Ok(())
    }

    async fn receive_message(&self) -> Result<Option<ChannelMessage>> {
        Ok(None)
    }

    async fn health_check(&self) -> Result<()> {
        Ok(())
    }
}

/// Discord Channel - 本地模拟实现
pub struct DiscordChannel {
    id: String,
}

impl DiscordChannel {
    pub fn new(id: String) -> Self {
        Self { id }
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
        println!("[Discord {}] -> User {}: {}", self.id, user_id, content);
        Ok(())
    }

    async fn receive_message(&self) -> Result<Option<ChannelMessage>> {
        Ok(None)
    }

    async fn health_check(&self) -> Result<()> {
        Ok(())
    }
}

/// Feishu Channel - 本地模拟实现
pub struct FeishuChannel {
    id: String,
}

impl FeishuChannel {
    pub fn new(id: String) -> Self {
        Self { id }
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
        println!("[Feishu {}] -> User {}: {}", self.id, user_id, content);
        Ok(())
    }

    async fn receive_message(&self) -> Result<Option<ChannelMessage>> {
        Ok(None)
    }

    async fn health_check(&self) -> Result<()> {
        Ok(())
    }
}

/// QQ Channel - 本地模拟实现
pub struct QQChannel {
    id: String,
}

impl QQChannel {
    pub fn new(id: String) -> Self {
        Self { id }
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
        println!("[QQ {}] -> User {}: {}", self.id, user_id, content);
        Ok(())
    }

    async fn receive_message(&self) -> Result<Option<ChannelMessage>> {
        Ok(None)
    }

    async fn health_check(&self) -> Result<()> {
        Ok(())
    }
}

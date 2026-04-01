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

    /// Support downcasting for platform-specific operations
    fn as_any(&self) -> &dyn std::any::Any {
        unreachable!("as_any should be implemented by concrete types")
    }
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

/// Slack Channel - 本地模拟实现
pub struct SlackChannel {
    id: String,
}

impl SlackChannel {
    pub fn new(id: String) -> Self {
        Self { id }
    }
}

#[async_trait]
impl Channel for SlackChannel {
    fn channel_type(&self) -> ChannelType {
        ChannelType::Slack
    }

    fn id(&self) -> &str {
        &self.id
    }

    async fn send_message(&self, user_id: &str, content: &str) -> Result<()> {
        println!("[Slack {}] -> User {}: {}", self.id, user_id, content);
        Ok(())
    }

    async fn receive_message(&self) -> Result<Option<ChannelMessage>> {
        Ok(None)
    }

    async fn health_check(&self) -> Result<()> {
        Ok(())
    }
}

/// DingTalk Channel - 本地模拟实现
pub struct DingTalkChannel {
    id: String,
}

impl DingTalkChannel {
    pub fn new(id: String) -> Self {
        Self { id }
    }
}

#[async_trait]
impl Channel for DingTalkChannel {
    fn channel_type(&self) -> ChannelType {
        ChannelType::DingTalk
    }

    fn id(&self) -> &str {
        &self.id
    }

    async fn send_message(&self, user_id: &str, content: &str) -> Result<()> {
        println!("[DingTalk {}] -> User {}: {}", self.id, user_id, content);
        Ok(())
    }

    async fn receive_message(&self) -> Result<Option<ChannelMessage>> {
        Ok(None)
    }

    async fn health_check(&self) -> Result<()> {
        Ok(())
    }
}

/// WeChatWork Channel - 本地模拟实现
pub struct WeChatWorkChannel {
    id: String,
}

impl WeChatWorkChannel {
    pub fn new(id: String) -> Self {
        Self { id }
    }
}

#[async_trait]
impl Channel for WeChatWorkChannel {
    fn channel_type(&self) -> ChannelType {
        ChannelType::WeChatWork
    }

    fn id(&self) -> &str {
        &self.id
    }

    async fn send_message(&self, user_id: &str, content: &str) -> Result<()> {
        println!("[WeChatWork {}] -> User {}: {}", self.id, user_id, content);
        Ok(())
    }

    async fn receive_message(&self) -> Result<Option<ChannelMessage>> {
        Ok(None)
    }

    async fn health_check(&self) -> Result<()> {
        Ok(())
    }
}

/// Line Channel - 本地模拟实现
pub struct LineChannel {
    id: String,
}

impl LineChannel {
    pub fn new(id: String) -> Self {
        Self { id }
    }
}

#[async_trait]
impl Channel for LineChannel {
    fn channel_type(&self) -> ChannelType {
        ChannelType::Line
    }

    fn id(&self) -> &str {
        &self.id
    }

    async fn send_message(&self, user_id: &str, content: &str) -> Result<()> {
        println!("[Line {}] -> User {}: {}", self.id, user_id, content);
        Ok(())
    }

    async fn receive_message(&self) -> Result<Option<ChannelMessage>> {
        Ok(None)
    }

    async fn health_check(&self) -> Result<()> {
        Ok(())
    }
}

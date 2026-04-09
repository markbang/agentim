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

    /// Support downcasting for platform-specific operations.
    /// Concrete types in src/bots/ override this.
    fn as_any(&self) -> &dyn std::any::Any {
        unreachable!("as_any should be implemented by concrete types")
    }
}

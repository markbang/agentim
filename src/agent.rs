use crate::config::AgentType;
use crate::error::Result;
use crate::session::{Message, Session};
use async_trait::async_trait;

#[async_trait]
pub trait Agent: Send + Sync {
    fn agent_type(&self) -> AgentType;
    fn id(&self) -> &str;

    async fn send_message(&self, messages: Vec<Message>) -> Result<String>;
    async fn send_message_with_session(
        &self,
        session: &mut Session,
        messages: Vec<Message>,
    ) -> Result<String> {
        let _ = session;
        self.send_message(messages).await
    }
    async fn health_check(&self) -> Result<()>;
}

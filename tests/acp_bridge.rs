#![allow(dead_code)]
mod config {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub enum AgentType {
        Acp,
    }
}

mod error {
    use thiserror::Error;

    #[derive(Error, Debug)]
    pub enum AgentError {
        #[error("API error: {0}")]
        ApiError(String),

        #[error("Invalid configuration: {0}")]
        ConfigError(String),

        #[error("Serialization error: {0}")]
        SerializationError(#[from] serde_json::Error),

        #[error("IO error: {0}")]
        IoError(#[from] std::io::Error),

        #[error("Unknown error: {0}")]
        Unknown(String),
    }

    pub type Result<T> = std::result::Result<T, AgentError>;
}

mod session {
    pub use agentim::session::{Message, MessageRole, Session};
}

mod agent {
    use super::config::AgentType;
    use super::error::Result;
    use super::session::{Message, Session};
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
}

#[path = "../src/acp.rs"]
mod acp_impl;

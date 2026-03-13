use thiserror::Error;

#[derive(Error, Debug)]
pub enum AgentError {
    #[error("Agent not found: {0}")]
    AgentNotFound(String),

    #[error("Channel not found: {0}")]
    ChannelNotFound(String),

    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("API error: {0}")]
    ApiError(String),

    #[error("Invalid configuration: {0}")]
    ConfigError(String),

    #[error("Message send failed: {0}")]
    SendError(String),

    #[error("Channel error: {0}")]
    ChannelError(String),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Parse error: {0}")]
    ParseError(#[from] std::num::ParseIntError),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

pub type Result<T> = std::result::Result<T, AgentError>;

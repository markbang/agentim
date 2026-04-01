use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub agent_type: AgentType,
    pub api_key: String,
    pub model: Option<String>,
    pub base_url: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AgentType {
    Claude,
    Codex,
    Pi,
    OpenAI,
    Acp,
    Gemini,
}

impl std::fmt::Display for AgentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentType::Claude => write!(f, "claude"),
            AgentType::Codex => write!(f, "codex"),
            AgentType::Pi => write!(f, "pi"),
            AgentType::OpenAI => write!(f, "openai"),
            AgentType::Acp => write!(f, "acp"),
            AgentType::Gemini => write!(f, "gemini"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelConfig {
    pub channel_type: ChannelType,
    pub credentials: HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ChannelType {
    Telegram,
    Discord,
    Feishu,
    QQ,
    Slack,
    DingTalk,
    WeChatWork,
    Line,
}

impl std::fmt::Display for ChannelType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChannelType::Telegram => write!(f, "telegram"),
            ChannelType::Discord => write!(f, "discord"),
            ChannelType::Feishu => write!(f, "feishu"),
            ChannelType::QQ => write!(f, "qq"),
            ChannelType::Slack => write!(f, "slack"),
            ChannelType::DingTalk => write!(f, "dingtalk"),
            ChannelType::WeChatWork => write!(f, "wechatwork"),
            ChannelType::Line => write!(f, "line"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub agents: HashMap<String, AgentConfig>,
    pub channels: HashMap<String, ChannelConfig>,
}

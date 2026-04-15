use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AgentType {
    Acp,
}

impl std::fmt::Display for AgentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentType::Acp => write!(f, "acp"),
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

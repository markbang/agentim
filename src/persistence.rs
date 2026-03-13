use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub id: String,
    pub agent_type: String,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelConfig {
    pub id: String,
    pub channel_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub agent_id: String,
    pub channel_id: String,
    pub user_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistenceConfig {
    pub agents: Vec<AgentConfig>,
    pub channels: Vec<ChannelConfig>,
    pub sessions: Vec<SessionConfig>,
}

impl PersistenceConfig {
    pub fn new() -> Self {
        Self {
            agents: Vec::new(),
            channels: Vec::new(),
            sessions: Vec::new(),
        }
    }

    pub fn load(path: &str) -> anyhow::Result<Self> {
        if !Path::new(path).exists() {
            return Ok(Self::new());
        }
        let content = fs::read_to_string(path)?;
        let config = serde_json::from_str(&content)?;
        Ok(config)
    }

    pub fn save(&self, path: &str) -> anyhow::Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }

    pub fn add_agent(&mut self, agent: AgentConfig) {
        self.agents.push(agent);
    }

    pub fn add_channel(&mut self, channel: ChannelConfig) {
        self.channels.push(channel);
    }

    pub fn add_session(&mut self, session: SessionConfig) {
        self.sessions.push(session);
    }
}

impl Default for PersistenceConfig {
    fn default() -> Self {
        Self::new()
    }
}

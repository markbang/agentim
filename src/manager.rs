use crate::agent::Agent;
use crate::channel::Channel;
use crate::error::{AgentError, Result};
use crate::session::Session;
use dashmap::DashMap;
use std::sync::Arc;

pub struct AgentIM {
    agents: Arc<DashMap<String, Arc<dyn Agent>>>,
    channels: Arc<DashMap<String, Arc<dyn Channel>>>,
    sessions: Arc<DashMap<String, Session>>,
}

impl AgentIM {
    pub fn new() -> Self {
        Self {
            agents: Arc::new(DashMap::new()),
            channels: Arc::new(DashMap::new()),
            sessions: Arc::new(DashMap::new()),
        }
    }

    pub fn register_agent(&self, id: String, agent: Arc<dyn Agent>) -> Result<()> {
        self.agents.insert(id, agent);
        Ok(())
    }

    pub fn register_channel(&self, id: String, channel: Arc<dyn Channel>) -> Result<()> {
        self.channels.insert(id, channel);
        Ok(())
    }

    pub fn get_agent(&self, id: &str) -> Result<Arc<dyn Agent>> {
        self.agents
            .get(id)
            .map(|entry| entry.value().clone())
            .ok_or_else(|| AgentError::AgentNotFound(id.to_string()))
    }

    pub fn get_channel(&self, id: &str) -> Result<Arc<dyn Channel>> {
        self.channels
            .get(id)
            .map(|entry| entry.value().clone())
            .ok_or_else(|| AgentError::ChannelNotFound(id.to_string()))
    }

    pub fn create_session(
        &self,
        agent_id: String,
        channel_id: String,
        user_id: String,
    ) -> Result<String> {
        // Verify agent and channel exist
        self.get_agent(&agent_id)?;
        self.get_channel(&channel_id)?;

        let session = Session::new(agent_id, channel_id, user_id);
        let session_id = session.id.clone();
        self.sessions.insert(session_id.clone(), session);
        Ok(session_id)
    }

    pub fn get_session(&self, id: &str) -> Result<Session> {
        self.sessions
            .get(id)
            .map(|entry| entry.value().clone())
            .ok_or_else(|| AgentError::SessionNotFound(id.to_string()))
    }

    pub fn update_session(&self, id: &str, session: Session) -> Result<()> {
        self.sessions.insert(id.to_string(), session);
        Ok(())
    }

    pub fn list_sessions(&self) -> Vec<Session> {
        self.sessions
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    pub fn list_agents(&self) -> Vec<String> {
        self.agents
            .iter()
            .map(|entry| entry.key().clone())
            .collect()
    }

    pub fn list_channels(&self) -> Vec<String> {
        self.channels
            .iter()
            .map(|entry| entry.key().clone())
            .collect()
    }

    pub async fn send_to_agent(&self, session_id: &str, user_message: String) -> Result<String> {
        let mut session = self.get_session(session_id)?;
        let agent = self.get_agent(&session.agent_id)?;

        // Add user message to session
        session.add_message(crate::session::MessageRole::User, user_message);

        // Get context for agent
        let context = session.get_context(10);

        // Send to agent
        let response = agent.send_message(context).await?;

        // Add agent response to session
        session.add_message(crate::session::MessageRole::Assistant, response.clone());

        // Update session
        self.update_session(session_id, session)?;

        Ok(response)
    }

    pub async fn send_to_channel(&self, session_id: &str, message: String) -> Result<()> {
        let session = self.get_session(session_id)?;
        let channel = self.get_channel(&session.channel_id)?;

        channel.send_message(&session.user_id, &message).await?;
        Ok(())
    }

    pub async fn health_check(&self) -> Result<()> {
        for agent_ref in self.agents.iter() {
            agent_ref.value().health_check().await?;
        }

        for channel_ref in self.channels.iter() {
            channel_ref.value().health_check().await?;
        }

        Ok(())
    }

    pub fn find_or_create_session(
        &self,
        agent_id: &str,
        channel_id: &str,
        user_id: &str,
    ) -> Result<String> {
        // Look for existing session
        for session_ref in self.sessions.iter() {
            let session = session_ref.value();
            if session.agent_id == agent_id
                && session.channel_id == channel_id
                && session.user_id == user_id
            {
                return Ok(session.id.clone());
            }
        }

        // Create new session if not found
        self.create_session(
            agent_id.to_string(),
            channel_id.to_string(),
            user_id.to_string(),
        )
    }

    pub fn delete_session(&self, id: &str) -> Result<()> {
        self.sessions
            .remove(id)
            .ok_or_else(|| AgentError::SessionNotFound(id.to_string()))?;
        Ok(())
    }
}

impl Default for AgentIM {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for AgentIM {
    fn clone(&self) -> Self {
        Self {
            agents: self.agents.clone(),
            channels: self.channels.clone(),
            sessions: self.sessions.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::ClaudeAgent;
    use crate::channel::TelegramChannel;

    #[test]
    fn test_agentim_creation() {
        let agentim = AgentIM::new();
        assert_eq!(agentim.list_agents().len(), 0);
        assert_eq!(agentim.list_channels().len(), 0);
    }

    #[test]
    fn test_register_agent() {
        let agentim = AgentIM::new();
        let agent = Arc::new(ClaudeAgent::new("claude1".to_string(), None));
        agentim
            .register_agent("claude1".to_string(), agent)
            .unwrap();
        assert_eq!(agentim.list_agents().len(), 1);
    }

    #[test]
    fn test_create_session() {
        let agentim = AgentIM::new();
        let agent = Arc::new(ClaudeAgent::new("claude1".to_string(), None));
        let channel = Arc::new(TelegramChannel::new("tg1".to_string()));

        agentim
            .register_agent("claude1".to_string(), agent)
            .unwrap();
        agentim
            .register_channel("tg1".to_string(), channel)
            .unwrap();

        let session_id = agentim
            .create_session(
                "claude1".to_string(),
                "tg1".to_string(),
                "user1".to_string(),
            )
            .unwrap();

        let session = agentim.get_session(&session_id).unwrap();
        assert_eq!(session.agent_id, "claude1");
        assert_eq!(session.channel_id, "tg1");
    }
}

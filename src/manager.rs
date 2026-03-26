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
        let reply_target = session
            .metadata
            .get("reply_target")
            .cloned()
            .unwrap_or_else(|| session.user_id.clone());

        channel.send_message(&reply_target, &message).await?;
        Ok(())
    }

    pub async fn handle_incoming_message(
        &self,
        agent_id: &str,
        channel_id: &str,
        user_id: &str,
        reply_target: Option<&str>,
        user_message: String,
    ) -> Result<String> {
        let session_id = self.find_or_create_session(agent_id, channel_id, user_id)?;

        if let Some(reply_target) = reply_target {
            let mut session = self.get_session(&session_id)?;
            session
                .metadata
                .insert("reply_target".to_string(), reply_target.to_string());
            self.update_session(&session_id, session)?;
        }

        let response = self.send_to_agent(&session_id, user_message).await?;
        self.send_to_channel(&session_id, response.clone()).await?;
        Ok(response)
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
    use crate::agent::{Agent, ClaudeAgent};
    use crate::channel::{Channel, ChannelMessage, TelegramChannel};
    use crate::config::{AgentType, ChannelType};
    use crate::session::Message;
    use async_trait::async_trait;
    use std::sync::{Arc, Mutex};

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

    struct MockAgent;

    #[async_trait]
    impl Agent for MockAgent {
        fn agent_type(&self) -> AgentType {
            AgentType::Claude
        }

        fn id(&self) -> &str {
            "mock-agent"
        }

        async fn send_message(&self, messages: Vec<Message>) -> Result<String> {
            let last = messages.last().map(|msg| msg.content.clone()).unwrap_or_default();
            Ok(format!("echo:{}", last))
        }

        async fn health_check(&self) -> Result<()> {
            Ok(())
        }
    }

    struct MockChannel {
        sent_messages: Arc<Mutex<Vec<(String, String)>>>,
    }

    #[async_trait]
    impl Channel for MockChannel {
        fn channel_type(&self) -> ChannelType {
            ChannelType::Discord
        }

        fn id(&self) -> &str {
            "mock-channel"
        }

        async fn send_message(&self, user_id: &str, content: &str) -> Result<()> {
            self.sent_messages
                .lock()
                .unwrap()
                .push((user_id.to_string(), content.to_string()));
            Ok(())
        }

        async fn receive_message(&self) -> Result<Option<ChannelMessage>> {
            Ok(None)
        }

        async fn health_check(&self) -> Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_handle_incoming_message_autocreates_session_and_uses_reply_target() {
        let agentim = AgentIM::new();
        let sent_messages = Arc::new(Mutex::new(Vec::new()));

        agentim
            .register_agent("default-agent".to_string(), Arc::new(MockAgent))
            .unwrap();
        agentim
            .register_channel(
                "discord-bot".to_string(),
                Arc::new(MockChannel {
                    sent_messages: sent_messages.clone(),
                }),
            )
            .unwrap();

        let response = agentim
            .handle_incoming_message(
                "default-agent",
                "discord-bot",
                "user-1",
                Some("channel-42"),
                "ping".to_string(),
            )
            .await
            .unwrap();

        assert_eq!(response, "echo:ping");

        let sessions = agentim.list_sessions();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].user_id, "user-1");
        assert_eq!(
            sessions[0].metadata.get("reply_target"),
            Some(&"channel-42".to_string())
        );

        let sent = sent_messages.lock().unwrap();
        assert_eq!(sent.as_slice(), &[("channel-42".to_string(), "echo:ping".to_string())]);
    }
}
